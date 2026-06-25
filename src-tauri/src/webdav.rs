use std::fs;
use std::process::Command;

pub const MOUNT_POINT: &str = "/mnt/RealDebrid";
pub const WEBDAV_BASE_URL: &str = "https://dav.real-debrid.com";

/// Read username + plain-text password from rclone.conf via `rclone reveal`.
pub fn get_webdav_credentials() -> Option<(String, String)> {
    let home = dirs::home_dir()?;
    let conf_path = home.join(".config/rclone/rclone.conf");
    let conf = fs::read_to_string(conf_path).ok()?;

    let mut username = String::new();
    let mut obscured_pass = String::new();
    let mut in_section = false;

    for line in conf.lines() {
        let trimmed = line.trim();
        if trimmed == "[realdebrid]" {
            in_section = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_section = false;
        }
        if in_section {
            if let Some(v) = trimmed.strip_prefix("user = ") {
                username = v.trim().to_string();
            } else if let Some(v) = trimmed.strip_prefix("pass = ") {
                obscured_pass = v.trim().to_string();
            }
        }
    }

    if username.is_empty() || obscured_pass.is_empty() {
        return None;
    }

    // Decode rclone-obscured password
    let out = Command::new("rclone")
        .args(["reveal", &obscured_pass])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let password = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Some((username, password))
}

/// Convert a local mount path to its WebDAV HTTP URL.
/// e.g. "/mnt/RealDebrid/Movies/file.mkv" -> "https://dav.real-debrid.com/Movies/file.mkv"
pub fn mount_path_to_http_url(full_path: &str) -> String {
    let rel = full_path
        .strip_prefix(MOUNT_POINT)
        .unwrap_or(full_path)
        .trim_start_matches('/');
    format!("{}/{}", WEBDAV_BASE_URL, rel)
}

#[derive(Debug, Clone)]
pub struct WebDavFileEntry {
    pub name: String,
    pub full_path: String,
    pub is_dir: bool,
    pub size: u64,
}

pub fn list_mount_dir(rel_path: &str) -> Result<Vec<WebDavFileEntry>, String> {
    let base = std::path::Path::new(MOUNT_POINT);
    let dir = if rel_path.is_empty() || rel_path == "/" {
        base.to_path_buf()
    } else {
        base.join(rel_path.trim_start_matches('/'))
    };

    let read = fs::read_dir(&dir).map_err(|e| format!("{e}"))?;
    let mut entries: Vec<WebDavFileEntry> = read
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some(WebDavFileEntry {
                name: e.file_name().to_string_lossy().to_string(),
                full_path: e.path().to_string_lossy().to_string(),
                is_dir: meta.is_dir(),
                size: if meta.is_file() { meta.len() } else { 0 },
            })
        })
        .collect();

    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct WebDavStatus {
    pub platform: String,
    pub rclone_installed: bool,
    pub service_installed: bool,
    pub service_active: bool,
    pub is_mounted: bool,
}

pub fn webdav_status() -> WebDavStatus {
    let platform = std::env::consts::OS.to_string();

    #[cfg(target_os = "linux")]
    {
        let rclone_installed = Command::new("which")
            .arg("rclone")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        let service_installed =
            std::path::Path::new("/etc/systemd/system/realdebrid.service").exists();

        let service_active = Command::new("systemctl")
            .args(["is-active", "--quiet", "realdebrid.service"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        let is_mounted = fs::read_to_string("/proc/mounts")
            .map(|m| m.lines().any(|l| l.contains("/mnt/RealDebrid")))
            .unwrap_or(false);

        return WebDavStatus {
            platform,
            rclone_installed,
            service_installed,
            service_active,
            is_mounted,
        };
    }

    #[allow(unreachable_code)]
    WebDavStatus {
        platform,
        rclone_installed: false,
        service_installed: false,
        service_active: false,
        is_mounted: false,
    }
}

pub fn webdav_setup(username: String, password: String) -> Result<String, String> {
    #[cfg(not(target_os = "linux"))]
    return Err("WebDAV mounting is only supported on Linux".to_string());

    #[cfg(target_os = "linux")]
    {
        let rclone_ok = Command::new("which")
            .arg("rclone")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !rclone_ok {
            let install = Command::new("pkexec")
                .args(["pacman", "-S", "--noconfirm", "rclone", "fuse3"])
                .status()
                .map_err(|e| format!("install failed: {e}"))?;
            if !install.success() {
                return Err(
                    "rclone not found. Install manually: sudo pacman -S rclone fuse3".to_string(),
                );
            }
        }

        let obscure = Command::new("rclone")
            .args(["obscure", &password])
            .output()
            .map_err(|e| e.to_string())?;
        if !obscure.status.success() {
            return Err("rclone obscure failed".to_string());
        }
        let enc_pass = String::from_utf8_lossy(&obscure.stdout).trim().to_string();

        let home = dirs::home_dir().ok_or("cannot determine home directory")?;
        let rclone_dir = home.join(".config/rclone");
        fs::create_dir_all(&rclone_dir).map_err(|e| e.to_string())?;
        let rclone_conf = format!(
            "[realdebrid]\ntype = webdav\nurl = https://dav.real-debrid.com/\nvendor = other\nuser = {}\npass = {}\n",
            username, enc_pass
        );
        fs::write(rclone_dir.join("rclone.conf"), &rclone_conf).map_err(|e| e.to_string())?;

        let tmp = std::env::temp_dir();
        let tmp_root_conf = tmp.join("rdtool_root_rclone.conf");
        let tmp_svc = tmp.join("rdtool_realdebrid.service");
        let tmp_script = tmp.join("rdtool_webdav_setup.sh");

        let root_conf = format!(
            "[realdebrid]\ntype = webdav\nurl = https://dav.real-debrid.com/\nvendor = other\nuser = {}\npass = {}\n",
            username, enc_pass
        );
        let service = "\
[Unit]\n\
Description=Real-Debrid WebDAV (rclone mount)\n\
After=network-online.target\n\
Wants=network-online.target\n\
\n\
[Service]\n\
Type=simple\n\
User=root\n\
Group=root\n\
ExecStart=/usr/bin/rclone mount realdebrid: /mnt/RealDebrid \\\n\
    --vfs-cache-mode full \\\n\
    --vfs-cache-max-size 5G \\\n\
    --vfs-cache-max-age 24h \\\n\
    --buffer-size 32M \\\n\
    --dir-cache-time 12h \\\n\
    --poll-interval 1m \\\n\
    --use-mmap \\\n\
    --allow-other \\\n\
    --allow-non-empty\n\
ExecStop=/bin/fusermount -u /mnt/RealDebrid\n\
Restart=on-failure\n\
RestartSec=10\n\
\n\
[Install]\n\
WantedBy=multi-user.target\n";

        fs::write(&tmp_root_conf, &root_conf).map_err(|e| e.to_string())?;
        fs::write(&tmp_svc, service).map_err(|e| e.to_string())?;

        let script = format!(
            "#!/bin/sh\n\
            mkdir -p /root/.config/rclone\n\
            cp {root} /root/.config/rclone/rclone.conf\n\
            chmod 600 /root/.config/rclone/rclone.conf\n\
            mkdir -p /mnt/RealDebrid\n\
            chmod 755 /mnt/RealDebrid\n\
            cp {svc} /etc/systemd/system/realdebrid.service\n\
            systemctl daemon-reload\n\
            systemctl enable --now realdebrid.service\n",
            root = tmp_root_conf.display(),
            svc = tmp_svc.display(),
        );
        fs::write(&tmp_script, &script).map_err(|e| e.to_string())?;

        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp_script, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;

        let status = Command::new("pkexec")
            .arg(tmp_script.to_str().unwrap())
            .status()
            .map_err(|e| format!("pkexec: {e}"))?;

        let _ = fs::remove_file(&tmp_root_conf);
        let _ = fs::remove_file(&tmp_svc);
        let _ = fs::remove_file(&tmp_script);

        if status.success() {
            Ok("WebDAV mount configured and started at /mnt/RealDebrid".to_string())
        } else {
            Err("Setup failed or was cancelled".to_string())
        }
    }
}

pub fn webdav_start() -> Result<(), String> {
    #[cfg(not(target_os = "linux"))]
    return Err("Linux only".to_string());

    #[cfg(target_os = "linux")]
    {
        let s = Command::new("pkexec")
            .args(["/usr/bin/systemctl", "start", "realdebrid.service"])
            .status()
            .map_err(|e| e.to_string())?;
        if s.success() { Ok(()) } else { Err("Failed to start service".to_string()) }
    }
}

pub fn webdav_stop() -> Result<(), String> {
    #[cfg(not(target_os = "linux"))]
    return Err("Linux only".to_string());

    #[cfg(target_os = "linux")]
    {
        let s = Command::new("pkexec")
            .args(["/usr/bin/systemctl", "stop", "realdebrid.service"])
            .status()
            .map_err(|e| e.to_string())?;
        if s.success() { Ok(()) } else { Err("Failed to stop service".to_string()) }
    }
}

pub fn webdav_uninstall() -> Result<(), String> {
    #[cfg(not(target_os = "linux"))]
    return Err("Linux only".to_string());

    #[cfg(target_os = "linux")]
    {
        let tmp_script = std::env::temp_dir().join("rdtool_webdav_uninstall.sh");
        let script = "#!/bin/sh\n\
            systemctl stop realdebrid.service 2>/dev/null || true\n\
            systemctl disable realdebrid.service 2>/dev/null || true\n\
            rm -f /etc/systemd/system/realdebrid.service\n\
            systemctl daemon-reload\n\
            fusermount -u /mnt/RealDebrid 2>/dev/null || true\n\
            rm -rf /mnt/RealDebrid\n";

        fs::write(&tmp_script, script).map_err(|e| e.to_string())?;

        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp_script, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;

        let s = Command::new("pkexec")
            .arg(tmp_script.to_str().unwrap())
            .status()
            .map_err(|e| e.to_string())?;

        let _ = fs::remove_file(&tmp_script);

        if s.success() { Ok(()) } else { Err("Uninstall failed or cancelled".to_string()) }
    }
}

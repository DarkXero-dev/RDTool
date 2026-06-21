import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Loader2, FolderOpen, Check, HardDrive, Play, Square, Trash2, RefreshCw } from "lucide-react";

interface WebDavStatus {
  platform: string;
  rclone_installed: boolean;
  service_installed: boolean;
  service_active: boolean;
  is_mounted: boolean;
}

function WebDavSection() {
  const [status, setStatus] = useState<WebDavStatus | null>(null);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");
  const [err, setErr] = useState("");

  async function refresh() {
    const s = await invoke<WebDavStatus>("webdav_status");
    setStatus(s);
  }

  useEffect(() => { refresh(); }, []);

  async function run(fn: () => Promise<unknown>, successMsg: string) {
    setBusy(true);
    setMsg("");
    setErr("");
    try {
      await fn();
      setMsg(successMsg);
      await refresh();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  if (!status) return null;

  if (status.platform !== "linux") {
    return (
      <section className="flex flex-col gap-3">
        <h2 className="text-sm font-semibold text-muted-foreground uppercase tracking-wider flex items-center gap-2">
          <HardDrive size={14} /> WebDAV Mount
        </h2>
        <p className="text-xs text-muted-foreground">Linux only.</p>
      </section>
    );
  }

  const configured = status.service_installed;

  return (
    <section className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-muted-foreground uppercase tracking-wider flex items-center gap-2">
          <HardDrive size={14} /> WebDAV Mount
        </h2>
        <button onClick={refresh} className="text-muted-foreground hover:text-foreground" title="Refresh status">
          <RefreshCw size={13} className={busy ? "animate-spin" : ""} />
        </button>
      </div>

      <p className="text-xs text-muted-foreground">
        Mounts Real-Debrid as a local filesystem at{" "}
        <span className="font-mono text-foreground">/mnt/RealDebrid</span> via rclone + systemd.
        Requires polkit (pkexec).
      </p>

      <div className="flex items-center gap-4 text-xs">
        <span className={`flex items-center gap-1.5 ${status.rclone_installed ? "text-rd-green" : "text-muted-foreground"}`}>
          <span className={`w-2 h-2 rounded-full ${status.rclone_installed ? "bg-rd-green" : "bg-muted"}`} />
          rclone {status.rclone_installed ? "installed" : "not found"}
        </span>
        <span className={`flex items-center gap-1.5 ${status.service_active ? "text-rd-green" : "text-muted-foreground"}`}>
          <span className={`w-2 h-2 rounded-full ${status.service_active ? "bg-rd-green" : "bg-muted"}`} />
          service {status.service_active ? "active" : status.service_installed ? "stopped" : "not installed"}
        </span>
        <span className={`flex items-center gap-1.5 ${status.is_mounted ? "text-rd-green" : "text-muted-foreground"}`}>
          <span className={`w-2 h-2 rounded-full ${status.is_mounted ? "bg-rd-green" : "bg-muted"}`} />
          {status.is_mounted ? "mounted" : "not mounted"}
        </span>
      </div>

      {!configured && (
        <div className="flex flex-col gap-3 p-3 rounded-lg border border-border bg-card">
          <div className="flex flex-col gap-1.5">
            <label className="text-xs text-muted-foreground">WebDAV username</label>
            <Input
              placeholder="your@email.com"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              className="h-8 text-sm"
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <label className="text-xs text-muted-foreground">WebDAV password / secret</label>
            <Input
              type="password"
              placeholder="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="h-8 text-sm"
            />
          </div>
          <Button
            variant="rd"
            size="sm"
            disabled={busy || !username.trim() || !password.trim()}
            onClick={() =>
              run(
                () => invoke("webdav_setup", { username, password }),
                "Mounted at /mnt/RealDebrid"
              )
            }
          >
            {busy ? <Loader2 size={13} className="animate-spin" /> : null}
            Setup &amp; Mount
          </Button>
          <p className="text-xs text-muted-foreground">
            Get credentials at real-debrid.com - account - WebDAV password
          </p>
        </div>
      )}

      {configured && (
        <div className="flex gap-2 flex-wrap">
          {!status.service_active ? (
            <Button
              variant="outline"
              size="sm"
              disabled={busy}
              onClick={() => run(() => invoke("webdav_start"), "Service started")}
            >
              <Play size={12} /> Start
            </Button>
          ) : (
            <Button
              variant="outline"
              size="sm"
              disabled={busy}
              onClick={() => run(() => invoke("webdav_stop"), "Service stopped")}
            >
              <Square size={12} /> Stop
            </Button>
          )}
          <Button
            variant="outline"
            size="sm"
            disabled={busy}
            className="text-destructive hover:text-destructive"
            onClick={() => run(() => invoke("webdav_uninstall"), "Uninstalled")}
          >
            <Trash2 size={12} /> Uninstall
          </Button>
        </div>
      )}

      {msg && (
        <p className="text-xs text-rd-green">{msg}</p>
      )}
      {err && (
        <p className="text-xs text-destructive-foreground bg-destructive/20 border border-destructive/30 rounded px-2 py-1">
          {err}
        </p>
      )}
    </section>
  );
}

interface AppSettings {
  threads_per_download: number;
  max_concurrent_downloads: number;
  download_dir: string;
  quiet_hours_enabled: boolean;
  quiet_hours_start?: string;
  quiet_hours_end?: string;
}

export default function SettingsPage() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    invoke<AppSettings>("get_settings").then(setSettings).catch(console.error);
  }, []);

  async function handleSave() {
    if (!settings) return;
    setSaving(true);
    try {
      await invoke("save_settings_cmd", { newSettings: settings });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      console.error(e);
    } finally {
      setSaving(false);
    }
  }

  async function pickDir() {
    const path = await open({ directory: true });
    if (typeof path === "string") {
      setSettings((s) => s ? { ...s, download_dir: path } : s);
    }
  }

  if (!settings) {
    return (
      <div className="flex items-center gap-2 text-muted-foreground">
        <Loader2 size={16} className="animate-spin" />
        Loading settings…
      </div>
    );
  }

  function set<K extends keyof AppSettings>(key: K, val: AppSettings[K]) {
    setSettings((s) => s ? { ...s, [key]: val } : s);
  }

  return (
    <div className="flex flex-col gap-6 max-w-lg">
      <div>
        <h1 className="text-xl font-semibold">Settings</h1>
        <p className="text-sm text-muted-foreground mt-1">Configure download behavior.</p>
      </div>

      <div className="flex flex-col gap-5">
        <section className="flex flex-col gap-3">
          <h2 className="text-sm font-semibold text-muted-foreground uppercase tracking-wider">
            Downloads
          </h2>

          <div className="flex flex-col gap-1.5">
            <label className="text-sm font-medium">Threads per download</label>
            <p className="text-xs text-muted-foreground">
              Number of parallel chunks per file (1-16)
            </p>
            <div className="flex items-center gap-3">
              <input
                type="range"
                min={1}
                max={16}
                value={settings.threads_per_download}
                onChange={(e) => set("threads_per_download", Number(e.target.value))}
                className="flex-1 accent-[oklch(0.723_0.219_149.579)]"
              />
              <span className="text-sm font-mono w-6 text-center">
                {settings.threads_per_download}
              </span>
            </div>
          </div>

          <div className="flex flex-col gap-1.5">
            <label className="text-sm font-medium">Max concurrent downloads</label>
            <p className="text-xs text-muted-foreground">
              How many files download simultaneously (1-10)
            </p>
            <div className="flex items-center gap-3">
              <input
                type="range"
                min={1}
                max={10}
                value={settings.max_concurrent_downloads}
                onChange={(e) => set("max_concurrent_downloads", Number(e.target.value))}
                className="flex-1 accent-[oklch(0.723_0.219_149.579)]"
              />
              <span className="text-sm font-mono w-6 text-center">
                {settings.max_concurrent_downloads}
              </span>
            </div>
          </div>

          <div className="flex flex-col gap-1.5">
            <label className="text-sm font-medium">Download directory</label>
            <div className="flex gap-2">
              <Input
                value={settings.download_dir}
                onChange={(e) => set("download_dir", e.target.value)}
                className="font-mono text-xs"
              />
              <Button variant="outline" size="icon" onClick={pickDir}>
                <FolderOpen size={14} />
              </Button>
            </div>
          </div>
        </section>

        <section className="flex flex-col gap-3">
          <h2 className="text-sm font-semibold text-muted-foreground uppercase tracking-wider">
            Quiet Hours
          </h2>
          <p className="text-xs text-muted-foreground">
            Pause all downloads during the specified time window.
          </p>

          <div className="flex items-center gap-3">
            <input
              type="checkbox"
              id="qh"
              checked={settings.quiet_hours_enabled}
              onChange={(e) => set("quiet_hours_enabled", e.target.checked)}
              className="accent-[oklch(0.723_0.219_149.579)] h-4 w-4"
            />
            <label htmlFor="qh" className="text-sm">Enable quiet hours</label>
          </div>

          {settings.quiet_hours_enabled && (
            <div className="flex items-center gap-3">
              <div className="flex flex-col gap-1">
                <label className="text-xs text-muted-foreground">Start</label>
                <Input
                  type="time"
                  value={settings.quiet_hours_start ?? ""}
                  onChange={(e) => set("quiet_hours_start", e.target.value || undefined)}
                  className="w-32"
                />
              </div>
              <div className="flex flex-col gap-1">
                <label className="text-xs text-muted-foreground">End</label>
                <Input
                  type="time"
                  value={settings.quiet_hours_end ?? ""}
                  onChange={(e) => set("quiet_hours_end", e.target.value || undefined)}
                  className="w-32"
                />
              </div>
            </div>
          )}
        </section>

        <WebDavSection />
      </div>

      <Button variant="rd" onClick={handleSave} disabled={saving} className="w-fit">
        {saving ? <Loader2 size={14} className="animate-spin" /> : saved ? <Check size={14} /> : null}
        {saved ? "Saved!" : "Save Settings"}
      </Button>
    </div>
  );
}

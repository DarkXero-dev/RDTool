use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct ProgressEvent {
    pub id: String,
    pub bytes_done: u64,
    pub total_bytes: Option<u64>,
    pub speed_bps: u64,
}

#[derive(Clone)]
pub struct CompleteEvent {
    pub id: String,
    pub path: String,
    pub bytes_done: u64,
}

#[derive(Clone)]
pub struct ErrorEvent {
    pub id: String,
    pub error: String,
}

pub enum DownloadEvent {
    Progress(ProgressEvent),
    Complete(CompleteEvent),
    Error(ErrorEvent),
}

/// Extract `user:pass` credentials encoded in a URL and return `(clean_url, Option<(user, pass)>)`.
/// Handles percent-encoded `@` (%40) and `:` (%3A) in credentials.
fn parse_url_credentials(url: &str) -> (String, Option<(String, Option<String>)>) {
    let scheme_end = match url.find("://") {
        Some(i) => i + 3,
        None => return (url.to_string(), None),
    };
    let scheme = &url[..scheme_end];
    let rest = &url[scheme_end..];

    // Find `@` before the first `/` (host portion only)
    let path_start = rest.find('/').unwrap_or(rest.len());
    let host_part = &rest[..path_start];

    if let Some(at) = host_part.rfind('@') {
        let creds = &host_part[..at];
        let host_and_path = &rest[at + 1..];
        let clean_url = format!("{}{}", scheme, host_and_path);

        let decode = |s: &str| s.replace("%40", "@").replace("%3A", ":").replace("%25", "%");
        let (user, pass) = if let Some(colon) = creds.find(':') {
            (decode(&creds[..colon]), Some(decode(&creds[colon + 1..])))
        } else {
            (decode(creds), None)
        };
        (clean_url, Some((user, pass)))
    } else {
        (url.to_string(), None)
    }
}

pub async fn download_file(
    tx: Sender<DownloadEvent>,
    id: String,
    url: String,
    dest_path: String,
    threads: u8,
) -> Result<()> {
    // Handle local file:// URLs
    if url.starts_with("file://") {
        let src = url.trim_start_matches("file://");
        return local_copy_download(tx, id, src, &dest_path).await;
    }

    // WebDAV files: use rclone which handles caching/streaming natively
    if url.starts_with("webdav:") {
        let rel = url.trim_start_matches("webdav:").trim_start_matches('/');
        let rclone_src = format!("realdebrid:{}", rel);
        return rclone_copy_download(tx, id, rclone_src, dest_path).await;
    }

    let (url, basic_auth) = parse_url_credentials(&url);

    let client = Client::builder()
        .user_agent("Mozilla/5.0 (compatible; RDTool/0.1)")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;

    eprintln!("[download] HEAD {url}");

    let head_req = {
        let r = client.head(&url);
        match &basic_auth {
            Some((u, p)) => r.basic_auth(u, p.as_deref()),
            None => r,
        }
    };

    let total = match head_req.send().await {
        Ok(resp) => {
            eprintln!("[download] HEAD status: {}", resp.status());
            if resp.status().is_success() {
                resp.headers()
                    .get("content-length")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
            } else {
                None
            }
        }
        Err(e) => {
            eprintln!("[download] HEAD failed: {e} - continuing without content-length");
            None
        }
    };
    eprintln!("[download] content-length: {total:?}");

    if let Some(parent) = Path::new(&dest_path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let bytes_done = if total.map(|t| t > 0).unwrap_or(false) && threads > 1 {
        multi_thread_download(&tx, &client, &id, &url, &dest_path, total.unwrap(), threads, basic_auth.as_ref()).await?
    } else {
        single_thread_download(&tx, &client, &id, &url, &dest_path, total, basic_auth.as_ref()).await?
    };

    eprintln!("[download] done: {bytes_done} bytes written");

    if bytes_done == 0 {
        // Remove the empty file if created
        tokio::fs::remove_file(&dest_path).await.ok();
        return Err(anyhow!("download returned 0 bytes - URL may be invalid or expired"));
    }

    let _ = tx.send(DownloadEvent::Complete(CompleteEvent {
        id: id.clone(),
        path: dest_path,
        bytes_done,
    }));

    Ok(())
}

async fn rclone_copy_download(
    tx: Sender<DownloadEvent>,
    id: String,
    rclone_src: String,
    dest_path: String,
) -> Result<()> {
    if let Some(parent) = Path::new(&dest_path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Get file size for progress reporting (best-effort)
    let total: u64 = {
        let out = tokio::process::Command::new("rclone")
            .args(["size", "--json", &rclone_src])
            .output()
            .await
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default();
        // rclone size --json outputs {"count":1,"bytes":12345}
        out.split('"')
            .zip(out.split('"').skip(1))
            .find(|(k, _)| *k == "bytes")
            .and_then(|(_, rest)| rest.trim_start_matches(':').trim().split([',', '}']).next())
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    };

    let mut child = tokio::process::Command::new("rclone")
        .args(["copyto", &rclone_src, &dest_path, "--no-check-dest"])
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("rclone not found: {e}"))?;

    // Poll destination file size for progress while rclone runs
    loop {
        match child.try_wait()? {
            Some(status) => {
                if !status.success() {
                    // Collect stderr for a meaningful error message
                    let stderr = if let Some(mut e) = child.stderr.take() {
                        use tokio::io::AsyncReadExt;
                        let mut s = String::new();
                        let _ = e.read_to_string(&mut s).await;
                        s.trim().to_string()
                    } else {
                        String::new()
                    };
                    let msg = if stderr.is_empty() {
                        format!("rclone exited with {status}")
                    } else {
                        format!("rclone: {stderr}")
                    };
                    return Err(anyhow!("{msg}"));
                }
                break;
            }
            None => {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                let bytes_done = tokio::fs::metadata(&dest_path).await
                    .map(|m| m.len()).unwrap_or(0);
                let _ = tx.send(DownloadEvent::Progress(ProgressEvent {
                    id: id.clone(),
                    bytes_done,
                    total_bytes: if total > 0 { Some(total) } else { None },
                    speed_bps: 0,
                }));
            }
        }
    }

    let bytes_done = tokio::fs::metadata(&dest_path).await
        .map(|m| m.len()).unwrap_or(0);

    if bytes_done == 0 {
        return Err(anyhow!("rclone copy produced empty file - check rclone config"));
    }

    let _ = tx.send(DownloadEvent::Complete(CompleteEvent {
        id: id.clone(),
        path: dest_path,
        bytes_done,
    }));
    Ok(())
}

async fn local_copy_download(
    tx: Sender<DownloadEvent>,
    id: String,
    src_path: &str,
    dest_path: &str,
) -> Result<()> {
    use tokio::io::AsyncReadExt;

    let total = tokio::fs::metadata(src_path).await
        .map(|m| m.len())
        .unwrap_or(0);

    if let Some(parent) = Path::new(dest_path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut src = tokio::fs::File::open(src_path).await
        .map_err(|e| anyhow!("Cannot open source: {e}"))?;
    let mut dst = tokio::fs::File::create(dest_path).await?;

    let mut bytes_done: u64 = 0;
    let mut last_percent: u64 = u64::MAX;
    let mut buf = vec![0u8; 512 * 1024];

    loop {
        let n = src.read(&mut buf).await?;
        if n == 0 { break; }
        tokio::io::AsyncWriteExt::write_all(&mut dst, &buf[..n]).await?;
        bytes_done += n as u64;

        if total > 0 {
            let pct = bytes_done * 100 / total;
            if pct != last_percent {
                last_percent = pct;
                let _ = tx.send(DownloadEvent::Progress(ProgressEvent {
                    id: id.clone(),
                    bytes_done,
                    total_bytes: Some(total),
                    speed_bps: 0,
                }));
            }
        }
    }
    tokio::io::AsyncWriteExt::flush(&mut dst).await?;

    let _ = tx.send(DownloadEvent::Complete(CompleteEvent {
        id: id.clone(),
        path: dest_path.to_string(),
        bytes_done,
    }));
    Ok(())
}

async fn single_thread_download(
    tx: &Sender<DownloadEvent>,
    client: &Client,
    id: &str,
    url: &str,
    dest_path: &str,
    total: Option<u64>,
    auth: Option<&(String, Option<String>)>,
) -> Result<u64> {
    eprintln!("[download] GET {url}");
    let req = client.get(url);
    let req = match auth {
        Some((u, p)) => req.basic_auth(u, p.as_deref()),
        None => req,
    };
    let resp = req.send().await?;
    eprintln!("[download] GET status: {}", resp.status());
    let mut resp = resp.error_for_status()?;

    let mut file = File::create(dest_path).await?;
    let mut bytes_done: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    let mut speed_bytes: u64 = 0;
    let mut last_percent: u64 = u64::MAX;

    while let Some(chunk) = resp.chunk().await? {
        file.write_all(&chunk).await?;
        bytes_done += chunk.len() as u64;
        speed_bytes += chunk.len() as u64;

        let should_emit = match total {
            Some(t) if t > 0 => {
                let pct = bytes_done * 100 / t;
                pct != last_percent
            }
            _ => last_emit.elapsed().as_millis() >= 500,
        };

        if should_emit {
            let elapsed = last_emit.elapsed();
            let speed_bps = if elapsed.as_secs_f64() > 0.0 {
                (speed_bytes as f64 / elapsed.as_secs_f64()) as u64
            } else { 0 };
            speed_bytes = 0;
            last_emit = std::time::Instant::now();
            if let Some(t) = total.filter(|&t| t > 0) {
                last_percent = bytes_done * 100 / t;
            }
            let _ = tx.send(DownloadEvent::Progress(ProgressEvent {
                id: id.to_string(),
                bytes_done,
                total_bytes: total,
                speed_bps,
            }));
        }
    }
    file.flush().await?;
    eprintln!("[download] single-thread complete: {bytes_done} bytes");
    Ok(bytes_done)
}

async fn multi_thread_download(
    tx: &Sender<DownloadEvent>,
    client: &Client,
    id: &str,
    url: &str,
    dest_path: &str,
    total: u64,
    threads: u8,
    auth: Option<&(String, Option<String>)>,
) -> Result<u64> {
    let n = threads as u64;
    let chunk_size = total / n;
    let sem = Arc::new(Semaphore::new(threads as usize));
    let bytes_counter = Arc::new(AtomicU64::new(0));

    let tmp_dir = format!("{dest_path}.parts");
    tokio::fs::create_dir_all(&tmp_dir).await?;

    // Clone auth for thread use
    let auth_owned: Option<(String, Option<String>)> = auth.cloned();

    let mut handles = Vec::new();
    for i in 0..n {
        let start = i * chunk_size;
        let end = if i == n - 1 { total - 1 } else { start + chunk_size - 1 };
        let client = client.clone();
        let url = url.to_string();
        let part_path = format!("{tmp_dir}/{i}");
        let permit = sem.clone().acquire_owned().await?;
        let tx_clone = tx.clone();
        let id_clone = id.to_string();
        let bytes_counter = bytes_counter.clone();
        let auth_clone = auth_owned.clone();

        handles.push(tokio::spawn(async move {
            let _permit = permit;
            download_chunk(&tx_clone, &client, &url, &part_path, start, end, &id_clone, total, &bytes_counter, auth_clone.as_ref()).await
        }));
    }

    for handle in handles {
        handle.await.context("chunk task panicked")??;
    }

    merge_parts(&tmp_dir, dest_path, n as usize).await?;
    tokio::fs::remove_dir_all(&tmp_dir).await.ok();
    Ok(total)
}

async fn download_chunk(
    tx: &Sender<DownloadEvent>,
    client: &Client,
    url: &str,
    part_path: &str,
    start: u64,
    end: u64,
    id: &str,
    total: u64,
    bytes_counter: &Arc<AtomicU64>,
    auth: Option<&(String, Option<String>)>,
) -> Result<()> {
    let range = format!("bytes={start}-{end}");
    let req = client.get(url).header("Range", range);
    let req = match auth {
        Some((u, p)) => req.basic_auth(u, p.as_deref()),
        None => req,
    };
    let mut resp = req.send().await?.error_for_status()?;

    let mut file = File::create(part_path).await?;
    let mut last_emit = std::time::Instant::now();
    let mut since_last: u64 = 0;
    let mut last_percent: u64 = u64::MAX;

    while let Some(chunk) = resp.chunk().await? {
        let n = chunk.len() as u64;
        file.write_all(&chunk).await?;
        bytes_counter.fetch_add(n, Ordering::Relaxed);
        since_last += n;

        let bytes_done = bytes_counter.load(Ordering::Relaxed);
        let pct = bytes_done * 100 / total;
        if pct != last_percent {
            last_percent = pct;
            let elapsed = last_emit.elapsed();
            let speed_bps = if elapsed.as_secs_f64() > 0.0 {
                (since_last as f64 / elapsed.as_secs_f64()) as u64
            } else { 0 };
            since_last = 0;
            last_emit = std::time::Instant::now();
            let _ = tx.send(DownloadEvent::Progress(ProgressEvent {
                id: id.to_string(),
                bytes_done,
                total_bytes: Some(total),
                speed_bps,
            }));
        }
    }
    file.flush().await?;

    let bytes_done = bytes_counter.load(Ordering::Relaxed);
    let _ = tx.send(DownloadEvent::Progress(ProgressEvent {
        id: id.to_string(),
        bytes_done,
        total_bytes: Some(total),
        speed_bps: 0,
    }));

    Ok(())
}

async fn merge_parts(tmp_dir: &str, dest_path: &str, n: usize) -> Result<()> {
    let mut dest = File::create(dest_path).await?;
    for i in 0..n {
        let part = format!("{tmp_dir}/{i}");
        let mut src = File::open(&part).await?;
        tokio::io::copy(&mut src, &mut dest).await?;
    }
    dest.flush().await?;
    Ok(())
}

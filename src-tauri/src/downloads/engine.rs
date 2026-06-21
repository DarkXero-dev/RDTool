use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
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

pub async fn download_file(
    tx: Sender<DownloadEvent>,
    id: String,
    url: String,
    dest_path: String,
    threads: u8,
) -> Result<()> {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (compatible; RDTool/0.1)")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;

    eprintln!("[download] HEAD {url}");

    let total = match client.head(&url).send().await {
        Ok(resp) => {
            eprintln!("[download] HEAD status: {}", resp.status());
            resp.headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
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
        multi_thread_download(&tx, &client, &id, &url, &dest_path, total.unwrap(), threads).await?
    } else {
        single_thread_download(&tx, &client, &id, &url, &dest_path, total).await?
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

async fn single_thread_download(
    tx: &Sender<DownloadEvent>,
    client: &Client,
    id: &str,
    url: &str,
    dest_path: &str,
    total: Option<u64>,
) -> Result<u64> {
    eprintln!("[download] GET {url}");
    let resp = client.get(url).send().await?;
    eprintln!("[download] GET status: {}", resp.status());
    let mut resp = resp.error_for_status()?;

    let mut file = File::create(dest_path).await?;
    let mut bytes_done: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    let mut speed_bytes: u64 = 0;

    while let Some(chunk) = resp.chunk().await? {
        file.write_all(&chunk).await?;
        bytes_done += chunk.len() as u64;
        speed_bytes += chunk.len() as u64;

        let elapsed = last_emit.elapsed();
        if elapsed.as_millis() >= 500 {
            let speed_bps = (speed_bytes as f64 / elapsed.as_secs_f64()) as u64;
            speed_bytes = 0;
            last_emit = std::time::Instant::now();
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
) -> Result<u64> {
    let n = threads as u64;
    let chunk_size = total / n;
    let sem = Arc::new(Semaphore::new(threads as usize));
    let bytes_counter = Arc::new(Mutex::new(0u64));

    let tmp_dir = format!("{dest_path}.parts");
    tokio::fs::create_dir_all(&tmp_dir).await?;

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

        handles.push(tokio::spawn(async move {
            let _permit = permit;
            download_chunk(&tx_clone, &client, &url, &part_path, start, end, &id_clone, total, bytes_counter).await
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
    bytes_counter: Arc<Mutex<u64>>,
) -> Result<()> {
    let range = format!("bytes={start}-{end}");
    let mut resp = client
        .get(url)
        .header("Range", range)
        .send()
        .await?
        .error_for_status()?;

    let mut file = File::create(part_path).await?;
    let mut chunk_bytes: u64 = 0;

    while let Some(chunk) = resp.chunk().await? {
        file.write_all(&chunk).await?;
        chunk_bytes += chunk.len() as u64;
    }
    file.flush().await?;

    let current = {
        let mut c = bytes_counter.lock().unwrap();
        *c += chunk_bytes;
        *c
    };

    let _ = tx.send(DownloadEvent::Progress(ProgressEvent {
        id: id.to_string(),
        bytes_done: current,
        total_bytes: Some(total),
        speed_bps: 0,
    }));

    Ok(())
}

async fn merge_parts(tmp_dir: &str, dest_path: &str, n: usize) -> Result<()> {
    let mut dest = File::create(dest_path).await?;
    for i in 0..n {
        let part = format!("{tmp_dir}/{i}");
        let data = tokio::fs::read(&part).await?;
        dest.write_all(&data).await?;
    }
    dest.flush().await?;
    Ok(())
}

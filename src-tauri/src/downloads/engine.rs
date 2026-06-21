use anyhow::{Context, Result};
use reqwest::Client;
use std::path::Path;
use std::sync::Arc;
use tauri::AppHandle;
use tauri::Emitter;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;

#[derive(serde::Serialize, Clone)]
pub struct ProgressEvent {
    pub id: String,
    pub bytes_done: u64,
    pub total_bytes: Option<u64>,
    pub speed_bps: u64,
    pub status: String,
}

#[derive(serde::Serialize, Clone)]
pub struct CompleteEvent {
    pub id: String,
    pub path: String,
}

#[derive(serde::Serialize, Clone)]
pub struct ErrorEvent {
    pub id: String,
    pub error: String,
}

pub async fn download_file(
    app: AppHandle,
    id: String,
    url: String,
    dest_path: String,
    threads: u8,
) -> Result<()> {
    let client = Client::builder()
        .user_agent("RDTool/0.1")
        .build()?;

    let head = client.head(&url).send().await?;
    let total = head
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    let dest = Path::new(&dest_path);
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    if total.is_none() || threads <= 1 {
        single_thread_download(&app, &client, &id, &url, &dest_path, total).await?;
    } else {
        multi_thread_download(&app, &client, &id, &url, &dest_path, total.unwrap(), threads).await?;
    }

    let _ = app.emit("download-complete", CompleteEvent {
        id: id.clone(),
        path: dest_path.clone(),
    });

    Ok(())
}

async fn single_thread_download(
    app: &AppHandle,
    client: &Client,
    id: &str,
    url: &str,
    dest_path: &str,
    total: Option<u64>,
) -> Result<()> {
    let mut resp = client.get(url).send().await?.error_for_status()?;
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
            let _ = app.emit("download-progress", ProgressEvent {
                id: id.to_string(),
                bytes_done,
                total_bytes: total,
                speed_bps,
                status: "active".to_string(),
            });
        }
    }
    file.flush().await?;
    Ok(())
}

async fn multi_thread_download(
    app: &AppHandle,
    client: &Client,
    id: &str,
    url: &str,
    dest_path: &str,
    total: u64,
    threads: u8,
) -> Result<()> {
    let n = threads as u64;
    let chunk_size = total / n;
    let sem = Arc::new(Semaphore::new(threads as usize));

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
        let app_clone = app.clone();
        let id_clone = id.to_string();

        handles.push(tokio::spawn(async move {
            let _permit = permit;
            let result = download_chunk(&client, &url, &part_path, start, end, &app_clone, &id_clone, total).await;
            result
        }));
    }

    for handle in handles {
        handle.await.context("chunk task panicked")??;
    }

    merge_parts(&tmp_dir, dest_path, n as usize).await?;
    tokio::fs::remove_dir_all(&tmp_dir).await.ok();
    Ok(())
}

async fn download_chunk(
    client: &Client,
    url: &str,
    part_path: &str,
    start: u64,
    end: u64,
    app: &AppHandle,
    id: &str,
    total: u64,
) -> Result<()> {
    let range = format!("bytes={start}-{end}");
    let mut resp = client
        .get(url)
        .header("Range", range)
        .send()
        .await?
        .error_for_status()?;

    let mut file = File::create(part_path).await?;
    let mut bytes: u64 = 0;

    while let Some(chunk) = resp.chunk().await? {
        file.write_all(&chunk).await?;
        bytes += chunk.len() as u64;
    }
    file.flush().await?;

    let _ = app.emit("download-progress", ProgressEvent {
        id: id.to_string(),
        bytes_done: bytes,
        total_bytes: Some(total),
        speed_bps: 0,
        status: "active".to_string(),
    });

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

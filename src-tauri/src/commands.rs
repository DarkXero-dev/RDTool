use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

use crate::api::client::build_client;
use crate::api::{downloader, streaming, torrents, user};
use crate::auth;
use crate::downloads::queue::{self, DownloadOpts, DownloadStatus, QueuedDownload};
use crate::downloads::engine;
use crate::settings::{self, AppSettings};

pub struct AppState {
    pub settings: Arc<Mutex<AppSettings>>,
    pub db_conn: Arc<Mutex<rusqlite::Connection>>,
}

// ---- Auth ----------------------------------------------------------------

#[tauri::command]
pub fn save_token(token: String) -> Result<(), String> {
    auth::save_token(&token).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_token() -> Result<String, String> {
    auth::load_token().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_token() -> Result<(), String> {
    auth::clear_token().map_err(|e| e.to_string())
}

// ---- Account -------------------------------------------------------------

#[tauri::command]
pub async fn get_user() -> Result<user::RdUser, String> {
    let token = auth::load_token().map_err(|e| e.to_string())?;
    let client = build_client(token).map_err(|e| e.to_string())?;
    user::get_user(&client).await.map_err(|e| e.to_string())
}

// ---- Downloader ----------------------------------------------------------

#[tauri::command]
pub async fn unrestrict_link(link: String) -> Result<downloader::UnrestrictedLink, String> {
    let token = auth::load_token().map_err(|e| e.to_string())?;
    let client = build_client(token).map_err(|e| e.to_string())?;
    downloader::unrestrict_link(&client, &link)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn unrestrict_links(
    links: Vec<String>,
) -> Result<Vec<downloader::UnrestrictedLink>, String> {
    let token = auth::load_token().map_err(|e| e.to_string())?;
    let client = build_client(token).map_err(|e| e.to_string())?;
    downloader::unrestrict_links(&client, links)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_links_to_txt(links: Vec<String>, path: String) -> Result<(), String> {
    let content = links.join("\n");
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

// ---- Torrents ------------------------------------------------------------

#[tauri::command]
pub async fn add_magnet(magnet: String) -> Result<torrents::TorrentAddResult, String> {
    let token = auth::load_token().map_err(|e| e.to_string())?;
    let client = build_client(token).map_err(|e| e.to_string())?;
    torrents::add_magnet(&client, &magnet)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_torrent_file(
    bytes: Vec<u8>,
    filename: String,
) -> Result<torrents::TorrentAddResult, String> {
    let token = auth::load_token().map_err(|e| e.to_string())?;
    let client = build_client(token).map_err(|e| e.to_string())?;
    torrents::add_torrent_file(&client, bytes, filename)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_torrents() -> Result<Vec<torrents::Torrent>, String> {
    let token = auth::load_token().map_err(|e| e.to_string())?;
    let client = build_client(token).map_err(|e| e.to_string())?;
    torrents::get_torrents(&client).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_torrent(id: String) -> Result<torrents::Torrent, String> {
    let token = auth::load_token().map_err(|e| e.to_string())?;
    let client = build_client(token).map_err(|e| e.to_string())?;
    torrents::get_torrent(&client, &id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn select_torrent_files(id: String, file_ids: Vec<u32>) -> Result<(), String> {
    let token = auth::load_token().map_err(|e| e.to_string())?;
    let client = build_client(token).map_err(|e| e.to_string())?;
    torrents::select_torrent_files(&client, &id, file_ids)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_torrent(id: String) -> Result<(), String> {
    let token = auth::load_token().map_err(|e| e.to_string())?;
    let client = build_client(token).map_err(|e| e.to_string())?;
    torrents::delete_torrent(&client, &id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_rd_downloads() -> Result<Vec<torrents::RdDownload>, String> {
    let token = auth::load_token().map_err(|e| e.to_string())?;
    let client = build_client(token).map_err(|e| e.to_string())?;
    torrents::get_rd_downloads(&client)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_rd_download(id: String) -> Result<(), String> {
    let token = auth::load_token().map_err(|e| e.to_string())?;
    let client = build_client(token).map_err(|e| e.to_string())?;
    torrents::delete_rd_download(&client, &id)
        .await
        .map_err(|e| e.to_string())
}

// ---- Streaming -----------------------------------------------------------

#[tauri::command]
pub async fn get_stream_transcodes(id: String) -> Result<streaming::StreamInfo, String> {
    let token = auth::load_token().map_err(|e| e.to_string())?;
    let client = build_client(token).map_err(|e| e.to_string())?;
    streaming::get_stream_transcodes(&client, &id)
        .await
        .map_err(|e| e.to_string())
}

// ---- Download manager ----------------------------------------------------

#[tauri::command]
pub fn enqueue_download(
    url: String,
    filename: String,
    opts: DownloadOpts,
    state: State<AppState>,
) -> Result<QueuedDownload, String> {
    let settings = state.settings.lock().unwrap().clone();
    let dest_path = format!("{}/{}", settings.download_dir, filename);
    let conn = state.db_conn.lock().unwrap();
    queue::enqueue(&conn, url, filename, dest_path, opts, settings.threads_per_download)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_queue(state: State<AppState>) -> Result<Vec<QueuedDownload>, String> {
    let conn = state.db_conn.lock().unwrap();
    queue::get_all(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_download(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let item = {
        let conn = state.db_conn.lock().unwrap();
        queue::get_all(&conn)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|d| d.id == id)
            .ok_or_else(|| "download not found".to_string())?
    };
    {
        let conn = state.db_conn.lock().unwrap();
        queue::update_status(&conn, &id, DownloadStatus::Active).map_err(|e| e.to_string())?;
    }
    let id_clone = id.clone();
    tokio::spawn(async move {
        if let Err(e) = engine::download_file(app.clone(), id_clone.clone(), item.url, item.dest_path, item.threads).await {
            let _ = app.emit("download-error", engine::ErrorEvent {
                id: id_clone,
                error: e.to_string(),
            });
        }
    });
    Ok(())
}

#[tauri::command]
pub fn pause_download(id: String, state: State<AppState>) -> Result<(), String> {
    let conn = state.db_conn.lock().unwrap();
    queue::update_status(&conn, &id, DownloadStatus::Paused).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cancel_download(id: String, state: State<AppState>) -> Result<(), String> {
    let conn = state.db_conn.lock().unwrap();
    queue::update_status(&conn, &id, DownloadStatus::Cancelled).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_download(id: String, state: State<AppState>) -> Result<(), String> {
    let conn = state.db_conn.lock().unwrap();
    queue::remove(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn schedule_download(id: String, at: String, state: State<AppState>) -> Result<(), String> {
    let conn = state.db_conn.lock().unwrap();
    queue::update_schedule(&conn, &id, &at).map_err(|e| e.to_string())
}

// ---- Settings ------------------------------------------------------------

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> AppSettings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_settings_cmd(new_settings: AppSettings, state: State<AppState>) -> Result<(), String> {
    settings::save_settings(&new_settings).map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap() = new_settings;
    Ok(())
}

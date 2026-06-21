mod api;
mod auth;
mod commands;
mod db;
mod downloads;
mod settings;

use commands::AppState;
use std::sync::{Arc, Mutex};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let conn = db::open().expect("failed to open database");
    let loaded_settings = settings::load_settings();
    let settings = Arc::new(Mutex::new(loaded_settings));

    let state = AppState {
        settings: settings.clone(),
        db_conn: Arc::new(Mutex::new(conn)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(state)
        .setup(move |_app| {
            // Tokio runtime is live here - safe to spawn async tasks
            downloads::scheduler::start(settings);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::save_token,
            commands::load_token,
            commands::clear_token,
            commands::get_user,
            commands::unrestrict_link,
            commands::unrestrict_links,
            commands::export_links_to_txt,
            commands::add_magnet,
            commands::add_torrent_file,
            commands::get_torrents,
            commands::get_torrent,
            commands::select_torrent_files,
            commands::delete_torrent,
            commands::get_rd_downloads,
            commands::delete_rd_download,
            commands::get_stream_transcodes,
            commands::enqueue_download,
            commands::get_queue,
            commands::start_download,
            commands::pause_download,
            commands::cancel_download,
            commands::remove_download,
            commands::schedule_download,
            commands::get_settings,
            commands::save_settings_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

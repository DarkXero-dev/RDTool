mod api;
mod auth;
mod commands;
mod db;
mod downloads;
mod settings;
mod webdav;

use commands::AppState;
use std::sync::{Arc, Mutex};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // WebKit EGL crashes on Wayland without this - must be set before the app initializes
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
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
            webdav::webdav_status,
            webdav::webdav_setup,
            webdav::webdav_start,
            webdav::webdav_stop,
            webdav::webdav_uninstall,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

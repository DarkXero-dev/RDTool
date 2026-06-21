use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use arboard;

use egui::{RichText, Ui};

use crate::api::{client::build_client, downloader, streaming, torrents, user};
use crate::auth;
use crate::downloads::{
    engine::{self, DownloadEvent},
    queue::{self, DownloadOpts, DownloadStatus, QueuedDownload},
};
use crate::settings::{self, AppSettings};
use crate::webdav;

use super::theme;

fn make_tray_icon() -> tray_icon::Icon {
    let size = 32u32;
    let mut px = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - 16.0;
            let dy = y as f32 - 16.0;
            if (dx * dx + dy * dy).sqrt() <= 14.0 {
                let i = ((y * size + x) * 4) as usize;
                px[i]     = 74;
                px[i + 1] = 222;
                px[i + 2] = 128;
                px[i + 3] = 255;
            }
        }
    }
    tray_icon::Icon::from_rgba(px, size, size).expect("tray icon")
}

fn build_tray() -> Option<tray_icon::TrayIcon> {
    use tray_icon::menu::{Menu, MenuItem};
    let menu = Menu::new();
    let show = MenuItem::with_id("show", "Show App", true, None);
    let quit = MenuItem::with_id("quit", "Quit App", true, None);
    menu.append(&show).ok()?;
    menu.append(&quit).ok()?;
    tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("RDTool")
        .with_icon(make_tray_icon())
        .build()
        .ok()
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(PartialEq, Clone, Copy)]
enum Page {
    Dashboard,
    Downloader,
    Torrents,
    Streaming,
    Downloads,
    Settings,
}

pub enum AppEvent {
    LoginDone(Result<user::RdUser, String>),
    UserRefreshed(Result<user::RdUser, String>),
    LinksUnrestricted(Result<Vec<downloader::UnrestrictedLink>, String>),
    TorrentAdded(Result<torrents::TorrentAddResult, String>),
    TorrentFilesReady(Result<(String, Vec<torrents::TorrentFile>), String>),
    FilesSelected(Result<(), String>),
    TorrentDeleted(Result<(), String>),
    TorrentLinksUnrestricted(Result<Vec<downloader::UnrestrictedLink>, String>),
    RdDownloadDeleted(Result<(), String>),
    TorrentSelectionUpdate(String),
    TorrentsLoaded(Result<Vec<torrents::Torrent>, String>),
    RdDownloadsLoaded(Result<Vec<torrents::RdDownload>, String>),
    StreamInfoLoaded(Result<streaming::StreamInfo, String>),
    QueueRefreshed(Vec<QueuedDownload>),
    SettingsSaved(Result<(), String>),
    WebDavStatus(webdav::WebDavStatus),
    WebDavDone(Result<String, String>),
    UpdateAvailable(Option<String>),
}

pub struct RdApp {
    handle: tokio::runtime::Handle,
    settings: Arc<Mutex<AppSettings>>,
    db_conn: Arc<Mutex<rusqlite::Connection>>,

    ev_tx: std::sync::mpsc::Sender<AppEvent>,
    ev_rx: std::sync::mpsc::Receiver<AppEvent>,
    dl_tx: std::sync::mpsc::Sender<DownloadEvent>,
    dl_rx: std::sync::mpsc::Receiver<DownloadEvent>,

    logged_in: bool,
    page: Page,
    token_input: String,
    login_loading: bool,
    login_error: Option<String>,

    user: Option<user::RdUser>,
    update_available: Option<String>,

    // downloader page
    dl_input: String,
    dl_results: Vec<downloader::UnrestrictedLink>,
    dl_loading: bool,
    app_error: Option<String>,

    // torrents page
    torrent_magnet: String,
    torrents: Vec<torrents::Torrent>,
    torrents_loading: bool,
    rd_downloads: Vec<torrents::RdDownload>,
    rd_tab: TorrentTab,
    torrent_pending_files: Option<(String, Vec<torrents::TorrentFile>)>,
    torrent_file_selection: Vec<bool>,
    torrent_selecting: bool,
    torrent_selection_status: String,
    last_torrent_refresh: Option<std::time::Instant>,

    // streaming page
    stream_input: String,
    stream_info: Option<streaming::StreamInfo>,
    stream_loading: bool,
    // downloads queue
    queue: Vec<QueuedDownload>,
    dl_progress: HashMap<String, engine::ProgressEvent>,
    delete_confirm: Option<(String, Option<String>)>, // (item_id, dest_path)

    // settings page
    settings_edit: Option<AppSettings>,
    settings_saving: bool,
    settings_saved: bool,

    // tray
    tray_icon: Option<tray_icon::TrayIcon>,
    show_close_dialog: bool,
    force_quit: bool,

    // webdav
    webdav_status: Option<webdav::WebDavStatus>,
    webdav_username: String,
    webdav_password: String,
    webdav_busy: bool,
    webdav_msg: Option<String>,
}

#[derive(PartialEq)]
enum TorrentTab {
    Torrents,
    Downloads,
}

impl RdApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        handle: tokio::runtime::Handle,
        settings: Arc<Mutex<AppSettings>>,
        db_conn: Arc<Mutex<rusqlite::Connection>>,
        ev_tx: std::sync::mpsc::Sender<AppEvent>,
        ev_rx: std::sync::mpsc::Receiver<AppEvent>,
        dl_tx: std::sync::mpsc::Sender<DownloadEvent>,
        dl_rx: std::sync::mpsc::Receiver<DownloadEvent>,
    ) -> Self {
        let prefer_dark = dark_light::detect() != dark_light::Mode::Light;
        theme::apply(&cc.egui_ctx, prefer_dark);

        let logged_in = auth::load_token().is_ok();

        let mut app = Self {
            handle,
            settings,
            db_conn,
            ev_tx,
            ev_rx,
            dl_tx,
            dl_rx,
            logged_in,
            page: Page::Dashboard,
            token_input: String::new(),
            login_loading: false,
            login_error: None,
            user: None,
            update_available: None,
            dl_input: String::new(),
            dl_results: Vec::new(),
            dl_loading: false,
            app_error: None,
            torrent_magnet: String::new(),
            torrents: Vec::new(),
            torrents_loading: false,
            rd_downloads: Vec::new(),
            rd_tab: TorrentTab::Torrents,
            torrent_pending_files: None,
            torrent_file_selection: Vec::new(),
            torrent_selecting: false,
            torrent_selection_status: String::new(),
            last_torrent_refresh: None,
            stream_input: String::new(),
            stream_info: None,
            stream_loading: false,
            queue: Vec::new(),
            dl_progress: HashMap::new(),
            delete_confirm: None,
            settings_edit: None,
            settings_saving: false,
            settings_saved: false,
            tray_icon: None,
            show_close_dialog: false,
            force_quit: false,
            webdav_status: None,
            webdav_username: String::new(),
            webdav_password: String::new(),
            webdav_busy: false,
            webdav_msg: None,
        };

        if logged_in {
            app.fetch_user(cc.egui_ctx.clone(), false);
            app.check_update(cc.egui_ctx.clone());
            app.refresh_queue();
        }

        let tray_on = app.settings.lock().unwrap().tray_enabled;
        if tray_on {
            app.tray_icon = build_tray();
        }

        app
    }

    fn poll_events(&mut self) {
        while let Ok(ev) = self.ev_rx.try_recv() {
            match ev {
                AppEvent::LoginDone(Ok(u)) => {
                    self.user = Some(u);
                    self.logged_in = true;
                    self.login_loading = false;
                    self.login_error = None;
                    self.refresh_queue();
                }
                AppEvent::LoginDone(Err(e)) => {
                    self.login_error = Some(e);
                    self.login_loading = false;
                }
                AppEvent::UserRefreshed(Ok(u)) => {
                    self.user = Some(u);
                }
                AppEvent::UserRefreshed(Err(_)) => {}
                AppEvent::LinksUnrestricted(Ok(links)) => {
                    self.dl_results = links;
                    self.dl_loading = false;
                }
                AppEvent::LinksUnrestricted(Err(e)) => {
                    self.app_error = Some(e);
                    self.dl_loading = false;
                }
                AppEvent::TorrentAdded(Ok(result)) => {
                    self.torrent_magnet.clear();
                    self.torrent_selecting = true;
                    self.torrent_selection_status = "Waiting...".to_string();
                    self.fetch_torrent_for_selection(result.id, egui::Context::default());
                }
                AppEvent::TorrentAdded(Err(e)) => {
                    self.app_error = Some(e);
                }
                AppEvent::TorrentFilesReady(Ok((id, files))) => {
                    self.torrent_file_selection = vec![true; files.len()];
                    self.torrent_pending_files = Some((id, files));
                    self.torrent_selecting = false;
                }
                AppEvent::TorrentFilesReady(Err(e)) => {
                    self.app_error = Some(e);
                    self.torrent_selecting = false;
                    self.load_torrents(egui::Context::default());
                }
                AppEvent::FilesSelected(Ok(())) => {
                    self.torrent_pending_files = None;
                    self.load_torrents(egui::Context::default());
                }
                AppEvent::FilesSelected(Err(e)) => {
                    self.app_error = Some(e);
                }
                AppEvent::TorrentDeleted(Ok(())) => {
                    self.load_torrents(egui::Context::default());
                }
                AppEvent::TorrentDeleted(Err(_)) => {}
                AppEvent::RdDownloadDeleted(Ok(())) => {
                    self.load_rd_downloads(egui::Context::default());
                }
                AppEvent::RdDownloadDeleted(Err(_)) => {}
                AppEvent::TorrentSelectionUpdate(status) => {
                    self.torrent_selection_status = status;
                }
                AppEvent::TorrentLinksUnrestricted(Ok(links)) => {
                    for link in links {
                        self.enqueue_download(link.download, link.filename);
                    }
                    self.page = Page::Downloads;
                }
                AppEvent::TorrentLinksUnrestricted(Err(e)) => {
                    self.app_error = Some(format!("Failed to unrestrict links: {}", e));
                }
                AppEvent::TorrentsLoaded(Ok(list)) => {
                    self.torrents = list;
                    self.torrents_loading = false;
                }
                AppEvent::TorrentsLoaded(Err(e)) => {
                    self.app_error = Some(e);
                    self.torrents_loading = false;
                }
                AppEvent::RdDownloadsLoaded(Ok(list)) => {
                    self.rd_downloads = list;
                }
                AppEvent::RdDownloadsLoaded(Err(_)) => {}
                AppEvent::StreamInfoLoaded(Ok(info)) => {
                    self.stream_info = Some(info);
                    self.stream_loading = false;
                }
                AppEvent::StreamInfoLoaded(Err(e)) => {
                    self.app_error = Some(e);
                    self.stream_loading = false;
                }
                AppEvent::QueueRefreshed(items) => {
                    self.queue = items;
                }
                AppEvent::SettingsSaved(Ok(())) => {
                    self.settings_saving = false;
                    self.settings_saved = true;
                }
                AppEvent::SettingsSaved(Err(_)) => {
                    self.settings_saving = false;
                }
                AppEvent::WebDavStatus(s) => {
                    self.webdav_status = Some(s);
                    self.webdav_busy = false;
                }
                AppEvent::WebDavDone(Ok(msg)) => {
                    self.webdav_msg = Some(msg);
                    self.webdav_busy = false;
                    self.load_webdav_status(egui::Context::default());
                }
                AppEvent::WebDavDone(Err(e)) => {
                    self.app_error = Some(e);
                    self.webdav_msg = None;
                    self.webdav_busy = false;
                }
                AppEvent::UpdateAvailable(v) => {
                    self.update_available = v;
                }
            }
        }

        while let Ok(ev) = self.dl_rx.try_recv() {
            match ev {
                DownloadEvent::Progress(p) => {
                    if let Ok(conn) = self.db_conn.lock() {
                        let _ = queue::update_progress(&conn, &p.id, p.bytes_done, p.total_bytes);
                    }
                    self.dl_progress.insert(p.id.clone(), p);
                }
                DownloadEvent::Complete(c) => {
                    self.dl_progress.remove(&c.id);
                    if let Ok(conn) = self.db_conn.lock() {
                        let _ = queue::update_progress(&conn, &c.id, c.bytes_done, None);
                        let _ = queue::update_status(&conn, &c.id, DownloadStatus::Completed);
                    }
                    self.refresh_queue();
                }
                DownloadEvent::Error(e) => {
                    self.dl_progress.remove(&e.id);
                    if let Ok(conn) = self.db_conn.lock() {
                        let _ = queue::set_error(&conn, &e.id, &e.error);
                    }
                    self.refresh_queue();
                }
            }
        }
    }

    // ---- async operations ----

    fn fetch_user(&self, ctx: egui::Context, is_login: bool) {
        let tx = self.ev_tx.clone();
        let token = match auth::load_token() {
            Ok(t) => t,
            Err(e) => {
                if is_login {
                    let _ = tx.send(AppEvent::LoginDone(Err(e.to_string())));
                }
                return;
            }
        };
        self.handle.spawn(async move {
            let result = (async {
                let client = build_client(token).map_err(|e| e.to_string())?;
                user::get_user(&client).await.map_err(|e| e.to_string())
            })
            .await;
            let ev = if is_login {
                AppEvent::LoginDone(result)
            } else {
                AppEvent::UserRefreshed(result)
            };
            let _ = tx.send(ev);
            ctx.request_repaint();
        });
    }

    fn do_login(&mut self, ctx: egui::Context) {
        let token = self.token_input.trim().to_string();
        if token.is_empty() {
            return;
        }
        self.login_loading = true;
        self.login_error = None;
        if let Err(e) = auth::save_token(&token) {
            self.login_error = Some(e.to_string());
            self.login_loading = false;
            return;
        }
        self.fetch_user(ctx, true);
    }

    fn check_update(&self, ctx: egui::Context) {
        let tx = self.ev_tx.clone();
        let current = VERSION.to_string();
        self.handle.spawn(async move {
            let result = check_latest_version(&current).await;
            let _ = tx.send(AppEvent::UpdateAvailable(result));
            ctx.request_repaint();
        });
    }

    fn unrestrict_links(&mut self, ctx: egui::Context) {
        let links: Vec<String> = self
            .dl_input
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        if links.is_empty() {
            return;
        }
        self.dl_loading = true;
        self.dl_results.clear();
        let tx = self.ev_tx.clone();
        let token = match auth::load_token() {
            Ok(t) => t,
            Err(e) => {
                self.app_error = Some(e.to_string());
                self.dl_loading = false;
                return;
            }
        };
        self.handle.spawn(async move {
            let result = (async {
                let client = build_client(token).map_err(|e| e.to_string())?;
                downloader::unrestrict_links(&client, links)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await;
            let _ = tx.send(AppEvent::LinksUnrestricted(result));
            ctx.request_repaint();
        });
    }

    fn add_magnet(&mut self, ctx: egui::Context) {
        let magnet = self.torrent_magnet.trim().to_string();
        if magnet.is_empty() {
            return;
        }
        let tx = self.ev_tx.clone();
        let token = match auth::load_token() {
            Ok(t) => t,
            Err(e) => {
                self.app_error = Some(e.to_string());
                return;
            }
        };
        self.handle.spawn(async move {
            let result = (async {
                let client = build_client(token).map_err(|e| e.to_string())?;
                torrents::add_magnet(&client, &magnet)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await;
            let _ = tx.send(AppEvent::TorrentAdded(result));
            ctx.request_repaint();
        });
    }

    fn add_torrent_file(&mut self, bytes: Vec<u8>, filename: String, ctx: egui::Context) {
        let tx = self.ev_tx.clone();
        let token = match auth::load_token() {
            Ok(t) => t,
            Err(e) => {
                self.app_error = Some(e.to_string());
                return;
            }
        };
        self.handle.spawn(async move {
            let result = (async {
                let client = build_client(token).map_err(|e| e.to_string())?;
                torrents::add_torrent_file(&client, bytes, filename)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await;
            let _ = tx.send(AppEvent::TorrentAdded(result));
            ctx.request_repaint();
        });
    }

    fn fetch_torrent_for_selection(&self, torrent_id: String, ctx: egui::Context) {
        let tx = self.ev_tx.clone();
        let token = match auth::load_token() {
            Ok(t) => t,
            Err(e) => {
                let _ = tx.send(AppEvent::TorrentFilesReady(Err(e.to_string())));
                return;
            }
        };
        self.handle.spawn(async move {
            let result = (async {
                let client = build_client(token).map_err(|e| e.to_string())?;
                // Poll until status is waiting_files_selection (up to 5 minutes)
                for i in 0..150 {
                    let info = torrents::get_torrent(&client, &torrent_id)
                        .await
                        .map_err(|e| e.to_string())?;
                    let elapsed = i * 2;
                    let status_label = match info.status.as_str() {
                        "magnet_conversion" => format!("Converting magnet... ({}s)", elapsed),
                        "waiting_files_selection" => "Ready - loading files...".to_string(),
                        s => format!("{} ({}s)", s, elapsed),
                    };
                    let _ = tx.send(AppEvent::TorrentSelectionUpdate(status_label));
                    ctx.request_repaint();
                    if info.status == "waiting_files_selection" {
                        let files = info.files.unwrap_or_default();
                        return Ok((torrent_id.clone(), files));
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
                Err("Timeout: torrent did not reach selection state within 5 minutes".to_string())
            })
            .await;
            let _ = tx.send(AppEvent::TorrentFilesReady(result));
            ctx.request_repaint();
        });
    }

    fn select_files(&self, torrent_id: String, file_ids: Vec<u32>, ctx: egui::Context) {
        let tx = self.ev_tx.clone();
        let token = match auth::load_token() {
            Ok(t) => t,
            Err(e) => {
                let _ = tx.send(AppEvent::FilesSelected(Err(e.to_string())));
                return;
            }
        };
        self.handle.spawn(async move {
            let result = (async {
                let client = build_client(token).map_err(|e| e.to_string())?;
                torrents::select_torrent_files(&client, &torrent_id, file_ids)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await;
            let _ = tx.send(AppEvent::FilesSelected(result));
            ctx.request_repaint();
        });
    }

    fn unrestrict_and_enqueue_torrent(&self, links: Vec<String>, ctx: egui::Context) {
        let tx = self.ev_tx.clone();
        let token = match auth::load_token() {
            Ok(t) => t,
            Err(e) => {
                let _ = tx.send(AppEvent::TorrentLinksUnrestricted(Err(e.to_string())));
                return;
            }
        };
        self.handle.spawn(async move {
            let result = (async {
                let client = build_client(token).map_err(|e| e.to_string())?;
                downloader::unrestrict_links(&client, links)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await;
            let _ = tx.send(AppEvent::TorrentLinksUnrestricted(result));
            ctx.request_repaint();
        });
    }

    fn delete_torrent_async(&self, torrent_id: String, ctx: egui::Context) {
        let tx = self.ev_tx.clone();
        let token = match auth::load_token() {
            Ok(t) => t,
            Err(e) => {
                let _ = tx.send(AppEvent::TorrentDeleted(Err(e.to_string())));
                return;
            }
        };
        self.handle.spawn(async move {
            let result = (async {
                let client = build_client(token).map_err(|e| e.to_string())?;
                torrents::delete_torrent(&client, &torrent_id)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await;
            let _ = tx.send(AppEvent::TorrentDeleted(result));
            ctx.request_repaint();
        });
    }

    fn delete_rd_download_async(&self, download_id: String, ctx: egui::Context) {
        let tx = self.ev_tx.clone();
        let token = match auth::load_token() {
            Ok(t) => t,
            Err(e) => {
                let _ = tx.send(AppEvent::RdDownloadDeleted(Err(e.to_string())));
                return;
            }
        };
        self.handle.spawn(async move {
            let result = (async {
                let client = build_client(token).map_err(|e| e.to_string())?;
                torrents::delete_rd_download(&client, &download_id)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await;
            let _ = tx.send(AppEvent::RdDownloadDeleted(result));
            ctx.request_repaint();
        });
    }

    fn load_torrents(&self, ctx: egui::Context) {
        let tx = self.ev_tx.clone();
        let token = match auth::load_token() {
            Ok(t) => t,
            Err(_) => return,
        };
        self.handle.spawn(async move {
            let result = (async {
                let client = build_client(token).map_err(|e| e.to_string())?;
                torrents::get_torrents(&client)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await;
            let _ = tx.send(AppEvent::TorrentsLoaded(result));
            ctx.request_repaint();
        });
    }

    fn load_rd_downloads(&self, ctx: egui::Context) {
        let tx = self.ev_tx.clone();
        let token = match auth::load_token() {
            Ok(t) => t,
            Err(_) => return,
        };
        self.handle.spawn(async move {
            let result = (async {
                let client = build_client(token).map_err(|e| e.to_string())?;
                torrents::get_rd_downloads(&client)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await;
            let _ = tx.send(AppEvent::RdDownloadsLoaded(result));
            ctx.request_repaint();
        });
    }

    fn get_stream_info(&mut self, ctx: egui::Context) {
        let id = self.stream_input.trim().to_string();
        if id.is_empty() {
            return;
        }
        self.stream_loading = true;
        self.stream_info = None;
        let tx = self.ev_tx.clone();
        let token = match auth::load_token() {
            Ok(t) => t,
            Err(e) => {
                self.app_error = Some(e.to_string());
                self.stream_loading = false;
                return;
            }
        };
        self.handle.spawn(async move {
            let result = (async {
                let client = build_client(token).map_err(|e| e.to_string())?;
                streaming::get_stream_transcodes(&client, &id)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await;
            let _ = tx.send(AppEvent::StreamInfoLoaded(result));
            ctx.request_repaint();
        });
    }

    fn refresh_queue(&self) {
        if let Ok(conn) = self.db_conn.lock() {
            if let Ok(items) = queue::get_all(&conn) {
                let _ = self.ev_tx.send(AppEvent::QueueRefreshed(items));
            }
        }
    }

    fn enqueue_download(&mut self, url: String, filename: String) {
        let (dest_path, threads) = {
            let s = self.settings.lock().unwrap();
            let dest = format!("{}/{}", s.download_dir, filename);
            let t = s.threads_per_download;
            (dest, t)
        };
        let opts = DownloadOpts {
            threads: None,
            scheduled_at: None,
            priority: 0,
        };
        if let Ok(conn) = self.db_conn.lock() {
            let _ = queue::enqueue(&conn, url, filename, dest_path, opts, threads);
        }
        self.refresh_queue();
        self.page = Page::Downloads;
    }

    fn start_download_item(&mut self, id: String, ctx: egui::Context) {
        let item = self.queue.iter().find(|d| d.id == id).cloned();
        if let Some(item) = item {
            {
                if let Ok(conn) = self.db_conn.lock() {
                    let _ = queue::update_status(&conn, &id, DownloadStatus::Active);
                }
            }
            let tx = self.dl_tx.clone();
            let ctx_c = ctx.clone();
            self.handle.spawn(async move {
                if let Err(e) =
                    engine::download_file(tx.clone(), id.clone(), item.url, item.dest_path, item.threads).await
                {
                    let _ = tx.send(DownloadEvent::Error(engine::ErrorEvent {
                        id,
                        error: e.to_string(),
                    }));
                }
                ctx_c.request_repaint();
            });
            self.refresh_queue();
        }
    }

    fn load_webdav_status(&self, ctx: egui::Context) {
        let tx = self.ev_tx.clone();
        self.handle.spawn(async move {
            let status = tokio::task::spawn_blocking(webdav::webdav_status).await.unwrap();
            let _ = tx.send(AppEvent::WebDavStatus(status));
            ctx.request_repaint();
        });
    }

    fn run_webdav<F>(&mut self, ctx: egui::Context, op: F, success_msg: &'static str)
    where
        F: FnOnce() -> Result<(), String> + Send + 'static,
    {
        self.webdav_busy = true;
        self.webdav_msg = None;
        let tx = self.ev_tx.clone();
        self.handle.spawn(async move {
            let result = tokio::task::spawn_blocking(move || op())
                .await
                .unwrap()
                .map(|_| success_msg.to_string());
            let _ = tx.send(AppEvent::WebDavDone(result));
            ctx.request_repaint();
        });
    }

    fn save_settings(&mut self, ctx: egui::Context) {
        if let Some(new_settings) = self.settings_edit.clone() {
            self.settings_saving = true;
            let tx = self.ev_tx.clone();
            let settings_arc = Arc::clone(&self.settings);
            self.handle.spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    settings::save_settings(&new_settings).map_err(|e| e.to_string())?;
                    *settings_arc.lock().unwrap() = new_settings;
                    Ok::<(), String>(())
                })
                .await
                .unwrap();
                let _ = tx.send(AppEvent::SettingsSaved(result));
                ctx.request_repaint();
            });
        }
    }

    // ---- UI drawing ----

    fn show_login(&mut self, ctx: &egui::Context) {
        let bg        = egui::Color32::from_gray(22);
        let footer_bg = egui::Color32::from_gray(30);
        let content_w = 400.0_f32;
        let footer_h  = 56.0_f32;

        let links: &[(&str, &str, &str)] = &[
            (egui_phosphor::regular::GLOBE,         "Website",   "https://xerolinux.xyz"),
            (egui_phosphor::regular::GITHUB_LOGO,   "GitHub",    "https://github.com/DarkXero-dev"),
            (egui_phosphor::regular::DISCORD_LOGO,  "Discord",   "https://discord.xerolinux.xyz"),
            (egui_phosphor::regular::YOUTUBE_LOGO,  "YouTube",   "https://youtube.com/@XeroLinux"),
            (egui_phosphor::regular::MASTODON_LOGO, "Fosstodon", "https://fosstodon.org/@XeroLinux"),
        ];

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(bg))
            .show(ctx, |ui| {
                let avail    = ui.available_rect_before_wrap();
                let panel_w  = avail.width();
                let panel_h  = avail.height();
                let form_h   = 460.0_f32;
                let top_y    = avail.min.y + ((panel_h - footer_h - form_h) / 2.0).max(20.0);
                let form_x   = avail.min.x + (panel_w - content_w) / 2.0;
                let footer_y = avail.max.y - footer_h;

                // Background decoration: subtle download/torrent glyphs
                {
                    let p = ui.painter();
                    let ghost = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 18);
                    let font_id = egui::FontId::proportional(150.0);
                    p.text(
                        egui::pos2(avail.min.x + panel_w * 0.10, avail.min.y + panel_h * 0.22),
                        egui::Align2::CENTER_CENTER,
                        egui_phosphor::regular::MAGNET,
                        font_id.clone(),
                        ghost,
                    );
                    p.text(
                        egui::pos2(avail.min.x + panel_w * 0.88, avail.min.y + panel_h * 0.30),
                        egui::Align2::CENTER_CENTER,
                        egui_phosphor::regular::CLOUD_ARROW_DOWN,
                        egui::FontId::proportional(170.0),
                        ghost,
                    );
                    p.text(
                        egui::pos2(avail.min.x + panel_w * 0.85, avail.min.y + panel_h * 0.74),
                        egui::Align2::CENTER_CENTER,
                        egui_phosphor::regular::DOWNLOAD_SIMPLE,
                        egui::FontId::proportional(120.0),
                        ghost,
                    );
                }

                // Form - centered layout inside explicit rect (no vertical_centered NaN issue)
                let form_rect = egui::Rect::from_min_size(
                    egui::pos2(form_x, top_y),
                    egui::vec2(content_w, form_h),
                );
                let ir = ui.allocate_ui_at_rect(form_rect, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        ui.add(
                            egui::Image::new(egui::include_image!("../../assets/rd_logo.png"))
                                .max_width(300.0)
                                .maintain_aspect_ratio(true),
                        );
                        ui.add_space(14.0);
                        ui.label(
                            RichText::new("Premium multi-host downloader client")
                                .size(17.0)
                                .color(theme::MUTED),
                        );
                        ui.add_space(30.0);
                        ui.separator();
                        ui.add_space(24.0);
                        ui.label(
                            RichText::new("API TOKEN").size(13.0).strong().color(theme::MUTED),
                        );
                        ui.add_space(8.0);
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.token_input)
                                .password(true)
                                .desired_width(content_w)
                                .hint_text("Paste your Real-Debrid API token"),
                        );
                        ui.add_space(9.0);
                        let tok_link = ui.add(
                            egui::Label::new(
                                RichText::new("Get token at real-debrid.com/apitoken")
                                    .size(14.0)
                                    .color(theme::GREEN),
                            )
                            .sense(egui::Sense::click()),
                        );
                        if tok_link.clicked() {
                            let _ = open::that("https://real-debrid.com/apitoken");
                        }
                        if tok_link.hovered() {
                            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if let Some(ref e) = self.login_error.clone() {
                            ui.add_space(10.0);
                            egui::Frame::new()
                                .fill(egui::Color32::from_rgb(60, 20, 20))
                                .stroke(egui::Stroke::new(1.0, theme::ERROR))
                                .corner_radius(egui::CornerRadius::same(6))
                                .inner_margin(egui::Margin::same(8))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(e).size(13.0).color(theme::ERROR));
                                });
                        }
                        ui.add_space(22.0);
                        let can_login =
                            !self.token_input.trim().is_empty() && !self.login_loading;
                        let btn_label =
                            if self.login_loading { "Connecting..." } else { "Connect" };
                        let btn = if can_login {
                            ui.add(
                                egui::Button::new(
                                    RichText::new(btn_label)
                                        .size(16.0)
                                        .strong()
                                        .color(egui::Color32::from_rgb(8, 22, 14)),
                                )
                                .fill(theme::GREEN)
                                .min_size(egui::vec2(content_w, 48.0)),
                            )
                        } else {
                            ui.add(
                                egui::Button::new(
                                    RichText::new(btn_label).size(16.0).color(theme::MUTED),
                                )
                                .fill(egui::Color32::from_gray(32))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(45)))
                                .min_size(egui::vec2(content_w, 48.0)),
                            )
                        };
                        (resp, can_login, btn.clicked())
                    })
                    .inner
                });
                let (resp, can_login, btn_clicked) = ir.inner;
                if (btn_clicked && can_login)
                    || (resp.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    self.do_login(ctx.clone());
                }

                // Footer - painter-driven for exact centering + glow
                let footer_rect = egui::Rect::from_min_size(
                    egui::pos2(avail.min.x, footer_y),
                    egui::vec2(panel_w, footer_h),
                );
                ui.allocate_ui_at_rect(footer_rect, |ui| {
                    let fr  = ui.max_rect();
                    let p   = ui.painter_at(fr);
                    let sep = egui::Color32::from_gray(50);

                    p.rect_filled(fr, 0.0, footer_bg);
                    p.hline(fr.x_range(), fr.min.y, egui::Stroke::new(1.0, sep));
                    p.hline(fr.x_range(), fr.max.y - 1.0, egui::Stroke::new(1.0, sep));

                    // Per-link widths: icon(13) + 2 spaces(9) + chars * 7.5
                    let gap = 32.0_f32;
                    let font = egui::FontId::proportional(13.0);
                    let link_ws: Vec<f32> = links
                        .iter()
                        .map(|(_, lbl, _)| 22.0 + lbl.chars().count() as f32 * 7.5)
                        .collect();
                    let total_w: f32 =
                        link_ws.iter().sum::<f32>() + gap * (links.len() as f32 - 1.0);
                    let mut x   = fr.center().x - total_w / 2.0;
                    let y_mid   = fr.center().y;

                    for ((icon, lbl, url), &lw) in links.iter().zip(link_ws.iter()) {
                        let text = format!("{icon}  {lbl}");
                        let lr   = egui::Rect::from_min_size(
                            egui::pos2(x, y_mid - 12.0),
                            egui::vec2(lw, 24.0),
                        );
                        let resp = ui.allocate_rect(lr, egui::Sense::click());
                        let color = if resp.hovered() { theme::GREEN } else { theme::MUTED };

                        if resp.hovered() {
                            p.rect_filled(
                                lr.expand2(egui::vec2(9.0, 5.0)),
                                6.0,
                                egui::Color32::from_rgba_unmultiplied(74, 222, 128, 10),
                            );
                            p.rect_filled(
                                lr.expand2(egui::vec2(5.0, 3.0)),
                                4.0,
                                egui::Color32::from_rgba_unmultiplied(74, 222, 128, 22),
                            );
                            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        p.text(
                            egui::pos2(x, y_mid),
                            egui::Align2::LEFT_CENTER,
                            &text,
                            font.clone(),
                            color,
                        );
                        if resp.clicked() {
                            let _ = open::that(*url);
                        }
                        x += lw + gap;
                    }

                    ui.allocate_rect(fr, egui::Sense::hover());
                });
            });
    }

    fn show_main(&mut self, ctx: &egui::Context) {
        // Update banner
        if let Some(ref v) = self.update_available.clone() {
            egui::TopBottomPanel::top("update_banner")
                .frame(theme::green_frame())
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Update v{v} available"))
                                .color(theme::GREEN)
                                .size(13.0),
                        );
                        if ui.link("Download").clicked() {
                            let _ = open::that(
                                "https://github.com/DarkXero-dev/RDTool/releases/latest",
                            );
                        }
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui.small_button("x").clicked() {
                                    self.update_available = None;
                                }
                            },
                        );
                    });
                });
        }

        // Sidebar
        egui::SidePanel::left("sidebar")
            .exact_width(92.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .stroke(egui::Stroke::new(1.0, theme::BORDER)),
            )
            .show(ctx, |ui| {
                ui.add_space(14.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("RD").size(22.0).strong().color(theme::GREEN));
                });
                ui.add_space(10.0);
                ui.add(egui::Separator::default().spacing(0.0));
                ui.add_space(10.0);

                let nav = [
                    (egui_phosphor::regular::HOUSE, "Home", Page::Dashboard),
                    (egui_phosphor::regular::LINK, "Links", Page::Downloader),
                    (egui_phosphor::regular::MAGNET, "Magnets", Page::Torrents),
                    (egui_phosphor::regular::PLAY_CIRCLE, "Stream", Page::Streaming),
                    (egui_phosphor::regular::DOWNLOAD_SIMPLE, "Queue", Page::Downloads),
                    (egui_phosphor::regular::GEAR, "Settings", Page::Settings),
                ];

                for (icon, label, p) in nav {
                    let active = self.page == p;
                    let item_size = egui::vec2(76.0, 66.0);
                    let (rect, resp) =
                        ui.allocate_exact_size(item_size, egui::Sense::click());

                    if resp.hovered() {
                        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    if ui.is_rect_visible(rect) {
                        let bg = if active {
                            theme::GREEN_DIM
                        } else if resp.hovered() {
                            theme::CARD
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        let color = if active {
                            theme::GREEN
                        } else if resp.hovered() {
                            theme::TEXT
                        } else {
                            theme::MUTED
                        };

                        ui.painter()
                            .rect_filled(rect, egui::CornerRadius::same(8), bg);

                        if active {
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(
                                    rect.left_top(),
                                    egui::vec2(3.0, rect.height()),
                                ),
                                egui::CornerRadius::same(2),
                                theme::GREEN,
                            );
                        }

                        ui.painter().text(
                            egui::pos2(rect.center().x, rect.center().y - 8.0),
                            egui::Align2::CENTER_CENTER,
                            icon,
                            egui::FontId::proportional(24.0),
                            color,
                        );
                        ui.painter().text(
                            egui::pos2(rect.center().x, rect.bottom() - 9.0),
                            egui::Align2::CENTER_CENTER,
                            label,
                            egui::FontId::proportional(11.5),
                            color,
                        );
                    }

                    if resp.clicked() {
                        self.page = p;
                        self.on_page_enter(ctx);
                    }

                    ui.add_space(2.0);
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    if ui
                        .add(egui::Button::new(RichText::new("Logout").size(11.0).color(theme::MUTED)).min_size(egui::vec2(72.0, 28.0)))
                        .clicked()
                    {
                        let _ = auth::clear_token();
                        self.logged_in = false;
                        self.user = None;
                    }
                    if let Some(ref u) = self.user {
                        let name = if u.username.len() > 10 {
                            &u.username[..10]
                        } else {
                            &u.username
                        };
                        ui.label(RichText::new(name).size(12.0).color(theme::MUTED));
                    }
                    ui.add_space(6.0);
                });
            });

        // Main content
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::same(20)))
            .show(ctx, |ui| {
                // Per-page ghost decorations
                {
                    let pr = ui.max_rect();
                    let p  = ui.painter_at(pr);
                    let ghost = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 14);
                    let g2    = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 9);
                    match self.page {
                        Page::Dashboard => {
                            p.text(egui::pos2(pr.max.x - 160.0, pr.min.y + pr.height() * 0.18), egui::Align2::CENTER_CENTER, egui_phosphor::regular::USER_CIRCLE, egui::FontId::proportional(160.0), ghost);
                            p.text(egui::pos2(pr.min.x + 120.0, pr.min.y + pr.height() * 0.72), egui::Align2::CENTER_CENTER, egui_phosphor::regular::STAR, egui::FontId::proportional(110.0), g2);
                        }
                        Page::Downloader => {
                            p.text(egui::pos2(pr.max.x - 140.0, pr.min.y + pr.height() * 0.22), egui::Align2::CENTER_CENTER, egui_phosphor::regular::LINK, egui::FontId::proportional(150.0), ghost);
                            p.text(egui::pos2(pr.min.x + 100.0, pr.min.y + pr.height() * 0.68), egui::Align2::CENTER_CENTER, egui_phosphor::regular::LOCK_OPEN, egui::FontId::proportional(120.0), g2);
                        }
                        Page::Torrents => {
                            p.text(egui::pos2(pr.max.x - 140.0, pr.min.y + pr.height() * 0.25), egui::Align2::CENTER_CENTER, egui_phosphor::regular::MAGNET, egui::FontId::proportional(150.0), ghost);
                            p.text(egui::pos2(pr.min.x + 110.0, pr.min.y + pr.height() * 0.70), egui::Align2::CENTER_CENTER, egui_phosphor::regular::HARD_DRIVES, egui::FontId::proportional(110.0), g2);
                        }
                        Page::Streaming => {
                            p.text(egui::pos2(pr.max.x - 150.0, pr.min.y + pr.height() * 0.20), egui::Align2::CENTER_CENTER, egui_phosphor::regular::PLAY_CIRCLE, egui::FontId::proportional(160.0), ghost);
                            p.text(egui::pos2(pr.min.x + 110.0, pr.min.y + pr.height() * 0.65), egui::Align2::CENTER_CENTER, egui_phosphor::regular::FILM_SLATE, egui::FontId::proportional(120.0), g2);
                        }
                        Page::Downloads => {
                            p.text(egui::pos2(pr.max.x - 140.0, pr.min.y + pr.height() * 0.22), egui::Align2::CENTER_CENTER, egui_phosphor::regular::DOWNLOAD_SIMPLE, egui::FontId::proportional(150.0), ghost);
                            p.text(egui::pos2(pr.min.x + 100.0, pr.min.y + pr.height() * 0.68), egui::Align2::CENTER_CENTER, egui_phosphor::regular::LIST_BULLETS, egui::FontId::proportional(110.0), g2);
                        }
                        Page::Settings => {
                            p.text(egui::pos2(pr.max.x - 140.0, pr.min.y + pr.height() * 0.22), egui::Align2::CENTER_CENTER, egui_phosphor::regular::GEAR, egui::FontId::proportional(155.0), ghost);
                            p.text(egui::pos2(pr.min.x + 110.0, pr.min.y + pr.height() * 0.68), egui::Align2::CENTER_CENTER, egui_phosphor::regular::SLIDERS, egui::FontId::proportional(110.0), g2);
                        }
                    }
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    match self.page {
                        Page::Dashboard => self.show_dashboard(ui),
                        Page::Downloader => self.show_downloader(ui, ctx),
                        Page::Torrents => self.show_torrents(ui, ctx),
                        Page::Streaming => self.show_streaming(ui, ctx),
                        Page::Downloads => self.show_downloads_page(ui, ctx),
                        Page::Settings => self.show_settings_page(ui, ctx),
                    }
                });
            });
    }

    fn on_page_enter(&mut self, ctx: &egui::Context) {
        match self.page {
            Page::Downloader => {
                self.load_rd_downloads(ctx.clone());
            }
            Page::Torrents => {
                self.last_torrent_refresh = Some(std::time::Instant::now());
                self.load_torrents(ctx.clone());
            }
            Page::Settings => {
                let s = self.settings.lock().unwrap().clone();
                self.settings_edit = Some(s);
                self.load_webdav_status(ctx.clone());
                self.settings_saved = false;
            }
            Page::Downloads => {
                self.refresh_queue();
            }
            Page::Dashboard => {
                self.fetch_user(ctx.clone(), false);
            }
            _ => {}
        }
    }

    fn show_dashboard(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Dashboard").size(26.0).strong());
        ui.add_space(4.0);
        ui.label(RichText::new("Account overview").size(14.0).color(theme::MUTED));
        ui.add_space(28.0);

        if let Some(ref u) = self.user.clone() {
            // API returns seconds remaining, convert to days
            let days_left = u.premium / 86400;
            // Trim time from ISO date string
            let exp_raw = u.expiration.as_deref().unwrap_or("N/A");
            let exp = exp_raw.get(..10).unwrap_or(exp_raw);

            // Row 1: Username + Account Type
            ui.columns(2, |cols| {
                theme::card_frame().show(&mut cols[0], |ui| {
                    ui.label(RichText::new("USERNAME").size(11.0).color(theme::MUTED).strong());
                    ui.add_space(6.0);
                    ui.label(RichText::new(&u.username).size(20.0).strong());
                });
                theme::card_frame().show(&mut cols[1], |ui| {
                    ui.label(RichText::new("ACCOUNT TYPE").size(11.0).color(theme::MUTED).strong());
                    ui.add_space(6.0);
                    ui.label(RichText::new(&u.account_type).size(20.0).strong().color(theme::GREEN));
                });
            });
            ui.add_space(12.0);

            // Row 2: Days remaining + Points
            ui.columns(2, |cols| {
                theme::card_frame().show(&mut cols[0], |ui| {
                    ui.label(RichText::new("PREMIUM REMAINING").size(11.0).color(theme::MUTED).strong());
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(days_left.to_string()).size(36.0).strong().color(theme::GREEN));
                        ui.label(RichText::new(" days").size(16.0).color(theme::MUTED));
                    });
                });
                theme::card_frame().show(&mut cols[1], |ui| {
                    ui.label(RichText::new("FIDELITY POINTS").size(11.0).color(theme::MUTED).strong());
                    ui.add_space(6.0);
                    ui.label(RichText::new(u.points.to_string()).size(36.0).strong().color(theme::GREEN));
                });
            });
            ui.add_space(12.0);

            // Row 3: Email + Expiry date
            ui.columns(2, |cols| {
                theme::card_frame().show(&mut cols[0], |ui| {
                    ui.label(RichText::new("EMAIL").size(11.0).color(theme::MUTED).strong());
                    ui.add_space(6.0);
                    ui.label(RichText::new(&u.email).size(14.0));
                });
                theme::card_frame().show(&mut cols[1], |ui| {
                    ui.label(RichText::new("EXPIRES").size(11.0).color(theme::MUTED).strong());
                    ui.add_space(6.0);
                    ui.label(RichText::new(exp).size(14.0));
                });
            });
        } else {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Loading account info...").color(theme::MUTED).size(15.0));
            });
        }
    }

    fn show_downloader(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.label(RichText::new("Link Unrestrictor").size(26.0).strong());
        ui.add_space(4.0);
        ui.label(
            RichText::new("Paste premium links, one per line")
                .size(14.0)
                .color(theme::MUTED),
        );
        ui.add_space(16.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("Links").size(12.0).color(theme::MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Paste").clicked() {
                    if let Ok(mut cb) = arboard::Clipboard::new() {
                        if let Ok(txt) = cb.get_text() {
                            if !self.dl_input.is_empty() {
                                self.dl_input.push('\n');
                            }
                            self.dl_input.push_str(&txt);
                        }
                    }
                }
                if !self.dl_input.is_empty() {
                    if ui.small_button("Clear").clicked() {
                        self.dl_input.clear();
                    }
                }
            });
        });
        ui.add_space(4.0);
        ui.add(
            egui::TextEdit::multiline(&mut self.dl_input)
                .desired_rows(14)
                .desired_width(f32::INFINITY)
                .hint_text("https://example.com/file.mkv\nhttps://..."),
        );
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            let busy = self.dl_loading;
            let can_go = !busy && !self.dl_input.trim().is_empty();

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        can_go,
                        egui::Button::new(
                            RichText::new(if busy { "Unrestricting..." } else { "Unrestrict Links" })
                                .size(14.0)
                                .strong()
                                .color(if can_go { egui::Color32::from_gray(20) } else { egui::Color32::from_gray(150) }),
                        )
                        .fill(if can_go { theme::GREEN } else { egui::Color32::from_gray(45) })
                        .min_size(egui::vec2(180.0, 40.0)),
                    )
                    .clicked()
                {
                    self.unrestrict_links(ctx.clone());
                }
            });
        });

        ui.add_space(8.0);
        theme::card_frame().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("HOW TO USE").size(11.0).color(theme::MUTED).strong());
            ui.add_space(8.0);
            let steps: &[(&str, &str)] = &[
                (egui_phosphor::regular::NUMBER_CIRCLE_ONE, "Paste one or more premium links (one per line) into the box above. Use the Paste button or right-click paste."),
                (egui_phosphor::regular::NUMBER_CIRCLE_TWO, "Click Unrestrict Links. Real-Debrid converts them into fast direct download links."),
                (egui_phosphor::regular::NUMBER_CIRCLE_THREE, "Queue individual links to download them directly via the built-in downloader, or Copy to use an external tool."),
                (egui_phosphor::regular::NUMBER_CIRCLE_FOUR, "Export to TXT saves all unrestricted links to a file for use with any download manager."),
            ];
            for (icon, text) in steps {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(*icon).size(18.0).color(theme::GREEN));
                    ui.add_space(6.0);
                    ui.label(RichText::new(*text).size(12.0).color(theme::MUTED));
                });
                ui.add_space(4.0);
            }
        });

        // Supported hosters - always visible below input box
        ui.add_space(16.0);
        ui.label(RichText::new("SUPPORTED HOSTERS").size(11.0).color(theme::MUTED).strong());
        ui.add_space(8.0);
        const HOSTERS: &[&str] = &[
            "1Fichier", "Rapidgator", "Nitroflare", "Turbobit", "Keep2Share",
            "Uploaded", "Filefactory", "Depositfiles", "Katfile", "Mexashare",
            "4Shared", "Dropbox", "Google Drive", "Mega", "OneDrive",
            "Uptobox", "Alfafile", "Wupfile", "Subyshare", "Ddownload",
            "Rg.to", "Icerbox", "Hotlink", "Upload.ee", "Worldbytez",
            "Dailyuploads", "FileSpace", "Hugefiles", "Veryfiles", "Dl.free.fr",
            "UsersDrive", "ClicknUpload", "Filerio", "Takefile", "Filesfly",
        ];
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for hoster in HOSTERS {
                ui.add(
                    egui::Button::new(RichText::new(*hoster).size(11.5).color(egui::Color32::from_gray(210)))
                        .fill(egui::Color32::from_gray(42))
                        .corner_radius(egui::CornerRadius::same(5)),
                );
            }
        });

        // RD Downloads section at bottom
        if !self.rd_downloads.is_empty() {
            ui.add_space(20.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("RD DOWNLOADS").size(11.0).color(theme::MUTED).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let ctx_c = ctx.clone();
                    if ui.small_button(egui_phosphor::regular::ARROWS_CLOCKWISE).clicked() {
                        self.load_rd_downloads(ctx_c);
                    }
                });
            });
            ui.add_space(8.0);
            let rd_downloads = self.rd_downloads.clone();
            let mut rd_delete_id: Option<String> = None;
            for d in &rd_downloads {
                theme::card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_max_width(ui.available_width() - 175.0);
                            ui.label(RichText::new(&d.filename).size(13.0).strong());
                            ui.label(
                                RichText::new(format!("{} - {}", d.host, format_bytes(d.filesize)))
                                    .size(11.0)
                                    .color(theme::MUTED),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(
                                RichText::new("Delete").color(theme::ERROR)
                            ).min_size(egui::vec2(55.0, 24.0))).clicked() {
                                rd_delete_id = Some(d.id.clone());
                            }
                            if ui.add(egui::Button::new("Queue").min_size(egui::vec2(55.0, 24.0))).clicked() {
                                self.enqueue_download(d.download.clone(), d.filename.clone());
                            }
                            if ui.add(egui::Button::new("Copy").min_size(egui::vec2(55.0, 24.0))).clicked() {
                                ui.ctx().copy_text(d.download.clone());
                            }
                        });
                    });
                });
                ui.add_space(6.0);
            }
            if let Some(id) = rd_delete_id {
                self.delete_rd_download_async(id, ctx.clone());
            }
        }
    }

    fn show_torrents(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.label(RichText::new("Torrents").size(26.0).strong());
        ui.add_space(16.0);

        // Add magnet / torrent file
        theme::card_frame().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical(|ui| {
                ui.label(RichText::new("Add Torrent").size(13.0).strong());
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let ctx_c = ctx.clone();
                    let busy = self.torrent_selecting;
                    let btn_size = egui::vec2(100.0, 32.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.torrent_magnet)
                            .desired_width(ui.available_width() - 216.0)
                            .hint_text("magnet:?xt=urn:btih:...")
                            .margin(egui::Margin::symmetric(8, 7)),
                    );
                    if ui.add(egui::Button::new("Paste").min_size(btn_size)).clicked() {
                        if let Ok(mut cb) = arboard::Clipboard::new() {
                            if let Ok(txt) = cb.get_text() {
                                self.torrent_magnet = txt.trim().to_string();
                            }
                        }
                    }
                    if ui.add_enabled(!busy, egui::Button::new(
                        RichText::new(if busy { "Waiting..." } else { "Add Magnet" })
                    ).min_size(btn_size)).clicked() {
                        self.add_magnet(ctx_c);
                    }
                });
                ui.add_space(4.0);
                let ctx_c = ctx.clone();
                if ui.button("Open .torrent file...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Torrent", &["torrent"])
                        .pick_file()
                    {
                        if let Ok(bytes) = std::fs::read(&path) {
                            let filename = path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            self.add_torrent_file(bytes, filename, ctx_c);
                        }
                    }
                }
            });
        });



        ui.add_space(8.0);
        theme::card_frame().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("HOW TO USE").size(11.0).color(theme::MUTED).strong());
            ui.add_space(8.0);
            let steps: &[(&str, &str)] = &[
                (egui_phosphor::regular::NUMBER_CIRCLE_ONE, "Paste a magnet link above or open a .torrent file. Click Add Magnet / open file."),
                (egui_phosphor::regular::NUMBER_CIRCLE_TWO, "A file picker will appear once Real-Debrid processes the torrent. Select which files to download and confirm."),
                (egui_phosphor::regular::NUMBER_CIRCLE_THREE, "Once the torrent finishes leeching (status: Downloaded), click Queue to add files to the download queue."),
                (egui_phosphor::regular::NUMBER_CIRCLE_FOUR, "Switch to the Queue page to track download progress. Use Delete to remove a torrent from Real-Debrid."),
            ];
            for (icon, text) in steps {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(*icon).size(18.0).color(theme::GREEN));
                    ui.add_space(6.0);
                    ui.label(RichText::new(*text).size(12.0).color(theme::MUTED));
                });
                ui.add_space(4.0);
            }
        });

        ui.add_space(16.0);

        // Tabs
        let has_done = self.torrents.iter().any(|t| t.status == "downloaded");
        ui.horizontal(|ui| {
            ui.label(RichText::new("My Torrents").size(13.0).strong());
            ui.add_space(8.0);
            let ctx_c = ctx.clone();
            if ui.small_button(egui_phosphor::regular::ARROWS_CLOCKWISE).clicked() {
                self.last_torrent_refresh = Some(std::time::Instant::now());
                self.load_torrents(ctx_c);
            }
            if has_done {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let ctx_c2 = ctx.clone();
                    if ui.small_button("Clear Completed").clicked() {
                        let ids: Vec<String> = self.torrents.iter()
                            .filter(|t| t.status == "downloaded")
                            .map(|t| t.id.clone())
                            .collect();
                        for id in ids {
                            self.delete_torrent_async(id, ctx_c2.clone());
                        }
                    }
                });
            }
        });
        ui.add_space(8.0);

        {
                if self.torrents_loading && self.torrents.is_empty() {
                    ui.label(RichText::new("Loading...").color(theme::MUTED));
                } else if self.torrents.is_empty() {
                    ui.label(
                        RichText::new("No torrents found")
                            .color(theme::MUTED)
                            .size(13.0),
                    );
                } else {

                    let torrents = self.torrents.clone();
                    let mut delete_torrent_id: Option<String> = None;
                    const BTN_W: f32 = 110.0;
                    const BTN_H: f32 = 30.0;
                    for t in &torrents {
                        let n_btns = 1
                            + if t.status == "downloaded" && !t.links.is_empty() { 1 } else { 0 }
                            + if t.status == "waiting_files_selection" { 1 } else { 0 };
                        let reserved = BTN_W * n_btns as f32 + 8.0 * (n_btns as f32 + 1.0);
                        theme::card_frame().show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_max_width(ui.available_width() - reserved);
                                    ui.label(RichText::new(&t.filename).size(13.0).strong());
                                    let (status_text, status_color) = match t.status.as_str() {
                                        "downloaded" => (format!("Downloaded - {}", format_bytes(t.bytes)), theme::GREEN),
                                        "downloading" => (format!("Downloading {:.0}% - {} seeders", t.progress, t.seeders.unwrap_or(0)), theme::WARNING),
                                        "waiting_files_selection" => ("Waiting for file selection".to_string(), theme::WARNING),
                                        "queued" => ("Queued".to_string(), theme::MUTED),
                                        "magnet_conversion" => ("Converting magnet...".to_string(), theme::MUTED),
                                        s => (s.to_string(), theme::MUTED),
                                    };
                                    ui.label(RichText::new(status_text).size(11.0).color(status_color));
                                    if t.status == "downloading" {
                                        ui.add(egui::ProgressBar::new(t.progress as f32 / 100.0)
                                            .desired_width(f32::INFINITY));
                                    }
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    // right_to_left: first added = rightmost
                                    if ui.add(egui::Button::new(
                                        RichText::new("Delete").color(theme::ERROR)
                                    ).min_size(egui::vec2(BTN_W, BTN_H))).clicked() {
                                        delete_torrent_id = Some(t.id.clone());
                                    }
                                    if t.status == "downloaded" && !t.links.is_empty() {
                                        if ui.add(egui::Button::new("Queue")
                                            .min_size(egui::vec2(BTN_W, BTN_H))).clicked() {
                                            let links = t.links.clone();
                                            self.unrestrict_and_enqueue_torrent(links, ctx.clone());
                                        }
                                    }
                                    if t.status == "waiting_files_selection" {
                                        if ui.add(egui::Button::new(
                                            RichText::new("Select Files").color(theme::GREEN)
                                        ).min_size(egui::vec2(BTN_W, BTN_H))).clicked() {
                                            self.torrent_selecting = true;
                                            self.torrent_selection_status = "Waiting...".to_string();
                                            self.fetch_torrent_for_selection(t.id.clone(), ctx.clone());
                                        }
                                    }
                                });
                            });
                        });
                        ui.add_space(6.0);
                    }
                    if let Some(id) = delete_torrent_id {
                        self.delete_torrent_async(id, ctx.clone());
                    }
                }
            }
    }

    fn show_streaming(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.label(RichText::new("Streaming").size(26.0).strong());
        ui.add_space(4.0);
        ui.label(
            RichText::new("Generate stream transcodes for a download ID")
                .size(14.0)
                .color(theme::MUTED),
        );
        ui.add_space(16.0);

        theme::card_frame().show(ui, |ui| {
            ui.label(RichText::new("DOWNLOAD ID").size(12.0).color(theme::MUTED).strong());
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let ctx_c = ctx.clone();
                let can_go = !self.stream_loading && !self.stream_input.trim().is_empty();
                ui.add(
                    egui::TextEdit::singleline(&mut self.stream_input)
                        .desired_width(ui.available_width() - 150.0)
                        .hint_text("Download ID (from RD Downloads)")
                        .font(egui::TextStyle::Monospace),
                );
                if ui
                    .add_enabled(
                        can_go,
                        egui::Button::new(
                            RichText::new(if self.stream_loading { "Loading..." } else { "Get Streams" })
                                .size(13.0),
                        )
                        .min_size(egui::vec2(140.0, 0.0)),
                    )
                    .clicked()
                {
                    self.get_stream_info(ctx_c);
                }
            });
        });



        if let Some(ref info) = self.stream_info.clone() {
            ui.add_space(16.0);

            let mut show_section = |label: &str, map: &Option<std::collections::HashMap<String, String>>| {
                if let Some(m) = map {
                    if !m.is_empty() {
                        ui.label(RichText::new(label).size(13.0).strong());
                        ui.add_space(4.0);
                        for (quality, url) in m {
                            theme::card_frame().show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(quality).size(13.0).color(theme::GREEN),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let url_c = url.clone();
                                            if ui.small_button("Open in player").clicked() {
                                                let _ = open::that(&url_c);
                                            }
                                            let url_c2 = url.clone();
                                            if ui.small_button("Copy URL").clicked() {
                                                ui.ctx().copy_text(url_c2);
                                            }
                                        },
                                    );
                                });
                            });
                            ui.add_space(4.0);
                        }
                        ui.add_space(8.0);
                    }
                }
            };

            show_section("HLS (Apple)", &info.apple);
            show_section("DASH", &info.dash);
            show_section("MP4 (Live)", &info.liveMP4);
            show_section("WebM (H264)", &info.h264WebM);
        }
    }

    fn show_downloads_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Download Queue").size(26.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button(egui_phosphor::regular::ARROWS_CLOCKWISE).clicked() {
                    self.refresh_queue();
                }
            });
        });

        ui.add_space(16.0);

        if self.queue.is_empty() {
            ui.label(
                RichText::new("Queue is empty. Unrestrict links and click Queue to add downloads.")
                    .color(theme::MUTED)
                    .size(13.0),
            );
            return;
        }

        let queue = self.queue.clone();
        let progress = self.dl_progress.clone();
        let mut start_id: Option<String> = None;
        let mut cancel_id: Option<String> = None;
        let mut retry_id: Option<String> = None;
        let mut delete_id: Option<(String, Option<String>)> = None;

        for item in &queue {
            theme::card_frame().show(ui, |ui| {
                let card_w = ui.available_width();
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        // Info column - reserve space for buttons on right
                        let btn_w = match item.status {
                            DownloadStatus::Paused => 210.0,
                            _ => 130.0,
                        };
                        ui.vertical(|ui| {
                            ui.set_max_width(ui.available_width() - btn_w);
                            ui.label(RichText::new(&item.filename).size(13.0).strong());
                            let (status_text, status_color) = match item.status {
                                DownloadStatus::Active => (format!("Downloading - {}", format_bytes(item.bytes_done)), theme::GREEN),
                                DownloadStatus::Completed => (format!("Completed - {}", format_bytes(item.bytes_done)), theme::GREEN),
                                DownloadStatus::Failed => {
                                    let err = item.error_msg.as_deref().unwrap_or("Unknown error");
                                    (format!("Failed: {}", err), theme::ERROR)
                                }
                                DownloadStatus::Paused => (format!("Paused - {}", format_bytes(item.bytes_done)), theme::WARNING),
                                DownloadStatus::Cancelled => ("Cancelled".to_string(), theme::MUTED),
                                DownloadStatus::Queued => ("Queued".to_string(), theme::MUTED),
                                DownloadStatus::Scheduled => {
                                    let at = item.scheduled_at.as_deref().unwrap_or("?");
                                    (format!("Scheduled: {}", at), theme::MUTED)
                                }
                            };
                            ui.label(RichText::new(status_text).size(11.0).color(status_color));
                        });

                        // Action buttons (right-aligned)
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Delete always available - opens confirm dialog
                            if ui.add(egui::Button::new(
                                RichText::new("Delete").color(theme::ERROR)
                            ).min_size(egui::vec2(60.0, 24.0))).clicked() {
                                let path = if std::path::Path::new(&item.dest_path).exists() {
                                    Some(item.dest_path.clone())
                                } else {
                                    None
                                };
                                delete_id = Some((item.id.clone(), path));
                            }
                            match item.status {
                                DownloadStatus::Queued | DownloadStatus::Scheduled => {
                                    if ui.add(egui::Button::new("Start").min_size(egui::vec2(60.0, 24.0))).clicked() {
                                        start_id = Some(item.id.clone());
                                    }
                                }
                                DownloadStatus::Active => {
                                    if ui.add(egui::Button::new("Cancel").min_size(egui::vec2(60.0, 24.0))).clicked() {
                                        cancel_id = Some(item.id.clone());
                                    }
                                }
                                DownloadStatus::Paused => {
                                    if ui.add(egui::Button::new("Cancel").min_size(egui::vec2(60.0, 24.0))).clicked() {
                                        cancel_id = Some(item.id.clone());
                                    }
                                    if ui.add(egui::Button::new("Resume").min_size(egui::vec2(60.0, 24.0))).clicked() {
                                        start_id = Some(item.id.clone());
                                    }
                                }
                                DownloadStatus::Failed | DownloadStatus::Cancelled => {
                                    if ui.add(egui::Button::new(
                                        RichText::new("Retry").color(theme::GREEN)
                                    ).min_size(egui::vec2(60.0, 24.0))).clicked() {
                                        retry_id = Some(item.id.clone());
                                    }
                                }
                                DownloadStatus::Completed => {}
                            }
                        });
                    });

                    // Progress bar for active and paused
                    let show_progress = matches!(item.status, DownloadStatus::Active | DownloadStatus::Paused);
                    if show_progress {
                        if let Some(prog) = progress.get(&item.id) {
                            let fraction = prog
                                .total_bytes
                                .map(|t| if t > 0 { prog.bytes_done as f32 / t as f32 } else { 0.0 })
                                .unwrap_or(0.0)
                                .clamp(0.0, 1.0);
                            ui.add_space(6.0);
                            let label = if item.status == DownloadStatus::Active {
                                format!(
                                    "{} / {} - {}/s",
                                    format_bytes(prog.bytes_done),
                                    prog.total_bytes.map(format_bytes).unwrap_or_else(|| "?".into()),
                                    format_bytes(prog.speed_bps)
                                )
                            } else {
                                format!(
                                    "{} / {} (paused)",
                                    format_bytes(prog.bytes_done),
                                    prog.total_bytes.map(format_bytes).unwrap_or_else(|| "?".into()),
                                )
                            };
                            ui.add(
                                egui::ProgressBar::new(fraction)
                                    .text(label)
                                    .desired_width(card_w),
                            );
                        } else if item.total_bytes.map(|t| t > 0).unwrap_or(false) {
                            let fraction = item.total_bytes
                                .map(|t| if t > 0 { item.bytes_done as f32 / t as f32 } else { 0.0 })
                                .unwrap_or(0.0)
                                .clamp(0.0, 1.0);
                            ui.add_space(6.0);
                            ui.add(
                                egui::ProgressBar::new(fraction)
                                    .text(format!("{} / {}", format_bytes(item.bytes_done), format_bytes(item.total_bytes.unwrap_or(0))))
                                    .desired_width(card_w),
                            );
                        }
                    }
                    if item.status == DownloadStatus::Completed {
                        ui.add_space(6.0);
                        ui.add(
                            egui::ProgressBar::new(1.0)
                                .text(format!("{} - Complete", format_bytes(item.bytes_done)))
                                .desired_width(card_w),
                        );
                    }
                });
            });
            ui.add_space(6.0);
        }

        // Apply deferred actions
        if let Some(id) = start_id {
            self.start_download_item(id, ctx.clone());
        }
        if let Some(id) = cancel_id {
            if let Ok(conn) = self.db_conn.lock() {
                let _ = queue::update_status(&conn, &id, DownloadStatus::Cancelled);
            }
            self.dl_progress.remove(&id);
            self.refresh_queue();
        }
        if let Some(id) = retry_id {
            if let Ok(conn) = self.db_conn.lock() {
                let _ = queue::update_status(&conn, &id, DownloadStatus::Queued);
            }
            self.dl_progress.remove(&id);
            self.start_download_item(id, ctx.clone());
        }
        if let Some((id, path)) = delete_id {
            self.delete_confirm = Some((id, path));
        }
    }

    fn show_settings_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let saving = self.settings_saving;
        let saved = self.settings_saved;
        let mut do_save = false;

        egui::TopBottomPanel::bottom("settings_save_bar")
            .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(0, 12)))
            .show_inside(ui, |ui| {
                ui.separator();
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Settings will take effect immediately.")
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                !saving,
                                egui::Button::new(
                                    RichText::new(if saving { "Saving..." } else { "Save Settings" })
                                        .size(14.0)
                                        .strong()
                                        .color(egui::Color32::from_gray(20)),
                                )
                                .fill(theme::GREEN)
                                .min_size(egui::vec2(200.0, 44.0)),
                            )
                            .clicked()
                        {
                            do_save = true;
                        }
                        if saved {
                            ui.label(
                                RichText::new(format!(
                                    "{} Saved!",
                                    egui_phosphor::regular::CHECK_CIRCLE
                                ))
                                .color(theme::GREEN)
                                .size(13.0),
                            );
                        }
                    });
                });
            });

        if do_save {
            self.save_settings(ctx.clone());
        }

        egui::ScrollArea::vertical()
            .id_salt("settings_scroll")
            .show(ui, |ui| {
                ui.label(RichText::new("Settings").size(26.0).strong());
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Configure download behavior")
                        .size(14.0)
                        .color(theme::MUTED),
                );
                ui.add_space(16.0);

                let s = match self.settings_edit.as_mut() {
                    Some(s) => s,
                    None => {
                        ui.label(RichText::new("Loading...").color(theme::MUTED));
                        return;
                    }
                };

                // Row 1: Threads + Concurrency side-by-side
                let threads_val = s.threads_per_download;
                let concurrent_val = s.max_concurrent_downloads;
                ui.columns(2, |cols| {
                    theme::card_frame().show(&mut cols[0], |ui| {
                        ui.label(
                            RichText::new("THREADS PER DOWNLOAD")
                                .size(11.0)
                                .color(theme::MUTED)
                                .strong(),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(format!("{}", threads_val))
                                .size(32.0)
                                .strong()
                                .color(theme::GREEN),
                        );
                        ui.add_space(4.0);
                        ui.add(
                            egui::Slider::new(&mut s.threads_per_download, 1..=16)
                                .clamp_to_range(true),
                        );
                    });
                    theme::card_frame().show(&mut cols[1], |ui| {
                        ui.label(
                            RichText::new("MAX CONCURRENT")
                                .size(11.0)
                                .color(theme::MUTED)
                                .strong(),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(format!("{}", concurrent_val))
                                .size(32.0)
                                .strong()
                                .color(theme::GREEN),
                        );
                        ui.add_space(4.0);
                        ui.add(
                            egui::Slider::new(&mut s.max_concurrent_downloads, 1..=10)
                                .clamp_to_range(true),
                        );
                    });
                });
                ui.add_space(12.0);

                // Row 2: Download directory (full width)
                theme::card_frame().show(ui, |ui| {
                    ui.label(
                        RichText::new("DOWNLOAD DIRECTORY")
                            .size(11.0)
                            .color(theme::MUTED)
                            .strong(),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut s.download_dir)
                                .desired_width(ui.available_width() - 72.0)
                                .font(egui::TextStyle::Monospace),
                        );
                        if ui.button("Browse").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                s.download_dir = path.to_string_lossy().to_string();
                            }
                        }
                    });
                });
                ui.add_space(12.0);

                // Row 3: Quiet hours + System tray side-by-side
                let mut tray_changed = false;
                let mut new_tray_enabled = s.tray_enabled;
                ui.columns(2, |cols| {
                    theme::card_frame().show(&mut cols[0], |ui| {
                        ui.label(
                            RichText::new("QUIET HOURS")
                                .size(11.0)
                                .color(theme::MUTED)
                                .strong(),
                        );
                        ui.add_space(8.0);
                        ui.checkbox(&mut s.quiet_hours_enabled, "Pause during quiet hours");
                        if s.quiet_hours_enabled {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("From").color(theme::MUTED).size(12.0));
                                let mut start = s.quiet_hours_start.clone().unwrap_or_default();
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut start)
                                            .desired_width(60.0)
                                            .hint_text("00:00"),
                                    )
                                    .changed()
                                {
                                    s.quiet_hours_start =
                                        if start.is_empty() { None } else { Some(start) };
                                }
                                ui.label(RichText::new("to").color(theme::MUTED).size(12.0));
                                let mut end = s.quiet_hours_end.clone().unwrap_or_default();
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut end)
                                            .desired_width(60.0)
                                            .hint_text("08:00"),
                                    )
                                    .changed()
                                {
                                    s.quiet_hours_end =
                                        if end.is_empty() { None } else { Some(end) };
                                }
                            });
                        }
                    });
                    theme::card_frame().show(&mut cols[1], |ui| {
                        ui.label(
                            RichText::new("SYSTEM TRAY")
                                .size(11.0)
                                .color(theme::MUTED)
                                .strong(),
                        );
                        ui.add_space(8.0);
                        if ui
                            .checkbox(&mut new_tray_enabled, "Enable system tray icon")
                            .changed()
                        {
                            s.tray_enabled = new_tray_enabled;
                            tray_changed = true;
                        }
                        if new_tray_enabled {
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new("Close minimizes to tray.")
                                    .size(11.0)
                                    .color(theme::MUTED),
                            );
                        }
                    });
                });
                if tray_changed {
                    self.tray_icon = if new_tray_enabled { build_tray() } else { None };
                }
                ui.add_space(16.0);

                self.show_webdav_section(ui, ctx);
                ui.add_space(8.0);
            });
    }

    fn show_webdav_section(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.label(
            RichText::new("WEBDAV MOUNT")
                .size(12.0)
                .color(theme::MUTED)
                .strong(),
        );
        ui.add_space(8.0);

        #[cfg(not(target_os = "linux"))]
        {
            ui.label(RichText::new("WebDAV mount is Linux only.").color(theme::MUTED).size(12.0));
            return;
        }

        #[cfg(target_os = "linux")]
        {
            theme::card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Mounts Real-Debrid as /mnt/RealDebrid via rclone + systemd. Requires polkit (pkexec).")
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                    ui.add_space(8.0);

                    // Status dots
                    if let Some(ref status) = self.webdav_status.clone() {
                        ui.horizontal(|ui| {
                            status_dot(ui, "rclone", status.rclone_installed);
                            ui.add_space(12.0);
                            status_dot(
                                ui,
                                if status.service_active { "service active" } else if status.service_installed { "service stopped" } else { "service not installed" },
                                status.service_active,
                            );
                            ui.add_space(12.0);
                            status_dot(ui, if status.is_mounted { "mounted" } else { "not mounted" }, status.is_mounted);
                        });
                        ui.add_space(8.0);

                        if !status.service_installed {
                            // Setup form
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Username").size(12.0).color(theme::MUTED));
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.webdav_username)
                                        .desired_width(180.0)
                                        .hint_text("your@email.com"),
                                );
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Password").size(12.0).color(theme::MUTED));
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.webdav_password)
                                        .password(true)
                                        .desired_width(180.0)
                                        .hint_text("WebDAV password"),
                                );
                            });
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("Get credentials at real-debrid.com > Account > WebDAV password")
                                    .size(11.0)
                                    .color(theme::MUTED),
                            );
                            ui.add_space(8.0);

                            let ctx_c = ctx.clone();
                            let can_setup = !self.webdav_busy
                                && !self.webdav_username.trim().is_empty()
                                && !self.webdav_password.trim().is_empty();
                            if ui
                                .add_enabled(can_setup, egui::Button::new("Setup & Mount"))
                                .clicked()
                            {
                                let u = self.webdav_username.clone();
                                let p = self.webdav_password.clone();
                                self.run_webdav(ctx_c, move || webdav::webdav_setup(u, p).map(|_| ()), "Mounted at /mnt/RealDebrid");
                            }
                        } else {
                            ui.horizontal(|ui| {
                                let btn_size = egui::vec2(100.0, 32.0);
                                if !status.service_active {
                                    let ctx_c = ctx.clone();
                                    if ui
                                        .add_enabled(!self.webdav_busy, egui::Button::new("Start").min_size(btn_size))
                                        .clicked()
                                    {
                                        self.run_webdav(ctx_c, webdav::webdav_start, "Service started");
                                    }
                                } else {
                                    let ctx_c = ctx.clone();
                                    if ui
                                        .add_enabled(!self.webdav_busy, egui::Button::new("Stop").min_size(btn_size))
                                        .clicked()
                                    {
                                        self.run_webdav(ctx_c, webdav::webdav_stop, "Service stopped");
                                    }
                                }
                                let ctx_c = ctx.clone();
                                if ui
                                    .add_enabled(!self.webdav_busy, egui::Button::new("Uninstall").min_size(btn_size))
                                    .clicked()
                                {
                                    self.run_webdav(ctx_c, webdav::webdav_uninstall, "Uninstalled");
                                }
                                let ctx_c = ctx.clone();
                                if ui
                                    .add_enabled(!self.webdav_busy, egui::Button::new(egui_phosphor::regular::ARROWS_CLOCKWISE).min_size(egui::vec2(36.0, 32.0)))
                                    .clicked()
                                {
                                    self.load_webdav_status(ctx_c);
                                }
                            });
                        }
                    } else {
                        let ctx_c = ctx.clone();
                        if ui.button("Check status").clicked() {
                            self.load_webdav_status(ctx_c);
                        }
                    }

                    if let Some(ref msg) = self.webdav_msg.clone() {
                        ui.add_space(4.0);
                        ui.label(RichText::new(msg).color(theme::GREEN).size(12.0));
                    }

                });
            });
        }
    }
}

impl eframe::App for RdApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();

        // Tray menu events
        while let Ok(ev) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            match ev.id.0.as_str() {
                "show" => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                "quit" => {
                    std::process::exit(0);
                }
                _ => {}
            }
        }

        // Close button interception
        if !self.force_quit && ctx.input(|i| i.viewport().close_requested()) {
            let tray_on = self.settings.lock().unwrap().tray_enabled;
            if tray_on && self.tray_icon.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.show_close_dialog = true;
            }
        }

        // Auto-refresh torrent list every 5s when on Torrents page
        if self.logged_in && self.page == Page::Torrents && !self.torrents_loading {
            let should = self.last_torrent_refresh
                .map(|t| t.elapsed().as_secs() >= 5)
                .unwrap_or(true);
            if should {
                self.last_torrent_refresh = Some(std::time::Instant::now());
                self.load_torrents(ctx.clone());
            }
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.logged_in {
            self.show_login(&ctx);
        } else {
            self.show_main(&ctx);
        }

        // Unrestricted links results modal
        if !self.dl_results.is_empty() {
            egui::Modal::new(egui::Id::new("unrestrict_results")).show(&ctx, |ui| {
                ui.set_width(560.0);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(
                        format!("{} Unrestricted Links", self.dl_results.len())
                    ).size(17.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(
                            RichText::new("Clear All").color(theme::ERROR)
                        ).min_size(egui::vec2(90.0, 28.0))).clicked() {
                            self.dl_results.clear();
                        }
                        if ui.add(egui::Button::new("Queue All").min_size(egui::vec2(90.0, 28.0))).clicked() {
                            let results = self.dl_results.clone();
                            for item in &results {
                                self.enqueue_download(item.download.clone(), item.filename.clone());
                            }
                            self.dl_results.clear();
                        }
                        if ui.add(egui::Button::new("Export TXT").min_size(egui::vec2(90.0, 28.0))).clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Text", &["txt"])
                                .set_file_name("links.txt")
                                .save_file()
                            {
                                let content = self.dl_results.iter()
                                    .map(|r| r.download.as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                let _ = std::fs::write(path, content);
                            }
                        }
                    });
                });
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                let max_h = 400.0_f32.min(ctx.screen_rect().height() * 0.65);
                let results = self.dl_results.clone();
                let mut queue_idx: Option<usize> = None;
                let mut remove_idx: Option<usize> = None;
                egui::ScrollArea::vertical().max_height(max_h).show(ui, |ui| {
                    for (i, item) in results.iter().enumerate() {
                        theme::card_frame().show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_max_width(ui.available_width() - 185.0);
                                    ui.label(RichText::new(&item.filename).size(13.0).strong());
                                    ui.label(RichText::new(
                                        format!("{} - {}", item.host, format_bytes(item.filesize))
                                    ).size(11.0).color(theme::MUTED));
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add(egui::Button::new(
                                        RichText::new("Remove").color(theme::ERROR)
                                    ).min_size(egui::vec2(70.0, 26.0))).clicked() {
                                        remove_idx = Some(i);
                                    }
                                    if ui.add(egui::Button::new("Copy").min_size(egui::vec2(55.0, 26.0))).clicked() {
                                        ui.ctx().copy_text(item.download.clone());
                                    }
                                    if ui.add(egui::Button::new("Queue").min_size(egui::vec2(60.0, 26.0))).clicked() {
                                        queue_idx = Some(i);
                                    }
                                });
                            });
                        });
                        ui.add_space(4.0);
                    }
                });

                if let Some(i) = queue_idx {
                    if let Some(item) = self.dl_results.get(i) {
                        let url = item.download.clone();
                        let name = item.filename.clone();
                        self.enqueue_download(url, name);
                    }
                    self.dl_results.remove(i);
                }
                if let Some(i) = remove_idx {
                    self.dl_results.remove(i);
                }

                ui.add_space(8.0);
            });
        }

        // File selection modal for torrents
        if self.torrent_selecting {
            egui::Modal::new(egui::Id::new("torrent_selecting")).show(&ctx, |ui| {
                ui.set_width(360.0);
                ui.add_space(16.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Preparing Torrent").size(17.0).strong());
                    ui.add_space(4.0);
                    ui.label(RichText::new("Waiting for Real-Debrid to process your torrent...").size(12.0).color(theme::MUTED));
                    ui.add_space(20.0);
                    ui.add(egui::widgets::Spinner::new().size(36.0).color(theme::GREEN));
                    ui.add_space(12.0);
                    let status = self.torrent_selection_status.clone();
                    ui.label(RichText::new(&status).size(12.0).color(theme::WARNING));
                    ui.add_space(20.0);
                    if ui.add(egui::Button::new(
                        RichText::new("Cancel").color(theme::ERROR)
                    ).min_size(egui::vec2(100.0, 32.0))).clicked() {
                        self.torrent_selecting = false;
                        self.torrent_pending_files = None;
                        self.torrent_selection_status.clear();
                    }
                    ui.add_space(12.0);
                });
            });
        }

        if let Some((torrent_id, files)) = self.torrent_pending_files.clone() {
            let mut confirmed = false;
            let mut cancelled = false;
            egui::Modal::new(egui::Id::new("file_selection")).show(&ctx, |ui| {
                ui.set_width(460.0);
                ui.add_space(4.0);
                ui.label(RichText::new("Select Files to Download").size(16.0).strong());
                ui.add_space(4.0);
                ui.label(RichText::new(format!("{} files", files.len())).size(12.0).color(theme::MUTED));
                ui.add_space(10.0);

                // Select all / none row
                ui.horizontal(|ui| {
                    if ui.small_button("Select All").clicked() {
                        self.torrent_file_selection.iter_mut().for_each(|s| *s = true);
                    }
                    if ui.small_button("Select None").clicked() {
                        self.torrent_file_selection.iter_mut().for_each(|s| *s = false);
                    }
                });
                ui.add_space(6.0);

                let max_h = 320.0_f32.min(ctx.screen_rect().height() * 0.6);
                egui::ScrollArea::vertical().max_height(max_h).show(ui, |ui| {
                    for (i, file) in files.iter().enumerate() {
                        if i < self.torrent_file_selection.len() {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.torrent_file_selection[i], "");
                                ui.vertical(|ui| {
                                    let name = file.path.split('/').last().unwrap_or(&file.path);
                                    ui.label(RichText::new(name).size(12.0));
                                    ui.label(RichText::new(format_bytes(file.bytes)).size(11.0).color(theme::MUTED));
                                });
                            });
                            ui.add_space(2.0);
                        }
                    }
                });

                ui.add_space(12.0);
                let selected_count = self.torrent_file_selection.iter().filter(|&&s| s).count();
                ui.horizontal(|ui| {
                    let can_confirm = selected_count > 0;
                    if ui.add_enabled(can_confirm, egui::Button::new(
                        RichText::new(format!("Download {} file(s)", selected_count))
                            .color(egui::Color32::from_gray(20)).strong()
                    ).fill(if can_confirm { theme::GREEN } else { egui::Color32::from_gray(45) })
                     .min_size(egui::vec2(160.0, 34.0))).clicked() {
                        confirmed = true;
                    }
                    ui.add_space(8.0);
                    if ui.add(egui::Button::new("Cancel").min_size(egui::vec2(80.0, 34.0))).clicked() {
                        cancelled = true;
                    }
                });
                ui.add_space(4.0);
            });
            if confirmed {
                let ids: Vec<u32> = files.iter().enumerate()
                    .filter(|(i, _)| self.torrent_file_selection.get(*i).copied().unwrap_or(false))
                    .map(|(_, f)| f.id)
                    .collect();
                self.torrent_pending_files = None;
                self.select_files(torrent_id, ids, ctx.clone());
            }
            if cancelled {
                self.torrent_pending_files = None;
            }
        }

        // Delete confirm modal for queue items
        if let Some((item_id, dest_path)) = self.delete_confirm.clone() {
            let mut do_delete = false;
            let mut delete_file = false;
            let mut cancel = false;
            egui::Modal::new(egui::Id::new("delete_confirm")).show(&ctx, |ui| {
                ui.set_width(340.0);
                ui.add_space(4.0);
                ui.label(RichText::new("Delete Download").size(16.0).strong());
                ui.add_space(8.0);
                if let Some(ref path) = dest_path {
                    ui.label(RichText::new(format!("File: {}", path)).size(12.0).color(theme::MUTED));
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if dest_path.is_some() {
                        if ui.add(egui::Button::new(
                            RichText::new("Delete + Remove File").color(theme::ERROR)
                        ).min_size(egui::vec2(160.0, 34.0))).clicked() {
                            do_delete = true;
                            delete_file = true;
                        }
                        ui.add_space(4.0);
                    }
                    if ui.add(egui::Button::new("Remove from Queue")
                        .min_size(egui::vec2(140.0, 34.0))).clicked() {
                        do_delete = true;
                        delete_file = false;
                    }
                    ui.add_space(4.0);
                    if ui.add(egui::Button::new("Cancel")
                        .min_size(egui::vec2(60.0, 34.0))).clicked() {
                        cancel = true;
                    }
                });
                ui.add_space(4.0);
            });
            if cancel {
                self.delete_confirm = None;
            }
            if do_delete {
                if delete_file {
                    if let Some(ref path) = dest_path {
                        let _ = std::fs::remove_file(path);
                    }
                }
                if let Ok(conn) = self.db_conn.lock() {
                    let _ = queue::remove(&conn, &item_id);
                }
                self.dl_progress.remove(&item_id);
                self.delete_confirm = None;
                self.refresh_queue();
            }
        }

        // Global error popup
        if self.app_error.is_some() {
            let err_text = self.app_error.clone().unwrap_or_default();
            egui::Modal::new(egui::Id::new("app_error")).show(&ctx, |ui| {
                ui.set_width(460.0);
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(egui_phosphor::regular::WARNING_CIRCLE)
                        .size(48.0).color(theme::ERROR));
                    ui.add_space(8.0);
                    ui.label(RichText::new("Something went wrong").size(17.0).strong());
                    ui.add_space(12.0);
                    egui::Frame::new()
                        .fill(egui::Color32::from_rgba_unmultiplied(239, 68, 68, 20))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_width(410.0);
                            ui.label(
                                RichText::new(&err_text)
                                    .size(12.0)
                                    .color(egui::Color32::from_rgb(255, 160, 160))
                                    .family(egui::FontFamily::Monospace),
                            );
                        });
                    ui.add_space(16.0);
                    if ui.add(egui::Button::new(
                        RichText::new("Dismiss").size(13.0).strong()
                    ).min_size(egui::vec2(120.0, 36.0))).clicked() {
                        self.app_error = None;
                    }
                    ui.add_space(12.0);
                });
            });
        }

        if self.show_close_dialog {
            egui::Modal::new(egui::Id::new("close_dialog")).show(&ctx, |ui| {
                ui.set_width(340.0);
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Minimize to Tray?")
                        .size(16.0)
                        .strong()
                        .color(theme::TEXT),
                );
                ui.add_space(10.0);
                ui.label(
                    RichText::new(
                        "Closing the window will keep RDTool running in the system tray.",
                    )
                    .size(13.0)
                    .color(theme::MUTED),
                );
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    let btn_w = 150.0;
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Keep in Tray")
                                    .size(13.0)
                                    .color(egui::Color32::from_rgb(8, 22, 14))
                                    .strong(),
                            )
                            .fill(theme::GREEN)
                            .min_size(egui::vec2(btn_w, 36.0)),
                        )
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        self.show_close_dialog = false;
                    }
                    ui.add_space(10.0);
                    if ui
                        .add(
                            egui::Button::new(RichText::new("Quit App").size(13.0))
                                .min_size(egui::vec2(btn_w, 36.0)),
                        )
                        .clicked()
                    {
                        self.force_quit = true;
                        self.show_close_dialog = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.add_space(4.0);
            });
        }
    }
}

// ---- Helpers ----

fn field(ui: &mut Ui, label: &str, value: &str) {
    ui.label(RichText::new(label).size(12.0).color(theme::MUTED));
    ui.label(RichText::new(value).size(13.0).strong());
}

fn status_dot(ui: &mut Ui, label: &str, active: bool) {
    let color = if active { theme::GREEN } else { theme::MUTED };
    ui.horizontal(|ui| {
        ui.painter().circle_filled(
            ui.cursor().min + egui::vec2(5.0, 8.0),
            4.0,
            color,
        );
        ui.add_space(12.0);
        ui.label(RichText::new(label).size(12.0).color(color));
    });
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

async fn check_latest_version(current: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .user_agent("RDTool/0.1")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;

    let resp = client
        .get("https://api.github.com/repos/DarkXero-dev/RDTool/releases/latest")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .ok()?;

    let json: serde_json::Value = resp.json().await.ok()?;
    let tag = json.get("tag_name")?.as_str()?.trim_start_matches('v').to_string();

    if tag.as_str() > current {
        Some(tag)
    } else {
        None
    }
}

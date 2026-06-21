use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
    dl_error: Option<String>,

    // torrents page
    torrent_magnet: String,
    torrents: Vec<torrents::Torrent>,
    torrents_loading: bool,
    torrent_error: Option<String>,
    rd_downloads: Vec<torrents::RdDownload>,
    rd_tab: TorrentTab,

    // streaming page
    stream_input: String,
    stream_info: Option<streaming::StreamInfo>,
    stream_loading: bool,
    stream_error: Option<String>,

    // downloads queue
    queue: Vec<QueuedDownload>,
    dl_progress: HashMap<String, engine::ProgressEvent>,

    // settings page
    settings_edit: Option<AppSettings>,
    settings_saving: bool,
    settings_saved: bool,

    // webdav
    webdav_status: Option<webdav::WebDavStatus>,
    webdav_username: String,
    webdav_password: String,
    webdav_busy: bool,
    webdav_msg: Option<String>,
    webdav_err: Option<String>,
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
        theme::apply(&cc.egui_ctx);

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
            dl_error: None,
            torrent_magnet: String::new(),
            torrents: Vec::new(),
            torrents_loading: false,
            torrent_error: None,
            rd_downloads: Vec::new(),
            rd_tab: TorrentTab::Torrents,
            stream_input: String::new(),
            stream_info: None,
            stream_loading: false,
            stream_error: None,
            queue: Vec::new(),
            dl_progress: HashMap::new(),
            settings_edit: None,
            settings_saving: false,
            settings_saved: false,
            webdav_status: None,
            webdav_username: String::new(),
            webdav_password: String::new(),
            webdav_busy: false,
            webdav_msg: None,
            webdav_err: None,
        };

        if logged_in {
            app.fetch_user(cc.egui_ctx.clone(), false);
            app.check_update(cc.egui_ctx.clone());
            app.refresh_queue();
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
                    self.dl_error = None;
                }
                AppEvent::LinksUnrestricted(Err(e)) => {
                    self.dl_error = Some(e);
                    self.dl_loading = false;
                }
                AppEvent::TorrentAdded(Ok(_)) => {
                    self.torrent_error = None;
                    self.torrent_magnet.clear();
                    self.load_torrents(egui::Context::default());
                }
                AppEvent::TorrentAdded(Err(e)) => {
                    self.torrent_error = Some(e);
                }
                AppEvent::TorrentsLoaded(Ok(list)) => {
                    self.torrents = list;
                    self.torrents_loading = false;
                }
                AppEvent::TorrentsLoaded(Err(e)) => {
                    self.torrent_error = Some(e);
                    self.torrents_loading = false;
                }
                AppEvent::RdDownloadsLoaded(Ok(list)) => {
                    self.rd_downloads = list;
                }
                AppEvent::RdDownloadsLoaded(Err(_)) => {}
                AppEvent::StreamInfoLoaded(Ok(info)) => {
                    self.stream_info = Some(info);
                    self.stream_loading = false;
                    self.stream_error = None;
                }
                AppEvent::StreamInfoLoaded(Err(e)) => {
                    self.stream_error = Some(e);
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
                    self.webdav_err = None;
                    self.webdav_busy = false;
                    self.load_webdav_status(egui::Context::default());
                }
                AppEvent::WebDavDone(Err(e)) => {
                    self.webdav_err = Some(e);
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
                    self.dl_progress.insert(p.id.clone(), p);
                }
                DownloadEvent::Complete(c) => {
                    self.dl_progress.remove(&c.id);
                    self.refresh_queue();
                }
                DownloadEvent::Error(e) => {
                    self.dl_progress.remove(&e.id);
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
        self.dl_error = None;
        self.dl_results.clear();
        let tx = self.ev_tx.clone();
        let token = match auth::load_token() {
            Ok(t) => t,
            Err(e) => {
                self.dl_error = Some(e.to_string());
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
                self.torrent_error = Some(e.to_string());
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
                self.torrent_error = Some(e.to_string());
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
        self.stream_error = None;
        let tx = self.ev_tx.clone();
        let token = match auth::load_token() {
            Ok(t) => t,
            Err(e) => {
                self.stream_error = Some(e.to_string());
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
        self.webdav_err = None;
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
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BG))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(120.0);
                    ui.label(RichText::new("RDTool").size(36.0).strong().color(theme::GREEN));
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("Real-Debrid GUI Client")
                            .size(14.0)
                            .color(theme::MUTED),
                    );
                    ui.add_space(32.0);

                    egui::Frame::new()
                        .fill(theme::CARD)
                        .stroke(egui::Stroke::new(1.0, theme::BORDER))
                        .rounding(12.0)
                        .inner_margin(egui::Margin::same(24))
                        .show(ui, |ui| {
                            ui.set_width(340.0);
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("API Token").size(12.0).color(theme::MUTED),
                                );
                                ui.add_space(4.0);
                                let resp = ui.add(
                                    egui::TextEdit::singleline(&mut self.token_input)
                                        .password(true)
                                        .desired_width(f32::INFINITY)
                                        .hint_text("Paste your Real-Debrid API token"),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("Get it at real-debrid.com/apitoken")
                                        .size(11.0)
                                        .color(theme::MUTED),
                                );

                                if let Some(ref e) = self.login_error.clone() {
                                    ui.add_space(8.0);
                                    ui.label(RichText::new(e).size(12.0).color(theme::ERROR));
                                }

                                ui.add_space(12.0);

                                let ctx_c = ctx.clone();
                                let can_login =
                                    !self.token_input.trim().is_empty() && !self.login_loading;
                                let btn = ui.add_enabled(
                                    can_login,
                                    egui::Button::new(if self.login_loading {
                                        "Connecting..."
                                    } else {
                                        "Connect"
                                    })
                                    .min_size(egui::vec2(f32::INFINITY, 34.0)),
                                );
                                if btn.clicked()
                                    || (resp.lost_focus()
                                        && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                {
                                    self.do_login(ctx_c);
                                }
                            });
                        });
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
            .exact_width(70.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .stroke(egui::Stroke::new(1.0, theme::BORDER)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(12.0);
                    ui.label(RichText::new("RD").size(20.0).strong().color(theme::GREEN));
                    ui.add_space(16.0);

                    let nav = [
                        (egui_phosphor::regular::HOUSE, "Home", Page::Dashboard),
                        (egui_phosphor::regular::LINK, "Links", Page::Downloader),
                        (egui_phosphor::regular::MAGNET, "Torrents", Page::Torrents),
                        (
                            egui_phosphor::regular::PLAY_CIRCLE,
                            "Stream",
                            Page::Streaming,
                        ),
                        (
                            egui_phosphor::regular::DOWNLOAD_SIMPLE,
                            "Queue",
                            Page::Downloads,
                        ),
                        (egui_phosphor::regular::GEAR, "Settings", Page::Settings),
                    ];

                    for (icon, label, p) in nav {
                        let active = self.page == p;
                        let icon_text = RichText::new(icon).size(22.0).color(if active {
                            theme::GREEN
                        } else {
                            theme::MUTED
                        });
                        let resp = ui.selectable_label(active, icon_text);
                        if resp.clicked() {
                            self.page = p;
                            self.on_page_enter(ctx);
                        }
                        ui.label(
                            RichText::new(label)
                                .size(10.0)
                                .color(if active { theme::GREEN } else { theme::MUTED }),
                        );
                        ui.add_space(6.0);
                    }
                });

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    if ui
                        .small_button(RichText::new("Logout").color(theme::MUTED))
                        .clicked()
                    {
                        let _ = auth::clear_token();
                        self.logged_in = false;
                        self.user = None;
                    }
                    if let Some(ref u) = self.user {
                        let name = if u.username.len() > 8 {
                            &u.username[..8]
                        } else {
                            &u.username
                        };
                        ui.label(RichText::new(name).size(10.0).color(theme::MUTED));
                    }
                    ui.add_space(4.0);
                });
            });

        // Main content
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::same(20)))
            .show(ctx, |ui| {
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
            Page::Torrents => {
                self.load_torrents(ctx.clone());
                self.load_rd_downloads(ctx.clone());
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
        ui.label(RichText::new("Dashboard").size(22.0).strong());
        ui.add_space(4.0);
        ui.label(RichText::new("Account overview").size(13.0).color(theme::MUTED));
        ui.add_space(20.0);

        if let Some(ref u) = self.user.clone() {
            let days_left = u.premium;
            let exp = u.expiration.as_deref().unwrap_or("N/A");

            egui::Grid::new("user_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    field(ui, "Username", &u.username);
                    ui.end_row();
                    field(ui, "Email", &u.email);
                    ui.end_row();
                    field(ui, "Type", &u.account_type);
                    ui.end_row();
                    field(ui, "Premium", &format!("{days_left} days remaining"));
                    ui.end_row();
                    field(ui, "Expires", exp);
                    ui.end_row();
                    field(ui, "Points", &u.points.to_string());
                    ui.end_row();
                });
        } else {
            ui.label(RichText::new("Loading account info...").color(theme::MUTED));
        }
    }

    fn show_downloader(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.label(RichText::new("Link Unrestrictor").size(22.0).strong());
        ui.add_space(4.0);
        ui.label(
            RichText::new("Paste premium links, one per line")
                .size(13.0)
                .color(theme::MUTED),
        );
        ui.add_space(16.0);

        ui.label(RichText::new("Links").size(12.0).color(theme::MUTED));
        ui.add_space(4.0);
        ui.add(
            egui::TextEdit::multiline(&mut self.dl_input)
                .desired_rows(5)
                .desired_width(f32::INFINITY)
                .hint_text("https://example.com/file.mkv\nhttps://..."),
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            let busy = self.dl_loading;
            if ui
                .add_enabled(
                    !busy && !self.dl_input.trim().is_empty(),
                    egui::Button::new(if busy { "Unrestricting..." } else { "Unrestrict" }),
                )
                .clicked()
            {
                self.unrestrict_links(ctx.clone());
            }

            if !self.dl_results.is_empty() {
                if ui.button("Export to TXT").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Text", &["txt"])
                        .set_file_name("links.txt")
                        .save_file()
                    {
                        let content = self
                            .dl_results
                            .iter()
                            .map(|r| r.download.as_str())
                            .collect::<Vec<_>>()
                            .join("\n");
                        let _ = std::fs::write(path, content);
                    }
                }
            }
        });

        if let Some(ref e) = self.dl_error.clone() {
            ui.add_space(8.0);
            ui.label(RichText::new(e).color(theme::ERROR).size(12.0));
        }

        if !self.dl_results.is_empty() {
            ui.add_space(16.0);
            ui.label(
                RichText::new(format!("{} unrestricted links", self.dl_results.len()))
                    .size(13.0)
                    .color(theme::MUTED),
            );
            ui.add_space(8.0);

            let results = self.dl_results.clone();
            for item in &results {
                theme::card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(&item.filename)
                                    .size(13.0)
                                    .strong(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{} - {}",
                                    item.host,
                                    format_bytes(item.filesize)
                                ))
                                .size(11.0)
                                .color(theme::MUTED),
                            );
                        });

                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui.small_button("Queue").clicked() {
                                    self.enqueue_download(
                                        item.download.clone(),
                                        item.filename.clone(),
                                    );
                                }
                                if ui.small_button("Copy").clicked() {
                                    ui.ctx().copy_text(item.download.clone());
                                }
                            },
                        );
                    });
                });
                ui.add_space(6.0);
            }
        }
    }

    fn show_torrents(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.label(RichText::new("Torrents").size(22.0).strong());
        ui.add_space(16.0);

        // Add magnet / torrent file
        theme::card_frame().show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("Add Torrent").size(13.0).strong());
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.torrent_magnet)
                            .desired_width(f32::INFINITY)
                            .hint_text("magnet:?xt=urn:btih:..."),
                    );
                    let ctx_c = ctx.clone();
                    if ui
                        .button("Add Magnet")
                        .clicked()
                    {
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

        if let Some(ref e) = self.torrent_error.clone() {
            ui.add_space(8.0);
            ui.label(RichText::new(e).color(theme::ERROR).size(12.0));
        }

        ui.add_space(16.0);

        // Tabs
        ui.horizontal(|ui| {
            if ui
                .selectable_label(self.rd_tab == TorrentTab::Torrents, "My Torrents")
                .clicked()
            {
                self.rd_tab = TorrentTab::Torrents;
                self.load_torrents(ctx.clone());
            }
            if ui
                .selectable_label(self.rd_tab == TorrentTab::Downloads, "RD Downloads")
                .clicked()
            {
                self.rd_tab = TorrentTab::Downloads;
                self.load_rd_downloads(ctx.clone());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ctx_c = ctx.clone();
                if ui.small_button(egui_phosphor::regular::ARROWS_CLOCKWISE).clicked() {
                    match self.rd_tab {
                        TorrentTab::Torrents => self.load_torrents(ctx_c),
                        TorrentTab::Downloads => self.load_rd_downloads(ctx_c),
                    }
                }
            });
        });
        ui.add_space(8.0);

        match self.rd_tab {
            TorrentTab::Torrents => {
                if self.torrents_loading {
                    ui.label(RichText::new("Loading...").color(theme::MUTED));
                } else if self.torrents.is_empty() {
                    ui.label(
                        RichText::new("No torrents found")
                            .color(theme::MUTED)
                            .size(13.0),
                    );
                } else {
                    let torrents = self.torrents.clone();
                    for t in &torrents {
                        theme::card_frame().show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&t.filename).size(13.0).strong());
                                    let status_color = match t.status.as_str() {
                                        "downloaded" => theme::GREEN,
                                        "downloading" => theme::WARNING,
                                        _ => theme::MUTED,
                                    };
                                    ui.label(
                                        RichText::new(format!(
                                            "{} - {:.0}% - {}",
                                            t.status,
                                            t.progress,
                                            format_bytes(t.bytes)
                                        ))
                                        .size(11.0)
                                        .color(status_color),
                                    );
                                    if t.status == "downloading" {
                                        ui.add(egui::ProgressBar::new(t.progress as f32 / 100.0));
                                    }
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if t.status == "downloaded" && !t.links.is_empty() {
                                            if ui.small_button("Queue all").clicked() {
                                                for link in &t.links {
                                                    let name = link
                                                        .split('/')
                                                        .last()
                                                        .unwrap_or("file")
                                                        .to_string();
                                                    self.enqueue_download(link.clone(), name);
                                                }
                                            }
                                        }
                                    },
                                );
                            });
                        });
                        ui.add_space(6.0);
                    }
                }
            }
            TorrentTab::Downloads => {
                let downloads = self.rd_downloads.clone();
                if downloads.is_empty() {
                    ui.label(
                        RichText::new("No RD downloads found")
                            .color(theme::MUTED)
                            .size(13.0),
                    );
                } else {
                    for d in &downloads {
                        theme::card_frame().show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&d.filename).size(13.0).strong());
                                    ui.label(
                                        RichText::new(format!(
                                            "{} - {}",
                                            d.host,
                                            format_bytes(d.filesize)
                                        ))
                                        .size(11.0)
                                        .color(theme::MUTED),
                                    );
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("Queue").clicked() {
                                            self.enqueue_download(
                                                d.download.clone(),
                                                d.filename.clone(),
                                            );
                                        }
                                        if ui.small_button("Copy").clicked() {
                                            ui.ctx().copy_text(d.download.clone());
                                        }
                                    },
                                );
                            });
                        });
                        ui.add_space(6.0);
                    }
                }
            }
        }
    }

    fn show_streaming(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.label(RichText::new("Streaming").size(22.0).strong());
        ui.add_space(4.0);
        ui.label(
            RichText::new("Generate stream transcodes for a download ID")
                .size(13.0)
                .color(theme::MUTED),
        );
        ui.add_space(16.0);

        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.stream_input)
                    .desired_width(300.0)
                    .hint_text("Download ID (from RD Downloads)"),
            );
            let ctx_c = ctx.clone();
            if ui
                .add_enabled(
                    !self.stream_loading && !self.stream_input.trim().is_empty(),
                    egui::Button::new(if self.stream_loading {
                        "Loading..."
                    } else {
                        "Get Streams"
                    }),
                )
                .clicked()
            {
                self.get_stream_info(ctx_c);
            }
        });

        if let Some(ref e) = self.stream_error.clone() {
            ui.add_space(8.0);
            ui.label(RichText::new(e).color(theme::ERROR).size(12.0));
        }

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
            ui.label(RichText::new("Download Queue").size(22.0).strong());
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
        let mut pause_id: Option<String> = None;
        let mut cancel_id: Option<String> = None;
        let mut remove_id: Option<String> = None;

        for item in &queue {
            theme::card_frame().show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&item.filename).size(13.0).strong());
                            let status_color = match item.status {
                                DownloadStatus::Active => theme::GREEN,
                                DownloadStatus::Completed => theme::GREEN,
                                DownloadStatus::Failed => theme::ERROR,
                                DownloadStatus::Paused => theme::WARNING,
                                _ => theme::MUTED,
                            };
                            ui.label(
                                RichText::new(format!(
                                    "{:?} - {}",
                                    item.status,
                                    format_bytes(item.bytes_done)
                                ))
                                .size(11.0)
                                .color(status_color),
                            );
                        });

                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| match item.status {
                                DownloadStatus::Queued | DownloadStatus::Scheduled => {
                                    if ui.small_button("Start").clicked() {
                                        start_id = Some(item.id.clone());
                                    }
                                    if ui.small_button("Remove").clicked() {
                                        remove_id = Some(item.id.clone());
                                    }
                                }
                                DownloadStatus::Active => {
                                    if ui.small_button("Cancel").clicked() {
                                        cancel_id = Some(item.id.clone());
                                    }
                                }
                                DownloadStatus::Paused => {
                                    if ui.small_button("Resume").clicked() {
                                        start_id = Some(item.id.clone());
                                    }
                                    if ui.small_button("Cancel").clicked() {
                                        cancel_id = Some(item.id.clone());
                                    }
                                }
                                DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled => {
                                    if ui.small_button("Remove").clicked() {
                                        remove_id = Some(item.id.clone());
                                    }
                                }
                            },
                        );
                    });

                    // Progress bar
                    if item.status == DownloadStatus::Active {
                        if let Some(prog) = progress.get(&item.id) {
                            let fraction = prog
                                .total_bytes
                                .map(|t| if t > 0 { prog.bytes_done as f32 / t as f32 } else { 0.0 })
                                .unwrap_or(0.0)
                                .clamp(0.0, 1.0);
                            ui.add_space(4.0);
                            ui.add(
                                egui::ProgressBar::new(fraction)
                                    .text(format!(
                                        "{} / {} - {}/s",
                                        format_bytes(prog.bytes_done),
                                        prog.total_bytes
                                            .map(format_bytes)
                                            .unwrap_or_else(|| "?".into()),
                                        format_bytes(prog.speed_bps)
                                    ))
                                    .desired_width(f32::INFINITY),
                            );
                        }
                    }
                });
            });
            ui.add_space(6.0);
        }

        // Apply deferred actions
        if let Some(id) = start_id {
            self.start_download_item(id, ctx.clone());
        }
        if let Some(id) = pause_id {
            if let Ok(conn) = self.db_conn.lock() {
                let _ = queue::update_status(&conn, &id, DownloadStatus::Paused);
            }
            self.refresh_queue();
        }
        if let Some(id) = cancel_id {
            if let Ok(conn) = self.db_conn.lock() {
                let _ = queue::update_status(&conn, &id, DownloadStatus::Cancelled);
            }
            self.dl_progress.remove(&id);
            self.refresh_queue();
        }
        if let Some(id) = remove_id {
            if let Ok(conn) = self.db_conn.lock() {
                let _ = queue::remove(&conn, &id);
            }
            self.dl_progress.remove(&id);
            self.refresh_queue();
        }
    }

    fn show_settings_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.label(RichText::new("Settings").size(22.0).strong());
        ui.add_space(4.0);
        ui.label(
            RichText::new("Configure download behavior")
                .size(13.0)
                .color(theme::MUTED),
        );
        ui.add_space(20.0);

        let s = match self.settings_edit.as_mut() {
            Some(s) => s,
            None => {
                ui.label(RichText::new("Loading...").color(theme::MUTED));
                return;
            }
        };

        // Downloads section
        ui.label(
            RichText::new("DOWNLOADS")
                .size(11.0)
                .color(theme::MUTED)
                .strong(),
        );
        ui.add_space(8.0);

        theme::card_frame().show(ui, |ui| {
            egui::Grid::new("dl_settings")
                .num_columns(2)
                .spacing([16.0, 12.0])
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("Threads per download")
                            .size(13.0),
                    );
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Slider::new(&mut s.threads_per_download, 1..=16)
                                .clamp_to_range(true),
                        );
                    });
                    ui.end_row();

                    ui.label(RichText::new("Max concurrent downloads").size(13.0));
                    ui.add(
                        egui::Slider::new(&mut s.max_concurrent_downloads, 1..=10)
                            .clamp_to_range(true),
                    );
                    ui.end_row();

                    ui.label(RichText::new("Download directory").size(13.0));
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut s.download_dir)
                                .desired_width(250.0)
                                .font(egui::TextStyle::Monospace),
                        );
                        if ui.small_button("Browse").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                s.download_dir = path.to_string_lossy().to_string();
                            }
                        }
                    });
                    ui.end_row();
                });
        });

        ui.add_space(16.0);

        // Quiet hours section
        ui.label(
            RichText::new("QUIET HOURS")
                .size(11.0)
                .color(theme::MUTED)
                .strong(),
        );
        ui.add_space(8.0);

        theme::card_frame().show(ui, |ui| {
            ui.vertical(|ui| {
                ui.checkbox(&mut s.quiet_hours_enabled, "Pause downloads during quiet hours");

                if s.quiet_hours_enabled {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Start").color(theme::MUTED).size(12.0));
                        let mut start = s.quiet_hours_start.clone().unwrap_or_default();
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut start)
                                    .desired_width(70.0)
                                    .hint_text("00:00"),
                            )
                            .changed()
                        {
                            s.quiet_hours_start = if start.is_empty() { None } else { Some(start) };
                        }
                        ui.add_space(16.0);
                        ui.label(RichText::new("End").color(theme::MUTED).size(12.0));
                        let mut end = s.quiet_hours_end.clone().unwrap_or_default();
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut end)
                                    .desired_width(70.0)
                                    .hint_text("08:00"),
                            )
                            .changed()
                        {
                            s.quiet_hours_end = if end.is_empty() { None } else { Some(end) };
                        }
                    });
                }
            });
        });

        ui.add_space(16.0);

        // WebDAV section (Linux only)
        self.show_webdav_section(ui, ctx);

        ui.add_space(20.0);

        // Save button
        ui.horizontal(|ui| {
            let ctx_c = ctx.clone();
            if ui
                .add_enabled(
                    !self.settings_saving,
                    egui::Button::new(if self.settings_saving {
                        "Saving..."
                    } else {
                        "Save Settings"
                    }),
                )
                .clicked()
            {
                self.save_settings(ctx_c);
            }
            if self.settings_saved {
                ui.label(RichText::new("Saved!").color(theme::GREEN).size(13.0));
            }
        });
    }

    fn show_webdav_section(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.label(
            RichText::new("WEBDAV MOUNT")
                .size(11.0)
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
                                if !status.service_active {
                                    let ctx_c = ctx.clone();
                                    if ui
                                        .add_enabled(!self.webdav_busy, egui::Button::new("Start"))
                                        .clicked()
                                    {
                                        self.run_webdav(ctx_c, webdav::webdav_start, "Service started");
                                    }
                                } else {
                                    let ctx_c = ctx.clone();
                                    if ui
                                        .add_enabled(!self.webdav_busy, egui::Button::new("Stop"))
                                        .clicked()
                                    {
                                        self.run_webdav(ctx_c, webdav::webdav_stop, "Service stopped");
                                    }
                                }
                                let ctx_c = ctx.clone();
                                if ui
                                    .add_enabled(!self.webdav_busy, egui::Button::new("Uninstall"))
                                    .clicked()
                                {
                                    self.run_webdav(ctx_c, webdav::webdav_uninstall, "Uninstalled");
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
                    if let Some(ref err) = self.webdav_err.clone() {
                        ui.add_space(4.0);
                        ui.label(RichText::new(err).color(theme::ERROR).size(12.0));
                    }

                    ui.add_space(4.0);
                    let ctx_c = ctx.clone();
                    if ui.small_button(egui_phosphor::regular::ARROWS_CLOCKWISE).clicked() {
                        self.load_webdav_status(ctx_c);
                    }
                });
            });
        }
    }
}

impl eframe::App for RdApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.logged_in {
            self.show_login(&ctx);
        } else {
            self.show_main(&ctx);
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

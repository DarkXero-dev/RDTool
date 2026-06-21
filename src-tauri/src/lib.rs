mod api;
mod auth;
mod db;
mod downloads;
mod settings;
mod webdav;
pub mod ui;

use std::sync::{Arc, Mutex};

pub fn run() {
    // tray-icon uses GTK AppIndicator on Linux; eframe's glow renderer
    // does not initialize GTK, so we must do it before any tray use.
    #[cfg(target_os = "linux")]
    gtk::init().expect("gtk init");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    let _guard = rt.enter();
    let handle = tokio::runtime::Handle::current();

    let settings = Arc::new(Mutex::new(settings::load_settings()));
    let db_conn = Arc::new(Mutex::new(db::open().expect("open database")));

    downloads::scheduler::start(Arc::clone(&settings));

    let (ev_tx, ev_rx) = std::sync::mpsc::channel::<ui::app::AppEvent>();
    let (dl_tx, dl_rx) = std::sync::mpsc::channel::<downloads::engine::DownloadEvent>();

    let settings_c = Arc::clone(&settings);
    let db_c = Arc::clone(&db_conn);
    let handle_c = handle.clone();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 750.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("RDTool"),
        ..Default::default()
    };

    eframe::run_native(
        "RDTool",
        native_options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::new(ui::app::RdApp::new(
                cc, handle_c, settings_c, db_c, ev_tx, ev_rx, dl_tx, dl_rx,
            )))
        }),
    )
    .expect("eframe run");
}

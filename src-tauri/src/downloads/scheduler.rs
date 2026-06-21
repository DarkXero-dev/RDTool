use std::sync::{Arc, Mutex};
use tokio::time::{interval, Duration};

use crate::db;
use crate::downloads::queue;
use crate::settings::AppSettings;

pub fn start(settings: Arc<Mutex<AppSettings>>) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = interval(Duration::from_secs(30));
        loop {
            ticker.tick().await;
            if let Err(e) = tick(&settings) {
                eprintln!("scheduler tick error: {e}");
            }
        }
    });
}

fn tick(settings: &Arc<Mutex<AppSettings>>) -> anyhow::Result<()> {
    let s = settings.lock().unwrap().clone();
    if s.quiet_hours_enabled {
        if in_quiet_hours(&s) {
            return Ok(());
        }
    }

    let conn = db::open()?;
    let ready = queue::get_queued_ready(&conn)?;

    let active_count: usize = queue::get_all(&conn)?
        .iter()
        .filter(|d| d.status == queue::DownloadStatus::Active)
        .count();

    let slots = (s.max_concurrent_downloads as usize).saturating_sub(active_count);
    if slots == 0 {
        return Ok(());
    }

    for item in ready.into_iter().take(slots) {
        queue::update_status(&conn, &item.id, queue::DownloadStatus::Active)?;
    }

    Ok(())
}

fn in_quiet_hours(s: &AppSettings) -> bool {
    let Some(ref start) = s.quiet_hours_start else { return false; };
    let Some(ref end) = s.quiet_hours_end else { return false; };

    let now = chrono::Local::now();
    let current = format!("{:02}:{:02}", now.hour(), now.minute());

    if start <= end {
        current >= *start && current < *end
    } else {
        current >= *start || current < *end
    }
}

trait TimeExt {
    fn hour(&self) -> u32;
    fn minute(&self) -> u32;
}

impl TimeExt for chrono::DateTime<chrono::Local> {
    fn hour(&self) -> u32 {
        chrono::Timelike::hour(self)
    }
    fn minute(&self) -> u32 {
        chrono::Timelike::minute(self)
    }
}

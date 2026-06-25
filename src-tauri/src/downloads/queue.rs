use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Queued,
    Scheduled,
    Active,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl DownloadStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Scheduled => "scheduled",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "scheduled" => Self::Scheduled,
            "active" => Self::Active,
            "paused" => Self::Paused,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Queued,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DownloadOpts {
    pub threads: Option<u8>,
    pub scheduled_at: Option<String>,
    pub priority: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QueuedDownload {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub dest_path: String,
    pub status: DownloadStatus,
    pub priority: u8,
    pub threads: u8,
    pub scheduled_at: Option<String>,
    pub total_bytes: Option<u64>,
    pub bytes_done: u64,
    pub error_msg: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub fn enqueue(
    conn: &Connection,
    url: String,
    filename: String,
    dest_path: String,
    opts: DownloadOpts,
    default_threads: u8,
) -> Result<QueuedDownload> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let threads = opts.threads.unwrap_or(default_threads);
    let status = if opts.scheduled_at.is_some() {
        DownloadStatus::Scheduled
    } else {
        DownloadStatus::Queued
    };

    conn.execute(
        "INSERT INTO downloads (id, url, filename, dest_path, status, priority, threads, scheduled_at, bytes_done, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?9)",
        params![
            id, url, filename, dest_path,
            status.as_str(), opts.priority, threads,
            opts.scheduled_at, now
        ],
    )?;

    Ok(QueuedDownload {
        id,
        url: url.to_string(),
        filename: filename.to_string(),
        dest_path: dest_path.to_string(),
        status,
        priority: opts.priority,
        threads,
        scheduled_at: opts.scheduled_at,
        total_bytes: None,
        bytes_done: 0,
        error_msg: None,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub fn get_all(conn: &Connection) -> Result<Vec<QueuedDownload>> {
    let mut stmt = conn.prepare(
        "SELECT id, url, filename, dest_path, status, priority, threads, scheduled_at,
                total_bytes, bytes_done, error_msg, created_at, updated_at
         FROM downloads
         WHERE status != 'cancelled'
         ORDER BY priority DESC, created_at ASC",
    )?;

    let items = stmt.query_map([], |row| {
        Ok(QueuedDownload {
            id: row.get(0)?,
            url: row.get(1)?,
            filename: row.get(2)?,
            dest_path: row.get(3)?,
            status: DownloadStatus::from_str(&row.get::<_, String>(4)?),
            priority: row.get::<_, u8>(5)?,
            threads: row.get::<_, u8>(6)?,
            scheduled_at: row.get(7)?,
            total_bytes: row.get(8)?,
            bytes_done: row.get::<_, u64>(9)?,
            error_msg: row.get(10)?,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
        })
    })?
    .filter_map(|r| r.ok())
    .collect();

    Ok(items)
}

pub fn update_status(conn: &Connection, id: &str, status: DownloadStatus) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE downloads SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![status.as_str(), now, id],
    )?;
    Ok(())
}

pub fn update_progress(conn: &Connection, id: &str, bytes_done: u64, total_bytes: Option<u64>) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE downloads SET bytes_done = ?1, total_bytes = ?2, updated_at = ?3 WHERE id = ?4",
        params![bytes_done, total_bytes, now, id],
    )?;
    Ok(())
}

pub fn set_error(conn: &Connection, id: &str, msg: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE downloads SET status = 'failed', error_msg = ?1, updated_at = ?2 WHERE id = ?3",
        params![msg, now, id],
    )?;
    Ok(())
}

pub fn remove(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM downloads WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn schedule_all_queued(conn: &Connection, at: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE downloads SET status = 'scheduled', scheduled_at = ?1, updated_at = ?2 WHERE status = 'queued'",
        params![at, now],
    )?;
    Ok(())
}

pub fn schedule(conn: &Connection, id: &str, at: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE downloads SET status = 'scheduled', scheduled_at = ?1, updated_at = ?2 WHERE id = ?3",
        params![at, now, id],
    )?;
    Ok(())
}

pub fn clear_queue(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT dest_path FROM downloads WHERE status NOT IN ('active')",
    )?;
    let paths: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    conn.execute("DELETE FROM downloads WHERE status NOT IN ('active')", [])?;
    Ok(paths)
}

pub fn reset_active_to_queued(conn: &Connection) -> Result<()> {
    conn.execute("UPDATE downloads SET status = 'queued' WHERE status = 'active'", [])?;
    Ok(())
}

pub fn get_queued_ready(conn: &Connection) -> Result<Vec<QueuedDownload>> {
    let now = Utc::now().to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT id, url, filename, dest_path, status, priority, threads, scheduled_at,
                total_bytes, bytes_done, error_msg, created_at, updated_at
         FROM downloads
         WHERE status = 'scheduled' AND scheduled_at <= ?1
         ORDER BY priority DESC, created_at ASC",
    )?;

    let items = stmt.query_map(params![now], |row| {
        Ok(QueuedDownload {
            id: row.get(0)?,
            url: row.get(1)?,
            filename: row.get(2)?,
            dest_path: row.get(3)?,
            status: DownloadStatus::from_str(&row.get::<_, String>(4)?),
            priority: row.get::<_, u8>(5)?,
            threads: row.get::<_, u8>(6)?,
            scheduled_at: row.get(7)?,
            total_bytes: row.get(8)?,
            bytes_done: row.get::<_, u64>(9)?,
            error_msg: row.get(10)?,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
        })
    })?
    .filter_map(|r| r.ok())
    .collect();

    Ok(items)
}

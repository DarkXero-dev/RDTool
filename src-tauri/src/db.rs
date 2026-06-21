use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;

pub fn db_path() -> Result<PathBuf> {
    let base = dirs::data_dir().ok_or_else(|| anyhow::anyhow!("no data dir"))?;
    let dir = base.join("rdtool");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("downloads.db"))
}

pub fn init_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS downloads (
            id          TEXT PRIMARY KEY,
            url         TEXT NOT NULL,
            filename    TEXT NOT NULL,
            dest_path   TEXT NOT NULL,
            status      TEXT NOT NULL DEFAULT 'queued',
            priority    INTEGER NOT NULL DEFAULT 0,
            threads     INTEGER NOT NULL DEFAULT 4,
            scheduled_at TEXT,
            total_bytes  INTEGER,
            bytes_done   INTEGER NOT NULL DEFAULT 0,
            error_msg   TEXT,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );",
    )?;
    Ok(())
}

pub fn open() -> Result<Connection> {
    let path = db_path()?;
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    init_db(&conn)?;
    Ok(conn)
}

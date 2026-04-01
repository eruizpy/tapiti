use crate::obd::Reading;
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    pub async fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS readings (
                id       INTEGER PRIMARY KEY AUTOINCREMENT,
                session  TEXT    NOT NULL DEFAULT (strftime('%Y%m%d_%H%M%S', 'now')),
                pid      TEXT    NOT NULL,
                value    REAL    NOT NULL,
                unit     TEXT    NOT NULL,
                ts_ms    INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_readings_session ON readings(session);
            CREATE INDEX IF NOT EXISTS idx_readings_pid     ON readings(pid, ts_ms);
        ",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn insert(&self, r: &Reading) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO readings (pid, value, unit, ts_ms) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![r.pid, r.value, r.unit, r.ts_ms as i64],
        )?;
        Ok(())
    }

    pub async fn export_csv(&self, session: &str) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT ts_ms, pid, value, unit FROM readings WHERE session = ?1 ORDER BY ts_ms",
        )?;
        let mut csv = String::from("ts_ms,pid,value,unit\n");
        let rows = stmt.query_map(rusqlite::params![session], |row| {
            Ok(format!(
                "{},{},{},{}\n",
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            csv.push_str(&row?);
        }
        Ok(csv)
    }

    pub async fn latest_session(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT session FROM readings ORDER BY ts_ms DESC LIMIT 1")?;
        let session: Option<String> = stmt.query_row([], |row| row.get(0)).optional()?;
        Ok(session)
    }
}

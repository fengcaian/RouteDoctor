use std::sync::Arc;
use tokio::sync::Mutex;
use rusqlite::{Connection, params};
use tauri::Manager;
use crate::models::ping::PingResult;
use crate::error::{AppError, AppResult};

/// Database state wrapper
pub struct DbState(pub Arc<Mutex<Connection>>);

/// Initialize database
pub fn init_database(app_handle: &tauri::AppHandle) -> AppResult<()> {
    let app_dir = app_handle.path().app_data_dir()
        .map_err(|e| AppError::Internal(format!("Failed to get app data dir: {}", e)))?;

    // Create directory if it doesn't exist
    std::fs::create_dir_all(&app_dir)?;

    let db_path = app_dir.join("history.db");
    let conn = Connection::open(&db_path)?;

    // Create tables
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ping_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            target TEXT NOT NULL,
            test_type TEXT NOT NULL,
            start_time INTEGER NOT NULL,
            end_time INTEGER NOT NULL,
            data TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS ping_results (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id INTEGER,
            seq INTEGER NOT NULL,
            target TEXT NOT NULL,
            ip TEXT NOT NULL,
            latency_ms REAL,
            is_timeout INTEGER NOT NULL,
            timestamp INTEGER NOT NULL,
            FOREIGN KEY (session_id) REFERENCES ping_history(id)
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_ping_results_target ON ping_results(target)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_ping_results_timestamp ON ping_results(timestamp)",
        [],
    )?;

    // Store connection in app state
    app_handle.manage(DbState(Arc::new(Mutex::new(conn))));

    Ok(())
}

/// Get history records
pub async fn get_history(
    app_handle: &tauri::AppHandle,
    target: Option<String>,
    test_type: Option<String>,
    limit: Option<u32>,
) -> AppResult<Vec<crate::commands::history::HistoryRecord>> {
    let db = app_handle.state::<DbState>();
    let conn = db.0.lock().await;

    let limit = limit.unwrap_or(100);
    let mut records = Vec::new();

    // Build query and parameters based on filters
    let (sql, params): (String, Vec<Box<dyn rusqlite::ToSql>>) = match (&target, &test_type) {
        (Some(t), Some(tt)) => (
            "SELECT id, target, test_type, start_time, end_time, data FROM ping_history
             WHERE target = ?1 AND test_type = ?2 ORDER BY start_time DESC LIMIT ?3".to_string(),
            vec![Box::new(t.clone()), Box::new(tt.clone()), Box::new(limit)]
        ),
        (Some(t), None) => (
            "SELECT id, target, test_type, start_time, end_time, data FROM ping_history
             WHERE target = ?1 ORDER BY start_time DESC LIMIT ?2".to_string(),
            vec![Box::new(t.clone()), Box::new(limit)]
        ),
        (None, Some(tt)) => (
            "SELECT id, target, test_type, start_time, end_time, data FROM ping_history
             WHERE test_type = ?1 ORDER BY start_time DESC LIMIT ?2".to_string(),
            vec![Box::new(tt.clone()), Box::new(limit)]
        ),
        (None, None) => (
            "SELECT id, target, test_type, start_time, end_time, data FROM ping_history
             ORDER BY start_time DESC LIMIT ?1".to_string(),
            vec![Box::new(limit)]
        ),
    };

    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_refs.as_slice(), |row| {
        Ok(crate::commands::history::HistoryRecord {
            id: row.get(0)?,
            target: row.get(1)?,
            test_type: row.get(2)?,
            start_time: row.get(3)?,
            end_time: row.get(4)?,
            data: row.get(5)?,
        })
    })?;

    for row in rows {
        records.push(row?);
    }

    Ok(records)
}

/// Save ping result to history
pub async fn save_ping_result(
    app_handle: &tauri::AppHandle,
    result: PingResult,
) -> AppResult<()> {
    let db = app_handle.state::<DbState>();
    let conn = db.0.lock().await;

    conn.execute(
        "INSERT INTO ping_results (session_id, seq, target, ip, latency_ms, is_timeout, timestamp)
         VALUES (NULL, ?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            result.seq,
            result.target,
            result.ip,
            result.latency_ms,
            result.is_timeout as i32,
            result.timestamp
        ],
    )?;

    Ok(())
}

/// Create a new ping session
pub async fn create_ping_session(
    app_handle: &tauri::AppHandle,
    target: &str,
    test_type: &str,
) -> AppResult<i64> {
    let db = app_handle.state::<DbState>();
    let conn = db.0.lock().await;

    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        "INSERT INTO ping_history (target, test_type, start_time, end_time, data)
         VALUES (?1, ?2, ?3, ?4, '')",
        params![target, test_type, now, now],
    )?;

    Ok(conn.last_insert_rowid())
}

/// Update ping session end time and data
pub async fn update_ping_session(
    app_handle: &tauri::AppHandle,
    session_id: i64,
    end_time: i64,
    data: &str,
) -> AppResult<()> {
    let db = app_handle.state::<DbState>();
    let conn = db.0.lock().await;

    conn.execute(
        "UPDATE ping_history SET end_time = ?1, data = ?2 WHERE id = ?3",
        params![end_time, data, session_id],
    )?;

    Ok(())
}

/// Delete old history records (cleanup)
pub async fn cleanup_old_history(
    app_handle: &tauri::AppHandle,
    days_to_keep: u32,
) -> AppResult<()> {
    let db = app_handle.state::<DbState>();
    let conn = db.0.lock().await;

    let cutoff_time = chrono::Utc::now().timestamp_millis()
        - (days_to_keep as i64 * 24 * 60 * 60 * 1000);

    conn.execute(
        "DELETE FROM ping_history WHERE start_time < ?1",
        params![cutoff_time],
    )?;

    conn.execute(
        "DELETE FROM ping_results WHERE timestamp < ?1",
        params![cutoff_time],
    )?;

    Ok(())
}

/// 清除所有历史记录
pub async fn clear_all_history(
    app_handle: &tauri::AppHandle,
) -> AppResult<()> {
    let db = app_handle.state::<DbState>();
    let conn = db.0.lock().await;

    conn.execute("DELETE FROM ping_results", [])?;
    conn.execute("DELETE FROM ping_history", [])?;

    Ok(())
}

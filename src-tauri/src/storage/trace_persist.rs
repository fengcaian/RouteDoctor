// 路径监控会话的持久化层
//
// 设计：
// - 30 跳 × 0.5Hz ≈ 15 写/秒，每次都直写 SQLite 浪费太多 fsync。
// - 引入一个独立的 writer task，所有 ping 任务通过 mpsc channel 把样本推过来，
//   writer 每 500ms（或样本数到阈值）做一次批量 INSERT，事务提交，性能问题。
// - session 状态：started_at + status='running' 立即落盘，确保即使进程崩溃也能在
//   下次启动时通过 `status='running'` 检测到"未正常关闭的会话"，将其标记为 crashed。

use std::time::Duration;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::Manager;
use tokio::sync::mpsc;
use crate::error::AppResult;
use crate::storage::database::DbState;

const FLUSH_INTERVAL_MS: u64 = 500;     // writer 最长积累时间
const FLUSH_BATCH_SIZE: usize = 200;    // 达到这么多样本提前 flush

/// 单条样本（推到 writer 的最小单元）
#[derive(Debug, Clone)]
pub struct PersistSample {
    pub session_id: i64,
    pub hop_number: u32,
    pub seq: u32,
    pub latency_ms: Option<f64>,
    pub is_timeout: bool,
    pub timestamp: i64,
}

/// 推到 writer 的事件类型
#[derive(Debug)]
pub enum PersistEvent {
    Sample(PersistSample),
    /// 会话主动停止：把 ended_at + status='stopped' 写盘
    SessionStopped(i64, i64), // (session_id, ended_at)
}

/// 会话句柄：包含 session_id 和向 writer 推数据的 sender
#[derive(Clone)]
pub struct PersistHandle {
    pub session_id: i64,
    pub tx: mpsc::Sender<PersistEvent>,
}

/// 启动后台 writer task。整个 app 共享一个 writer，所有会话往同一个 channel 推。
/// 返回 Sender 供后续 `start_session` 克隆使用。
pub fn spawn_writer(app_handle: tauri::AppHandle) -> mpsc::Sender<PersistEvent> {
    let (tx, mut rx) = mpsc::channel::<PersistEvent>(2048);

    // 注意：必须用 tauri::async_runtime::spawn 而非 tokio::spawn。
    // 该函数在 Tauri 的 setup 回调里被调用，此时还没进入 Tokio 运行时上下文，
    // tokio::spawn 会 panic。tauri::async_runtime::spawn 会自动定位到 Tauri 内部
    // 持有的运行时。
    tauri::async_runtime::spawn(async move {
        let mut buffer: Vec<PersistSample> = Vec::with_capacity(FLUSH_BATCH_SIZE * 2);
        let mut flush_ticker = tokio::time::interval(Duration::from_millis(FLUSH_INTERVAL_MS));
        flush_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // 收到事件
                evt = rx.recv() => {
                    match evt {
                        Some(PersistEvent::Sample(s)) => {
                            buffer.push(s);
                            if buffer.len() >= FLUSH_BATCH_SIZE {
                                if let Err(e) = flush_buffer(&app_handle, &mut buffer).await {
                                    log::error!("trace_persist flush failed: {}", e);
                                }
                            }
                        }
                        Some(PersistEvent::SessionStopped(sid, ended_at)) => {
                            // 先 flush 该会话剩余的 samples，再写 stopped 状态
                            if let Err(e) = flush_buffer(&app_handle, &mut buffer).await {
                                log::error!("trace_persist flush failed: {}", e);
                            }
                            if let Err(e) = mark_session_stopped(&app_handle, sid, ended_at).await {
                                log::error!("trace_persist mark stopped failed: {}", e);
                            }
                        }
                        None => {
                            // channel 关闭：把剩余样本 flush 完再退出
                            let _ = flush_buffer(&app_handle, &mut buffer).await;
                            log::info!("trace_persist writer task exiting");
                            break;
                        }
                    }
                }
                // 定时 flush
                _ = flush_ticker.tick() => {
                    if !buffer.is_empty() {
                        if let Err(e) = flush_buffer(&app_handle, &mut buffer).await {
                            log::error!("trace_persist periodic flush failed: {}", e);
                        }
                    }
                }
            }
        }
    });

    tx
}

/// 创建新的 trace_session 行并返回 id
pub async fn start_session(
    app_handle: &tauri::AppHandle,
    target: &str,
    ping_interval_ms: u32,
    timeout_ms: u32,
    probe_method: &str,
) -> AppResult<i64> {
    let db = app_handle.state::<DbState>();
    let conn = db.0.lock().await;
    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        "INSERT INTO trace_session (target, started_at, ended_at, ping_interval_ms, timeout_ms, probe_method, status)
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, 'running')",
        params![target, now, ping_interval_ms, timeout_ms, probe_method],
    )?;

    Ok(conn.last_insert_rowid())
}

/// 写入或更新某一跳的元信息（IP / hostname / geo）
pub async fn upsert_hop_info(
    app_handle: &tauri::AppHandle,
    session_id: i64,
    hop_number: u32,
    ip: Option<&str>,
    hostname: Option<&str>,
    geo_json: Option<&str>,
) -> AppResult<()> {
    let db = app_handle.state::<DbState>();
    let conn = db.0.lock().await;
    conn.execute(
        "INSERT INTO trace_hop_info (session_id, hop_number, ip, hostname, geo_json)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(session_id, hop_number) DO UPDATE SET
            ip = COALESCE(excluded.ip, ip),
            hostname = COALESCE(excluded.hostname, hostname),
            geo_json = COALESCE(excluded.geo_json, geo_json)",
        params![session_id, hop_number, ip, hostname, geo_json],
    )?;
    Ok(())
}

/// 把缓冲区里的样本一次事务批量插入
async fn flush_buffer(
    app_handle: &tauri::AppHandle,
    buffer: &mut Vec<PersistSample>,
) -> AppResult<()> {
    if buffer.is_empty() { return Ok(()); }

    let db = app_handle.state::<DbState>();
    let mut conn = db.0.lock().await;
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO trace_hop_sample
             (session_id, hop_number, seq, latency_ms, is_timeout, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
        )?;
        for s in buffer.iter() {
            stmt.execute(params![
                s.session_id,
                s.hop_number,
                s.seq,
                s.latency_ms,
                s.is_timeout as i32,
                s.timestamp,
            ])?;
        }
    }
    tx.commit()?;
    buffer.clear();
    Ok(())
}

async fn mark_session_stopped(
    app_handle: &tauri::AppHandle,
    session_id: i64,
    ended_at: i64,
) -> AppResult<()> {
    let db = app_handle.state::<DbState>();
    let conn = db.0.lock().await;
    conn.execute(
        "UPDATE trace_session SET ended_at = ?1, status = 'stopped' WHERE id = ?2",
        params![ended_at, session_id],
    )?;
    Ok(())
}

// ===== 查询接口（前端会话恢复用） =====

/// 一行 trace_session 的可序列化视图
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TraceSessionRow {
    pub id: i64,
    pub target: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub ping_interval_ms: u32,
    pub timeout_ms: u32,
    pub probe_method: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TraceHopInfoRow {
    pub hop_number: u32,
    pub ip: Option<String>,
    pub hostname: Option<String>,
    pub geo_json: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TraceSampleRow {
    pub hop_number: u32,
    pub seq: u32,
    pub latency_ms: Option<f64>,
    pub is_timeout: bool,
    pub timestamp: i64,
}

/// 列出最近的 N 个 trace_session（默认 20 条，按 started_at 倒序）
pub async fn list_sessions(
    app_handle: &tauri::AppHandle,
    limit: u32,
) -> AppResult<Vec<TraceSessionRow>> {
    let db = app_handle.state::<DbState>();
    let conn = db.0.lock().await;
    let mut stmt = conn.prepare(
        "SELECT id, target, started_at, ended_at, ping_interval_ms, timeout_ms, probe_method, status
         FROM trace_session ORDER BY started_at DESC LIMIT ?1"
    )?;
    let rows = stmt.query_map(params![limit], |row| {
        Ok(TraceSessionRow {
            id: row.get(0)?,
            target: row.get(1)?,
            started_at: row.get(2)?,
            ended_at: row.get(3)?,
            ping_interval_ms: row.get(4)?,
            timeout_ms: row.get(5)?,
            probe_method: row.get(6)?,
            status: row.get(7)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

/// 加载某个 session 的 hop 元信息
pub async fn load_hop_info(
    app_handle: &tauri::AppHandle,
    session_id: i64,
) -> AppResult<Vec<TraceHopInfoRow>> {
    let db = app_handle.state::<DbState>();
    let conn = db.0.lock().await;
    let mut stmt = conn.prepare(
        "SELECT hop_number, ip, hostname, geo_json FROM trace_hop_info
         WHERE session_id = ?1 ORDER BY hop_number"
    )?;
    let rows = stmt.query_map(params![session_id], |row| {
        Ok(TraceHopInfoRow {
            hop_number: row.get(0)?,
            ip: row.get(1)?,
            hostname: row.get(2)?,
            geo_json: row.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

/// 加载某个 session 的样本（可选时间范围 + 上限，避免一次拉太多）
pub async fn load_samples(
    app_handle: &tauri::AppHandle,
    session_id: i64,
    since: Option<i64>,
    limit: u32,
) -> AppResult<Vec<TraceSampleRow>> {
    let db = app_handle.state::<DbState>();
    let conn = db.0.lock().await;

    let (sql, params_vec): (&str, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(s) = since {
        (
            "SELECT hop_number, seq, latency_ms, is_timeout, timestamp
             FROM trace_hop_sample
             WHERE session_id = ?1 AND timestamp >= ?2
             ORDER BY timestamp ASC LIMIT ?3",
            vec![Box::new(session_id), Box::new(s), Box::new(limit)],
        )
    } else {
        (
            // 没有 since 时取末尾 N 条（适合"加载最近 X 个样本"）
            "SELECT hop_number, seq, latency_ms, is_timeout, timestamp FROM (
                SELECT hop_number, seq, latency_ms, is_timeout, timestamp
                FROM trace_hop_sample
                WHERE session_id = ?1
                ORDER BY timestamp DESC LIMIT ?2
             ) ORDER BY timestamp ASC",
            vec![Box::new(session_id), Box::new(limit)],
        )
    };

    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params_refs.as_slice(), |row| {
        Ok(TraceSampleRow {
            hop_number: row.get(0)?,
            seq: row.get(1)?,
            latency_ms: row.get(2)?,
            is_timeout: row.get(3)?,
            timestamp: row.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

/// 删除某个 session 及其所有样本
pub async fn delete_session(
    app_handle: &tauri::AppHandle,
    session_id: i64,
) -> AppResult<()> {
    let db = app_handle.state::<DbState>();
    let conn = db.0.lock().await;
    conn.execute("DELETE FROM trace_hop_sample WHERE session_id = ?1", params![session_id])?;
    conn.execute("DELETE FROM trace_hop_info WHERE session_id = ?1", params![session_id])?;
    conn.execute("DELETE FROM trace_session WHERE id = ?1", params![session_id])?;
    Ok(())
}

/// 删除超过 N 天的 trace_session（按 started_at），由 settings.maxHistoryDays 触发
pub async fn cleanup_old_sessions(
    app_handle: &tauri::AppHandle,
    days_to_keep: u32,
) -> AppResult<()> {
    let db = app_handle.state::<DbState>();
    let conn = db.0.lock().await;
    let cutoff = chrono::Utc::now().timestamp_millis()
        - (days_to_keep as i64 * 24 * 60 * 60 * 1000);
    // 先删子表，避免外键孤儿
    conn.execute(
        "DELETE FROM trace_hop_sample
         WHERE session_id IN (SELECT id FROM trace_session WHERE started_at < ?1)",
        params![cutoff],
    )?;
    conn.execute(
        "DELETE FROM trace_hop_info
         WHERE session_id IN (SELECT id FROM trace_session WHERE started_at < ?1)",
        params![cutoff],
    )?;
    conn.execute(
        "DELETE FROM trace_session WHERE started_at < ?1",
        params![cutoff],
    )?;
    Ok(())
}

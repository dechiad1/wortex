use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: i64,
    pub session_id: Uuid,
    pub hook_type: String, // "pre" or "post"
    pub tool_name: String,
    pub input: String, // JSON string of tool input
    pub timestamp: DateTime<Utc>,
}

fn db_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| {
        Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Home directory not found",
        ))
    })?;
    Ok(home.join(".wortex").join("tools.db"))
}

pub fn open_db() -> Result<Connection> {
    let path = db_path()?;
    let conn = Connection::open(&path).map_err(|e| Error::Database(e.to_string()))?;
    Ok(conn)
}

pub fn init_db() -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tool_calls (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            hook_type TEXT NOT NULL,
            tool_name TEXT NOT NULL,
            input TEXT NOT NULL,
            timestamp TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| Error::Database(e.to_string()))?;

    // Create index for faster session lookups
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tool_calls_session_id ON tool_calls(session_id)",
        [],
    )
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(())
}

pub fn insert_tool_call(
    session_id: Uuid,
    hook_type: &str,
    tool_name: &str,
    input: &str,
) -> Result<()> {
    let conn = open_db()?;
    let timestamp = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO tool_calls (session_id, hook_type, tool_name, input, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![session_id.to_string(), hook_type, tool_name, input, timestamp],
    )
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(())
}

pub fn get_tool_calls_by_session(session_id: Uuid) -> Result<Vec<ToolCall>> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, session_id, hook_type, tool_name, input, timestamp
             FROM tool_calls
             WHERE session_id = ?1
             ORDER BY timestamp ASC",
        )
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = stmt
        .query_map(params![session_id.to_string()], |row| {
            let session_str: String = row.get(1)?;
            let timestamp_str: String = row.get(5)?;
            Ok(ToolCall {
                id: row.get(0)?,
                session_id: Uuid::parse_str(&session_str).unwrap_or_default(),
                hook_type: row.get(2)?,
                tool_name: row.get(3)?,
                input: row.get(4)?,
                timestamp: DateTime::parse_from_rfc3339(&timestamp_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_default(),
            })
        })
        .map_err(|e| Error::Database(e.to_string()))?;

    let mut calls = Vec::new();
    for row in rows {
        calls.push(row.map_err(|e| Error::Database(e.to_string()))?);
    }
    Ok(calls)
}

pub fn get_all_tool_calls() -> Result<Vec<ToolCall>> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, session_id, hook_type, tool_name, input, timestamp
             FROM tool_calls
             ORDER BY timestamp DESC",
        )
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = stmt
        .query_map([], |row| {
            let session_str: String = row.get(1)?;
            let timestamp_str: String = row.get(5)?;
            Ok(ToolCall {
                id: row.get(0)?,
                session_id: Uuid::parse_str(&session_str).unwrap_or_default(),
                hook_type: row.get(2)?,
                tool_name: row.get(3)?,
                input: row.get(4)?,
                timestamp: DateTime::parse_from_rfc3339(&timestamp_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_default(),
            })
        })
        .map_err(|e| Error::Database(e.to_string()))?;

    let mut calls = Vec::new();
    for row in rows {
        calls.push(row.map_err(|e| Error::Database(e.to_string()))?);
    }
    Ok(calls)
}

pub fn delete_tool_calls_by_session(session_id: Uuid) -> Result<usize> {
    let conn = open_db()?;
    let count = conn
        .execute(
            "DELETE FROM tool_calls WHERE session_id = ?1",
            params![session_id.to_string()],
        )
        .map_err(|e| Error::Database(e.to_string()))?;
    Ok(count)
}

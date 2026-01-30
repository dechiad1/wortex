use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    init_db_with_conn(&conn)
}

/// Initialize database schema on given connection (for testing)
pub fn init_db_with_conn(conn: &Connection) -> Result<()> {
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
    insert_tool_call_with_conn(&conn, session_id, hook_type, tool_name, input)
}

/// Insert tool call using given connection (for testing)
pub fn insert_tool_call_with_conn(
    conn: &Connection,
    session_id: Uuid,
    hook_type: &str,
    tool_name: &str,
    input: &str,
) -> Result<()> {
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
    get_tool_calls_by_session_with_conn(&conn, session_id)
}

/// Get tool calls by session using given connection (for testing)
pub fn get_tool_calls_by_session_with_conn(
    conn: &Connection,
    session_id: Uuid,
) -> Result<Vec<ToolCall>> {
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
    get_all_tool_calls_with_conn(&conn)
}

/// Get all tool calls using given connection (for testing)
pub fn get_all_tool_calls_with_conn(conn: &Connection) -> Result<Vec<ToolCall>> {
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
    delete_tool_calls_by_session_with_conn(&conn, session_id)
}

/// Delete tool calls by session using given connection (for testing)
pub fn delete_tool_calls_by_session_with_conn(
    conn: &Connection,
    session_id: Uuid,
) -> Result<usize> {
    let count = conn
        .execute(
            "DELETE FROM tool_calls WHERE session_id = ?1",
            params![session_id.to_string()],
        )
        .map_err(|e| Error::Database(e.to_string()))?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_db_with_conn(&conn).unwrap();
        conn
    }

    #[test]
    fn test_init_creates_table() {
        let conn = create_test_db();

        // Table should exist and be queryable
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tool_calls", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_insert_and_retrieve_tool_call() {
        let conn = create_test_db();
        let session_id = Uuid::new_v4();

        insert_tool_call_with_conn(&conn, session_id, "pre", "Read", r#"{"path":"/test"}"#)
            .unwrap();

        let calls = get_tool_calls_by_session_with_conn(&conn, session_id).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].session_id, session_id);
        assert_eq!(calls[0].hook_type, "pre");
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(calls[0].input, r#"{"path":"/test"}"#);
    }

    #[test]
    fn test_multiple_tool_calls_same_session() {
        let conn = create_test_db();
        let session_id = Uuid::new_v4();

        insert_tool_call_with_conn(&conn, session_id, "pre", "Read", r#"{"path":"/a"}"#).unwrap();
        insert_tool_call_with_conn(&conn, session_id, "post", "Read", r#"{"path":"/a"}"#).unwrap();
        insert_tool_call_with_conn(&conn, session_id, "pre", "Write", r#"{"path":"/b"}"#).unwrap();

        let calls = get_tool_calls_by_session_with_conn(&conn, session_id).unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(calls[0].hook_type, "pre");
        assert_eq!(calls[1].tool_name, "Read");
        assert_eq!(calls[1].hook_type, "post");
        assert_eq!(calls[2].tool_name, "Write");
    }

    #[test]
    fn test_tool_calls_isolated_by_session() {
        let conn = create_test_db();
        let session1 = Uuid::new_v4();
        let session2 = Uuid::new_v4();

        insert_tool_call_with_conn(&conn, session1, "pre", "Read", "{}").unwrap();
        insert_tool_call_with_conn(&conn, session1, "pre", "Write", "{}").unwrap();
        insert_tool_call_with_conn(&conn, session2, "pre", "Bash", "{}").unwrap();

        let calls1 = get_tool_calls_by_session_with_conn(&conn, session1).unwrap();
        let calls2 = get_tool_calls_by_session_with_conn(&conn, session2).unwrap();

        assert_eq!(calls1.len(), 2);
        assert_eq!(calls2.len(), 1);
        assert_eq!(calls2[0].tool_name, "Bash");
    }

    #[test]
    fn test_get_all_tool_calls() {
        let conn = create_test_db();
        let session1 = Uuid::new_v4();
        let session2 = Uuid::new_v4();

        insert_tool_call_with_conn(&conn, session1, "pre", "Read", "{}").unwrap();
        insert_tool_call_with_conn(&conn, session2, "pre", "Write", "{}").unwrap();

        let all_calls = get_all_tool_calls_with_conn(&conn).unwrap();
        assert_eq!(all_calls.len(), 2);
    }

    #[test]
    fn test_delete_tool_calls_by_session() {
        let conn = create_test_db();
        let session1 = Uuid::new_v4();
        let session2 = Uuid::new_v4();

        insert_tool_call_with_conn(&conn, session1, "pre", "Read", "{}").unwrap();
        insert_tool_call_with_conn(&conn, session1, "post", "Read", "{}").unwrap();
        insert_tool_call_with_conn(&conn, session2, "pre", "Write", "{}").unwrap();

        let deleted = delete_tool_calls_by_session_with_conn(&conn, session1).unwrap();
        assert_eq!(deleted, 2);

        let calls1 = get_tool_calls_by_session_with_conn(&conn, session1).unwrap();
        let calls2 = get_tool_calls_by_session_with_conn(&conn, session2).unwrap();

        assert_eq!(calls1.len(), 0);
        assert_eq!(calls2.len(), 1);
    }

    #[test]
    fn test_empty_session_returns_empty_vec() {
        let conn = create_test_db();
        let session_id = Uuid::new_v4();

        let calls = get_tool_calls_by_session_with_conn(&conn, session_id).unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn test_tool_call_preserves_json_input() {
        let conn = create_test_db();
        let session_id = Uuid::new_v4();
        let complex_input = r#"{"command":"ls -la","timeout":5000,"nested":{"key":"value"}}"#;

        insert_tool_call_with_conn(&conn, session_id, "pre", "Bash", complex_input).unwrap();

        let calls = get_tool_calls_by_session_with_conn(&conn, session_id).unwrap();
        assert_eq!(calls[0].input, complex_input);
    }
}

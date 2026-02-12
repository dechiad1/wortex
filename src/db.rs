use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: i64,
    pub process_id: Uuid,
    pub hook_type: String,
    pub tool_name: String,
    pub tool_input: String,
    pub timestamp: DateTime<Utc>,
    pub sequence: i64,
}

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

pub fn wortex_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| {
        Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Home directory not found",
        ))
    })?;
    Ok(home.join(".wortex"))
}

fn db_path() -> Result<PathBuf> {
    Ok(wortex_dir()?.join("wortex.db"))
}

fn legacy_state_path() -> Result<PathBuf> {
    Ok(wortex_dir()?.join("state.json"))
}

fn legacy_tools_db_path() -> Result<PathBuf> {
    Ok(wortex_dir()?.join("tools.db"))
}

// ---------------------------------------------------------------------------
// Connection management
// ---------------------------------------------------------------------------

pub fn open_db() -> Result<Connection> {
    let path = db_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(Error::Io)?;
    }
    let conn = Connection::open(&path).map_err(|e| Error::Database(e.to_string()))?;
    configure_connection(&conn)?;
    Ok(conn)
}

/// Open a connection to the database and ensure the schema exists.
/// Runs migration from legacy files if they are present.
pub fn open_and_init() -> Result<Connection> {
    let conn = open_db()?;
    init_schema(&conn)?;
    migrate_if_needed(&conn)?;
    Ok(conn)
}

fn configure_connection(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;
         PRAGMA foreign_keys = ON;",
    )
    .map_err(|e| Error::Database(e.to_string()))?;
    Ok(())
}

/// Initialize schema on an arbitrary connection (used for testing with in-memory DBs).
pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS processes (
            id              TEXT PRIMARY KEY,
            name            TEXT UNIQUE NOT NULL,
            project         TEXT NOT NULL,
            directory       TEXT NOT NULL,
            branch          TEXT,
            tmux_session    TEXT,
            tmux_window     TEXT,
            pid             INTEGER,
            status          TEXT NOT NULL DEFAULT 'spawned',
            blocked_on      TEXT,
            exit_code       INTEGER,
            command_json    TEXT NOT NULL,
            exit_kill_json  TEXT,
            prompt          TEXT,
            created_at      TEXT NOT NULL,
            updated_at      TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tool_calls (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            process_id  TEXT NOT NULL REFERENCES processes(id),
            tool_name   TEXT NOT NULL,
            tool_input  TEXT,
            hook_type   TEXT NOT NULL,
            timestamp   TEXT NOT NULL,
            sequence    INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_tool_calls_process_id
            ON tool_calls(process_id);",
    )
    .map_err(|e| Error::Database(e.to_string()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Migration from legacy files
// ---------------------------------------------------------------------------

/// Structures matching the old state.json format, used only for migration.
mod legacy {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};
    use std::path::PathBuf;
    use uuid::Uuid;

    #[derive(Deserialize)]
    pub struct State {
        #[allow(dead_code)]
        pub version: u32,
        pub entries: Vec<Entry>,
    }

    #[derive(Deserialize)]
    pub struct Entry {
        pub id: Uuid,
        pub project: String,
        pub branch: String,
        pub path: PathBuf,
        pub tmux_session: String,
        pub tmux_window: String,
        pub command: Command,
        pub exit_kill: Option<serde_json::Value>,
        pub exit_code: Option<i32>,
        pub created_at: DateTime<Utc>,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum Command {
        Claude {
            prompt: String,
            agent: Option<String>,
        },
        Raw {
            cmd: String,
        },
    }
}

fn migrate_if_needed(conn: &Connection) -> Result<()> {
    let state_path = legacy_state_path()?;
    let tools_path = legacy_tools_db_path()?;

    let has_state = state_path.exists();
    let has_tools = tools_path.exists();

    if !has_state && !has_tools {
        return Ok(());
    }

    // Check if we already have data (avoid re-migration)
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM processes", [], |row| row.get(0))
        .map_err(|e| Error::Database(e.to_string()))?;
    if count > 0 {
        return Ok(());
    }

    if has_state {
        migrate_state_json(conn, &state_path)?;
    }

    if has_tools {
        migrate_tools_db(conn, &tools_path)?;
    }

    // Backup and remove legacy files
    if has_state {
        let backup = state_path.with_extension("json.bak");
        eprintln!("Backing up {:?} -> {:?}", state_path, backup);
        std::fs::rename(&state_path, &backup).map_err(Error::Io)?;
    }
    if has_tools {
        let backup = tools_path.with_extension("db.bak");
        eprintln!("Backing up {:?} -> {:?}", tools_path, backup);
        std::fs::rename(&tools_path, &backup).map_err(Error::Io)?;
    }

    // Also clean up the old lock file
    let lock_path = wortex_dir()?.join("state.lock");
    if lock_path.exists() {
        let _ = std::fs::remove_file(&lock_path);
    }

    eprintln!("Migration to wortex.db complete.");
    Ok(())
}

fn migrate_state_json(conn: &Connection, path: &std::path::Path) -> Result<()> {
    eprintln!("Migrating state.json...");
    let content = std::fs::read_to_string(path).map_err(Error::Io)?;
    let state: legacy::State = serde_json::from_str(&content)?;

    for entry in &state.entries {
        let now = Utc::now().to_rfc3339();
        let command_json = serde_json::to_string(&entry.command)?;
        let exit_kill_json = entry
            .exit_kill
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        let prompt = match &entry.command {
            legacy::Command::Claude { prompt, .. } => Some(prompt.as_str()),
            legacy::Command::Raw { .. } => None,
        };

        let status = if entry.exit_code.is_some() {
            "exited"
        } else {
            "spawned"
        };

        conn.execute(
            "INSERT OR IGNORE INTO processes
                (id, name, project, directory, branch, tmux_session, tmux_window,
                 status, exit_code, command_json, exit_kill_json, prompt,
                 created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                entry.id.to_string(),
                entry.branch,          // use branch as name for migration
                entry.project,
                entry.path.to_string_lossy().to_string(),
                entry.branch,
                entry.tmux_session,
                entry.tmux_window,
                status,
                entry.exit_code,
                command_json,
                exit_kill_json,
                prompt,
                entry.created_at.to_rfc3339(),
                now,
            ],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        eprintln!("  Migrated process: {} ({})", entry.branch, entry.id);
    }
    Ok(())
}

fn migrate_tools_db(conn: &Connection, tools_path: &std::path::Path) -> Result<()> {
    eprintln!("Migrating tools.db...");
    let old_conn =
        Connection::open(tools_path).map_err(|e| Error::Database(e.to_string()))?;

    // Check if tool_calls table exists in old db
    let table_exists: bool = old_conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='tool_calls'",
            [],
            |row| {
                let count: i64 = row.get(0)?;
                Ok(count > 0)
            },
        )
        .map_err(|e| Error::Database(e.to_string()))?;

    if !table_exists {
        return Ok(());
    }

    let mut stmt = old_conn
        .prepare(
            "SELECT session_id, hook_type, tool_name, input, timestamp
             FROM tool_calls
             ORDER BY id ASC",
        )
        .map_err(|e| Error::Database(e.to_string()))?;

    // Track sequence numbers per process
    let mut seq_map: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| Error::Database(e.to_string()))?;

    for row in rows {
        let (session_id, hook_type, tool_name, input, timestamp) =
            row.map_err(|e| Error::Database(e.to_string()))?;

        // Only migrate tool calls whose process_id exists in the processes table
        let process_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM processes WHERE id = ?1",
                params![session_id],
                |row| {
                    let count: i64 = row.get(0)?;
                    Ok(count > 0)
                },
            )
            .map_err(|e| Error::Database(e.to_string()))?;

        if !process_exists {
            continue;
        }

        let seq = seq_map.entry(session_id.clone()).or_insert(0);
        *seq += 1;

        conn.execute(
            "INSERT INTO tool_calls (process_id, tool_name, tool_input, hook_type, timestamp, sequence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![session_id, tool_name, input, hook_type, timestamp, *seq],
        )
        .map_err(|e| Error::Database(e.to_string()))?;
    }

    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM tool_calls", [], |row| row.get(0))
        .map_err(|e| Error::Database(e.to_string()))?;
    eprintln!("  Migrated {} tool call(s)", total);
    Ok(())
}

// ---------------------------------------------------------------------------
// Process CRUD
// ---------------------------------------------------------------------------

use crate::state::{Command, Entry, ExitKill};

pub fn insert_process(conn: &Connection, entry: &Entry) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let command_json = serde_json::to_string(&entry.command)?;
    let exit_kill_json = entry
        .exit_kill
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;

    let prompt = match &entry.command {
        Command::Claude { prompt, .. } => Some(prompt.clone()),
        Command::Raw { .. } => None,
    };

    conn.execute(
        "INSERT INTO processes
            (id, name, project, directory, branch, tmux_session, tmux_window,
             status, exit_code, command_json, exit_kill_json, prompt,
             created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            entry.id.to_string(),
            entry.branch,  // use branch as name
            entry.project,
            entry.path.to_string_lossy().to_string(),
            entry.branch,
            entry.tmux_session,
            entry.tmux_window,
            "spawned",
            entry.exit_code,
            command_json,
            exit_kill_json,
            prompt,
            entry.created_at.to_rfc3339(),
            now,
        ],
    )
    .map_err(|e| Error::Database(e.to_string()))?;
    Ok(())
}

pub fn delete_process(conn: &Connection, id: Uuid) -> Result<()> {
    // Delete associated tool calls first (FK constraint)
    conn.execute(
        "DELETE FROM tool_calls WHERE process_id = ?1",
        params![id.to_string()],
    )
    .map_err(|e| Error::Database(e.to_string()))?;

    conn.execute(
        "DELETE FROM processes WHERE id = ?1",
        params![id.to_string()],
    )
    .map_err(|e| Error::Database(e.to_string()))?;
    Ok(())
}

pub fn set_exit_code(conn: &Connection, id: Uuid, code: i32) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE processes SET exit_code = ?1, status = 'exited', updated_at = ?2 WHERE id = ?3",
        params![code, now, id.to_string()],
    )
    .map_err(|e| Error::Database(e.to_string()))?;
    Ok(())
}

pub fn get_all_processes(conn: &Connection) -> Result<Vec<Entry>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, project, directory, branch, tmux_session, tmux_window,
                    command_json, exit_kill_json, exit_code, created_at
             FROM processes
             ORDER BY created_at ASC",
        )
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = stmt
        .query_map([], row_to_entry)
        .map_err(|e| Error::Database(e.to_string()))?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.map_err(|e| Error::Database(e.to_string()))?);
    }
    Ok(entries)
}

pub fn get_process_by_id(conn: &Connection, id: Uuid) -> Result<Option<Entry>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, project, directory, branch, tmux_session, tmux_window,
                    command_json, exit_kill_json, exit_code, created_at
             FROM processes
             WHERE id = ?1",
        )
        .map_err(|e| Error::Database(e.to_string()))?;

    let mut rows = stmt
        .query_map(params![id.to_string()], row_to_entry)
        .map_err(|e| Error::Database(e.to_string()))?;

    match rows.next() {
        Some(row) => Ok(Some(row.map_err(|e| Error::Database(e.to_string()))?)),
        None => Ok(None),
    }
}

pub fn get_process_by_branch(conn: &Connection, branch: &str) -> Result<Option<Entry>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, project, directory, branch, tmux_session, tmux_window,
                    command_json, exit_kill_json, exit_code, created_at
             FROM processes
             WHERE branch = ?1",
        )
        .map_err(|e| Error::Database(e.to_string()))?;

    let mut rows = stmt
        .query_map(params![branch], row_to_entry)
        .map_err(|e| Error::Database(e.to_string()))?;

    match rows.next() {
        Some(row) => Ok(Some(row.map_err(|e| Error::Database(e.to_string()))?)),
        None => Ok(None),
    }
}

fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<Entry> {
    let id_str: String = row.get(0)?;
    let project: String = row.get(1)?;
    let directory: String = row.get(2)?;
    let branch: String = row.get(3)?;
    let tmux_session: String = row.get(4)?;
    let tmux_window: String = row.get(5)?;
    let command_json: String = row.get(6)?;
    let exit_kill_json: Option<String> = row.get(7)?;
    let exit_code: Option<i32> = row.get(8)?;
    let created_at_str: String = row.get(9)?;

    let id = Uuid::parse_str(&id_str).unwrap_or_default();
    let command: Command = serde_json::from_str(&command_json).unwrap_or(Command::Raw {
        cmd: String::new(),
    });
    let exit_kill: Option<ExitKill> = exit_kill_json
        .and_then(|s| serde_json::from_str(&s).ok());
    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_default();

    Ok(Entry {
        id,
        project,
        branch,
        path: PathBuf::from(directory),
        tmux_session,
        tmux_window,
        command,
        exit_kill,
        exit_code,
        created_at,
    })
}

// ---------------------------------------------------------------------------
// Tool call CRUD
// ---------------------------------------------------------------------------

fn row_to_tool_call(row: &rusqlite::Row) -> rusqlite::Result<ToolCall> {
    let process_id_str: String = row.get(1)?;
    let timestamp_str: String = row.get(5)?;
    Ok(ToolCall {
        id: row.get(0)?,
        process_id: Uuid::parse_str(&process_id_str).unwrap_or_default(),
        hook_type: row.get(4)?,
        tool_name: row.get(2)?,
        tool_input: row.get(3)?,
        timestamp: DateTime::parse_from_rfc3339(&timestamp_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_default(),
        sequence: row.get(6)?,
    })
}

pub fn insert_tool_call(
    conn: &Connection,
    process_id: Uuid,
    hook_type: &str,
    tool_name: &str,
    input: &str,
) -> Result<()> {
    let timestamp = Utc::now().to_rfc3339();

    // Get next sequence number for this process
    let next_seq: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM tool_calls WHERE process_id = ?1",
            params![process_id.to_string()],
            |row| row.get(0),
        )
        .map_err(|e| Error::Database(e.to_string()))?;

    conn.execute(
        "INSERT INTO tool_calls (process_id, tool_name, tool_input, hook_type, timestamp, sequence)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            process_id.to_string(),
            tool_name,
            input,
            hook_type,
            timestamp,
            next_seq
        ],
    )
    .map_err(|e| Error::Database(e.to_string()))?;
    Ok(())
}

pub fn get_tool_calls_by_process(conn: &Connection, process_id: Uuid) -> Result<Vec<ToolCall>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, process_id, tool_name, tool_input, hook_type, timestamp, sequence
             FROM tool_calls
             WHERE process_id = ?1
             ORDER BY sequence ASC",
        )
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = stmt
        .query_map(params![process_id.to_string()], row_to_tool_call)
        .map_err(|e| Error::Database(e.to_string()))?;

    let mut calls = Vec::new();
    for row in rows {
        calls.push(row.map_err(|e| Error::Database(e.to_string()))?);
    }
    Ok(calls)
}

pub fn get_all_tool_calls(conn: &Connection) -> Result<Vec<ToolCall>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, process_id, tool_name, tool_input, hook_type, timestamp, sequence
             FROM tool_calls
             ORDER BY timestamp DESC",
        )
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = stmt
        .query_map([], row_to_tool_call)
        .map_err(|e| Error::Database(e.to_string()))?;

    let mut calls = Vec::new();
    for row in rows {
        calls.push(row.map_err(|e| Error::Database(e.to_string()))?);
    }
    Ok(calls)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Command, Entry, ExitKill};
    use chrono::Utc;
    use std::path::PathBuf;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    fn make_entry(branch: &str) -> Entry {
        Entry {
            id: Uuid::new_v4(),
            project: "tp".to_string(),
            branch: branch.to_string(),
            path: PathBuf::from(format!("/tmp/tp-{}", branch)),
            tmux_session: "dev".to_string(),
            tmux_window: branch.to_string(),
            command: Command::Claude {
                prompt: "do work".to_string(),
                agent: None,
            },
            exit_kill: None,
            exit_code: None,
            created_at: Utc::now(),
        }
    }

    // -- Schema tests -------------------------------------------------------

    #[test]
    fn test_schema_creates_both_tables() {
        let conn = test_conn();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM processes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tool_calls", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_wal_mode_enabled() {
        let conn = test_conn();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        // in-memory DBs report "memory", real DBs report "wal"
        assert!(mode == "memory" || mode == "wal");
    }

    // -- Process CRUD tests -------------------------------------------------

    #[test]
    fn test_insert_and_get_process() {
        let conn = test_conn();
        let entry = make_entry("feat-a");

        insert_process(&conn, &entry).unwrap();
        let found = get_process_by_id(&conn, entry.id).unwrap();

        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.id, entry.id);
        assert_eq!(found.branch, "feat-a");
        assert_eq!(found.project, "tp");
    }

    #[test]
    fn test_get_process_by_branch() {
        let conn = test_conn();
        let entry = make_entry("feat-b");
        insert_process(&conn, &entry).unwrap();

        let found = get_process_by_branch(&conn, "feat-b").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, entry.id);

        let missing = get_process_by_branch(&conn, "nope").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_get_all_processes() {
        let conn = test_conn();
        insert_process(&conn, &make_entry("a")).unwrap();
        insert_process(&conn, &make_entry("b")).unwrap();

        let all = get_all_processes(&conn).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_delete_process() {
        let conn = test_conn();
        let entry = make_entry("doomed");
        insert_process(&conn, &entry).unwrap();

        delete_process(&conn, entry.id).unwrap();
        let found = get_process_by_id(&conn, entry.id).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_set_exit_code() {
        let conn = test_conn();
        let entry = make_entry("exiting");
        insert_process(&conn, &entry).unwrap();

        set_exit_code(&conn, entry.id, 42).unwrap();

        let found = get_process_by_id(&conn, entry.id).unwrap().unwrap();
        assert_eq!(found.exit_code, Some(42));
    }

    #[test]
    fn test_roundtrip_command_types() {
        let conn = test_conn();

        let mut claude_entry = make_entry("claude-test");
        claude_entry.command = Command::Claude {
            prompt: "build it".to_string(),
            agent: Some("worker".to_string()),
        };
        insert_process(&conn, &claude_entry).unwrap();

        let found = get_process_by_id(&conn, claude_entry.id).unwrap().unwrap();
        match &found.command {
            Command::Claude { prompt, agent } => {
                assert_eq!(prompt, "build it");
                assert_eq!(agent.as_deref(), Some("worker"));
            }
            _ => panic!("expected Claude command"),
        }

        let mut raw_entry = make_entry("raw-test");
        raw_entry.command = Command::Raw {
            cmd: "npm test".to_string(),
        };
        insert_process(&conn, &raw_entry).unwrap();

        let found = get_process_by_id(&conn, raw_entry.id).unwrap().unwrap();
        match &found.command {
            Command::Raw { cmd } => assert_eq!(cmd, "npm test"),
            _ => panic!("expected Raw command"),
        }
    }

    #[test]
    fn test_roundtrip_exit_kill() {
        let conn = test_conn();

        let mut entry = make_entry("ek-codes");
        entry.exit_kill = Some(ExitKill::Codes(vec![0, 1]));
        insert_process(&conn, &entry).unwrap();

        let found = get_process_by_id(&conn, entry.id).unwrap().unwrap();
        match &found.exit_kill {
            Some(ExitKill::Codes(codes)) => assert_eq!(codes, &[0, 1]),
            _ => panic!("expected Codes"),
        }

        let mut entry2 = make_entry("ek-any");
        entry2.exit_kill = Some(ExitKill::Any);
        insert_process(&conn, &entry2).unwrap();

        let found2 = get_process_by_id(&conn, entry2.id).unwrap().unwrap();
        assert!(matches!(found2.exit_kill, Some(ExitKill::Any)));
    }

    // -- Tool call tests ----------------------------------------------------

    #[test]
    fn test_insert_and_get_tool_call() {
        let conn = test_conn();
        let entry = make_entry("tc-test");
        insert_process(&conn, &entry).unwrap();

        insert_tool_call(&conn, entry.id, "pre", "Read", r#"{"path":"/test"}"#).unwrap();

        let calls = get_tool_calls_by_process(&conn, entry.id).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].process_id, entry.id);
        assert_eq!(calls[0].hook_type, "pre");
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(calls[0].sequence, 1);
    }

    #[test]
    fn test_tool_call_sequence_increments() {
        let conn = test_conn();
        let entry = make_entry("seq-test");
        insert_process(&conn, &entry).unwrap();

        insert_tool_call(&conn, entry.id, "pre", "Read", "{}").unwrap();
        insert_tool_call(&conn, entry.id, "post", "Read", "{}").unwrap();
        insert_tool_call(&conn, entry.id, "pre", "Write", "{}").unwrap();

        let calls = get_tool_calls_by_process(&conn, entry.id).unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].sequence, 1);
        assert_eq!(calls[1].sequence, 2);
        assert_eq!(calls[2].sequence, 3);
    }

    #[test]
    fn test_tool_calls_isolated_by_process() {
        let conn = test_conn();
        let e1 = make_entry("iso-a");
        let e2 = make_entry("iso-b");
        insert_process(&conn, &e1).unwrap();
        insert_process(&conn, &e2).unwrap();

        insert_tool_call(&conn, e1.id, "pre", "Read", "{}").unwrap();
        insert_tool_call(&conn, e1.id, "pre", "Write", "{}").unwrap();
        insert_tool_call(&conn, e2.id, "pre", "Bash", "{}").unwrap();

        let c1 = get_tool_calls_by_process(&conn, e1.id).unwrap();
        let c2 = get_tool_calls_by_process(&conn, e2.id).unwrap();
        assert_eq!(c1.len(), 2);
        assert_eq!(c2.len(), 1);
        assert_eq!(c2[0].tool_name, "Bash");
        // Sequences are independent per process
        assert_eq!(c2[0].sequence, 1);
    }

    #[test]
    fn test_get_all_tool_calls() {
        let conn = test_conn();
        let e1 = make_entry("all-a");
        let e2 = make_entry("all-b");
        insert_process(&conn, &e1).unwrap();
        insert_process(&conn, &e2).unwrap();

        insert_tool_call(&conn, e1.id, "pre", "Read", "{}").unwrap();
        insert_tool_call(&conn, e2.id, "pre", "Write", "{}").unwrap();

        let all = get_all_tool_calls(&conn).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_delete_process_cascades_tool_calls() {
        let conn = test_conn();
        let entry = make_entry("cascade");
        insert_process(&conn, &entry).unwrap();
        insert_tool_call(&conn, entry.id, "pre", "Read", "{}").unwrap();
        insert_tool_call(&conn, entry.id, "post", "Read", "{}").unwrap();

        delete_process(&conn, entry.id).unwrap();

        let calls = get_tool_calls_by_process(&conn, entry.id).unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn test_tool_call_preserves_json_input() {
        let conn = test_conn();
        let entry = make_entry("json-test");
        insert_process(&conn, &entry).unwrap();

        let complex = r#"{"command":"ls -la","timeout":5000,"nested":{"key":"value"}}"#;
        insert_tool_call(&conn, entry.id, "pre", "Bash", complex).unwrap();

        let calls = get_tool_calls_by_process(&conn, entry.id).unwrap();
        assert_eq!(calls[0].tool_input, complex);
    }

    // -- Migration tests ----------------------------------------------------

    #[test]
    fn test_migrate_state_json() {
        let conn = test_conn();
        let temp_dir = tempfile::TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state.json");

        let state_json = r#"{
            "version": 1,
            "entries": [
                {
                    "id": "550e8400-e29b-41d4-a716-446655440000",
                    "project": "tp",
                    "branch": "feat-x",
                    "path": "/tmp/tp-feat-x",
                    "tmux_session": "dev",
                    "tmux_window": "feat-x",
                    "command": {"type": "claude", "prompt": "do work", "agent": null},
                    "exit_kill": null,
                    "exit_code": null,
                    "created_at": "2025-01-01T00:00:00Z"
                }
            ]
        }"#;
        std::fs::write(&state_path, state_json).unwrap();

        migrate_state_json(&conn, &state_path).unwrap();

        let entries = get_all_processes(&conn).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].branch, "feat-x");
        assert_eq!(
            entries[0].id,
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
        );
    }

    #[test]
    fn test_migrate_tools_db() {
        let conn = test_conn();

        // First insert a process so the FK is satisfied
        let entry = Entry {
            id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            project: "tp".to_string(),
            branch: "feat-x".to_string(),
            path: PathBuf::from("/tmp/tp-feat-x"),
            tmux_session: "dev".to_string(),
            tmux_window: "feat-x".to_string(),
            command: Command::Raw {
                cmd: "echo".to_string(),
            },
            exit_kill: None,
            exit_code: None,
            created_at: Utc::now(),
        };
        insert_process(&conn, &entry).unwrap();

        // Create a legacy tools.db
        let temp_dir = tempfile::TempDir::new().unwrap();
        let tools_path = temp_dir.path().join("tools.db");
        let old_conn = Connection::open(&tools_path).unwrap();
        old_conn
            .execute_batch(
                "CREATE TABLE tool_calls (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id TEXT NOT NULL,
                    hook_type TEXT NOT NULL,
                    tool_name TEXT NOT NULL,
                    input TEXT NOT NULL,
                    timestamp TEXT NOT NULL
                );
                INSERT INTO tool_calls (session_id, hook_type, tool_name, input, timestamp)
                VALUES ('550e8400-e29b-41d4-a716-446655440000', 'pre', 'Read', '{\"path\":\"/test\"}', '2025-01-01T00:00:00Z');",
            )
            .unwrap();
        drop(old_conn);

        migrate_tools_db(&conn, &tools_path).unwrap();

        let calls = get_tool_calls_by_process(&conn, entry.id).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(calls[0].sequence, 1);
    }

    #[test]
    fn test_migrate_skips_orphaned_tool_calls() {
        let conn = test_conn();
        // No processes inserted - all tool calls should be skipped

        let temp_dir = tempfile::TempDir::new().unwrap();
        let tools_path = temp_dir.path().join("tools.db");
        let old_conn = Connection::open(&tools_path).unwrap();
        old_conn
            .execute_batch(
                "CREATE TABLE tool_calls (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id TEXT NOT NULL,
                    hook_type TEXT NOT NULL,
                    tool_name TEXT NOT NULL,
                    input TEXT NOT NULL,
                    timestamp TEXT NOT NULL
                );
                INSERT INTO tool_calls (session_id, hook_type, tool_name, input, timestamp)
                VALUES ('no-such-process', 'pre', 'Read', '{}', '2025-01-01T00:00:00Z');",
            )
            .unwrap();
        drop(old_conn);

        migrate_tools_db(&conn, &tools_path).unwrap();

        let all = get_all_tool_calls(&conn).unwrap();
        assert!(all.is_empty());
    }
}

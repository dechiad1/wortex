use crate::db;
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Types (unchanged -- still used throughout the codebase)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: Uuid,
    pub project: String,
    pub branch: String,
    pub path: PathBuf,
    pub tmux_session: String,
    pub tmux_window: String,
    pub command: Command,
    pub exit_kill: Option<ExitKill>,
    pub exit_code: Option<i32>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitKill {
    Codes(Vec<i32>),
    Any,
}

impl ExitKill {
    pub fn matches(&self, code: i32) -> bool {
        match self {
            ExitKill::Any => true,
            ExitKill::Codes(codes) => codes.contains(&code),
        }
    }
}

// ---------------------------------------------------------------------------
// Kept for backward compat -- State wrapper used by list/cleanup/status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub version: u32,
    pub entries: Vec<Entry>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Directory helpers
// ---------------------------------------------------------------------------

pub fn ensure_initialized() -> Result<()> {
    let dir = db::wortex_dir()?;
    if !dir.exists() {
        return Err(Error::NotInitialized);
    }
    Ok(())
}

pub fn initialize() -> Result<()> {
    let dir = db::wortex_dir()?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Public API -- backed by SQLite via db module
// ---------------------------------------------------------------------------

pub fn load() -> Result<State> {
    let conn = db::open_and_init()?;
    let entries = db::get_all_processes(&conn)?;
    Ok(State {
        version: 1,
        entries,
    })
}

pub fn add_entry(entry: Entry) -> Result<()> {
    let conn = db::open_and_init()?;
    db::insert_process(&conn, &entry)
}

pub fn remove_entry(id: Uuid) -> Result<()> {
    let conn = db::open_and_init()?;
    db::delete_process(&conn, id)
}

pub fn update_exit_code(id: Uuid, code: i32) -> Result<()> {
    let conn = db::open_and_init()?;
    db::set_exit_code(&conn, id, code)
}

pub fn find_by_id(id: Uuid) -> Result<Option<Entry>> {
    let conn = db::open_and_init()?;
    db::get_process_by_id(&conn, id)
}

pub fn find_by_branch(branch: &str) -> Result<Option<Entry>> {
    let conn = db::open_and_init()?;
    db::get_process_by_branch(&conn, branch)
}

use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::path::PathBuf;
use uuid::Uuid;

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

fn wortex_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| {
        Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Home directory not found",
        ))
    })?;
    Ok(home.join(".wortex"))
}

fn state_path() -> Result<PathBuf> {
    Ok(wortex_dir()?.join("state.json"))
}

fn lock_path() -> Result<PathBuf> {
    Ok(wortex_dir()?.join("state.lock"))
}

pub fn ensure_initialized() -> Result<()> {
    let dir = wortex_dir()?;
    if !dir.exists() {
        return Err(Error::NotInitialized);
    }
    Ok(())
}

pub fn initialize() -> Result<()> {
    let dir = wortex_dir()?;
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(())
}

pub fn with_state_lock<T, F>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    let lock_file = File::create(lock_path()?)?;
    lock_file.lock_exclusive()?;
    // lock released on drop
    f()
}

pub fn load() -> Result<State> {
    let path = state_path()?;
    if !path.exists() {
        return Ok(State::default());
    }
    let content = fs::read_to_string(&path)?;
    let state: State = serde_json::from_str(&content)?;
    Ok(state)
}

pub fn save(state: &State) -> Result<()> {
    let path = state_path()?;
    let content = serde_json::to_string_pretty(state)?;
    fs::write(&path, content)?;
    Ok(())
}

pub fn add_entry(entry: Entry) -> Result<()> {
    with_state_lock(|| {
        let mut state = load()?;
        state.entries.push(entry);
        save(&state)
    })
}

pub fn remove_entry(id: Uuid) -> Result<()> {
    with_state_lock(|| {
        let mut state = load()?;
        state.entries.retain(|e| e.id != id);
        save(&state)
    })
}

pub fn update_exit_code(id: Uuid, code: i32) -> Result<()> {
    with_state_lock(|| {
        let mut state = load()?;
        if let Some(entry) = state.entries.iter_mut().find(|e| e.id == id) {
            entry.exit_code = Some(code);
        }
        save(&state)
    })
}

pub fn find_by_id(id: Uuid) -> Result<Option<Entry>> {
    let state = load()?;
    Ok(state.entries.into_iter().find(|e| e.id == id))
}

pub fn find_by_branch(branch: &str) -> Result<Option<Entry>> {
    let state = load()?;
    Ok(state.entries.into_iter().find(|e| e.branch == branch))
}

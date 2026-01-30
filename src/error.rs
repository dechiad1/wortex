use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Run `wortex init` first")]
    NotInitialized,

    #[error("Must run inside tmux session")]
    NotInTmux,

    #[error("Not a git repository")]
    NotGitRepo,

    #[error("Must run from main repo, not a worktree")]
    InsideWorktree,

    #[error("Remote '{0}' not found")]
    RemoteNotFound(String),

    #[error("Branch '{0}' already exists")]
    BranchExists(String),

    #[error("Entry for branch '{0}' already exists in state (run `wortex cleanup` to remove stale entries)")]
    EntryExists(String),

    #[error("Directory '{0}' already exists")]
    DirectoryExists(PathBuf),

    #[error("Must specify --prompt or --cmd")]
    NoCommand,

    #[error("--prompt and --cmd are mutually exclusive")]
    ConflictingCommands,

    #[error("Entry not found: {0}")]
    EntryNotFound(String),

    #[error("Tmux window '{0}' not found")]
    WindowNotFound(String),

    #[error("Git error: {0}")]
    Git(String),

    #[error("Tmux error: {0}")]
    Tmux(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(String),
}

pub type Result<T> = std::result::Result<T, Error>;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "wortex")]
#[command(about = "Worktree + Tmux Manager CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize wortex (creates ~/.wortex directory)
    Init,

    /// Create a new worktree with tmux window
    New {
        /// Branch name to create
        branch: String,

        /// Prompt passed to claude
        #[arg(long, group = "cmd_type")]
        prompt: Option<String>,

        /// Arbitrary command to run (mutually exclusive with --prompt)
        #[arg(long, group = "cmd_type")]
        cmd: Option<String>,

        /// Agent identifier passed to claude
        #[arg(long)]
        agent: Option<String>,

        /// Kill pane on exit. No value = exit 0. "any" = any code. "0,1" = specific codes
        #[arg(long, value_name = "CODES")]
        exit_kill: Option<Option<String>>,

        /// Git remote
        #[arg(long, default_value = "origin")]
        remote: String,

        /// Base branch to create worktree from
        #[arg(long, default_value = "main")]
        base: String,
    },

    /// Internal command executed inside tmux window
    #[command(hide = true)]
    #[command(name = "__run")]
    Run {
        /// Entry ID
        id: String,
    },

    /// List tracked worktrees
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Switch to a worktree's tmux window
    Switch {
        /// Branch name
        branch: String,
    },

    /// Kill a worktree and its tmux window
    Kill {
        /// Branch name
        branch: String,

        /// Keep the worktree directory
        #[arg(long)]
        keep_worktree: bool,
    },

    /// Clean up stale entries
    #[command(alias = "clean")]
    Cleanup {
        /// Show what would be removed without removing
        #[arg(long)]
        dry_run: bool,
    },

    /// Show git status for all tracked worktrees
    Status,
}

#[derive(Debug, Clone)]
pub enum ExitKillArg {
    /// Kill on exit code 0
    Default,
    /// Kill on any exit code
    Any,
    /// Kill on specific exit codes
    Codes(Vec<i32>),
}

impl ExitKillArg {
    pub fn parse(value: Option<Option<String>>) -> Option<Self> {
        match value {
            None => None,
            Some(None) => Some(ExitKillArg::Default),
            Some(Some(s)) if s.to_lowercase() == "any" => Some(ExitKillArg::Any),
            Some(Some(s)) => {
                let codes: Vec<i32> = s
                    .split(',')
                    .filter_map(|c| c.trim().parse().ok())
                    .collect();
                if codes.is_empty() {
                    Some(ExitKillArg::Default)
                } else {
                    Some(ExitKillArg::Codes(codes))
                }
            }
        }
    }
}

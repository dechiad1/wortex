use crate::cli::ExitKillArg;
use crate::db;
use crate::error::{Error, Result};
use crate::state::{self, Command, Entry, ExitKill};
use crate::{git, tmux};
use chrono::Utc;
use serde_json::json;
use std::env;
use std::fs;
use std::path::Path;
use uuid::Uuid;

pub struct NewArgs {
    pub branch: String,
    pub prompt: Option<String>,
    pub cmd: Option<String>,
    pub agent: Option<String>,
    pub exit_kill: Option<ExitKillArg>,
    pub remote: String,
    pub base: String,
}

pub fn execute(args: NewArgs) -> Result<()> {
    // Validate command args
    if args.prompt.is_none() && args.cmd.is_none() {
        return Err(Error::NoCommand);
    }
    if args.prompt.is_some() && args.cmd.is_some() {
        return Err(Error::ConflictingCommands);
    }

    // Validate running inside tmux
    if !tmux::is_inside_tmux() {
        return Err(Error::NotInTmux);
    }

    // Validate running in git repo
    if !git::is_git_repo() {
        return Err(Error::NotGitRepo);
    }

    // Validate not inside a worktree
    if git::is_worktree()? {
        return Err(Error::InsideWorktree);
    }

    // Validate remote exists
    if !git::remote_exists(&args.remote)? {
        return Err(Error::RemoteNotFound(args.remote.clone()));
    }

    // Derive project prefix
    let prefix = git::get_project_prefix(&args.remote)?;

    // Check if branch already exists in git
    if git::branch_exists(&args.branch)? {
        return Err(Error::BranchExists(args.branch.clone()));
    }

    // Check if entry already exists in state
    if state::find_by_branch(&args.branch)?.is_some() {
        return Err(Error::EntryExists(args.branch.clone()));
    }

    // Calculate worktree path
    let current_dir = env::current_dir()?;
    let parent = current_dir
        .parent()
        .ok_or_else(|| Error::Git("Cannot get parent directory".to_string()))?;
    let worktree_path = parent.join(format!("{}-{}", prefix, args.branch));

    // Check if directory already exists
    if worktree_path.exists() {
        return Err(Error::DirectoryExists(worktree_path));
    }

    // Fetch from remote
    println!("Fetching from {}...", args.remote);
    git::fetch(&args.remote)?;

    // Create worktree
    let start_point = format!("{}/{}", args.remote, args.base);
    println!("Creating worktree at {:?}...", worktree_path);
    git::add_worktree(&worktree_path, &args.branch, &start_point)?;

    // Get wortex binary path (needed for hooks config)
    let wortex_bin = env::current_exe()?;

    // Get tmux session
    let session = tmux::get_current_session()?;

    // Create state entry
    let command = if let Some(prompt) = args.prompt {
        Command::Claude {
            prompt,
            agent: args.agent,
        }
    } else {
        Command::Raw {
            cmd: args.cmd.unwrap(),
        }
    };

    let exit_kill = args.exit_kill.map(|ek| match ek {
        ExitKillArg::Default => ExitKill::Codes(vec![0]),
        ExitKillArg::Any => ExitKill::Any,
        ExitKillArg::Codes(codes) => ExitKill::Codes(codes),
    });

    let entry = Entry {
        id: Uuid::new_v4(),
        project: prefix,
        branch: args.branch.clone(),
        path: worktree_path.clone(),
        tmux_session: session.clone(),
        tmux_window: args.branch.clone(),
        command,
        exit_kill,
        exit_code: None,
        created_at: Utc::now(),
    };

    // Save entry before creating window
    state::add_entry(entry.clone())?;

    // Initialize the database
    db::init_db()?;

    // Create Claude hooks configuration for tool usage logging
    if matches!(entry.command, Command::Claude { .. }) {
        println!("Setting up Claude hooks for tool logging...");
        create_claude_hooks_config(&worktree_path, &wortex_bin, entry.id)?;
    }

    // Create tmux window with wortex __run command
    let run_command = format!("{} __run {}", wortex_bin.display(), entry.id);
    println!("Creating tmux window '{}'...", args.branch);
    tmux::create_window(&session, &args.branch, &worktree_path, &run_command)?;

    println!(
        "Created worktree and tmux window for branch '{}'",
        args.branch
    );
    Ok(())
}

/// Creates .claude/settings.local.json with hooks to log tool usage
fn create_claude_hooks_config(
    worktree_path: &Path,
    wortex_bin: &Path,
    session_id: Uuid,
) -> Result<()> {
    let claude_dir = worktree_path.join(".claude");
    fs::create_dir_all(&claude_dir)?;

    let wortex_path = wortex_bin.display().to_string();
    let session_str = session_id.to_string();

    let settings = json!({
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": ".*",
                    "hooks": [
                        {
                            "type": "command",
                            "command": format!("{} __log-tool {} pre", wortex_path, session_str)
                        }
                    ]
                }
            ],
            "PostToolUse": [
                {
                    "matcher": ".*",
                    "hooks": [
                        {
                            "type": "command",
                            "command": format!("{} __log-tool {} post", wortex_path, session_str)
                        }
                    ]
                }
            ]
        }
    });

    let settings_path = claude_dir.join("settings.local.json");
    let content = serde_json::to_string_pretty(&settings)?;
    fs::write(&settings_path, content)?;

    Ok(())
}

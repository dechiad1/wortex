use crate::error::{Error, Result};
use crate::state;
use crate::{git, tmux};

pub fn execute(branch: &str, keep_worktree: bool) -> Result<()> {
    // Find the entry
    let entry = state::find_by_branch(branch)?
        .ok_or_else(|| Error::EntryNotFound(branch.to_string()))?;

    // Kill tmux window if exists
    if tmux::window_exists(&entry.tmux_session, &entry.tmux_window)? {
        println!("Killing tmux window '{}'...", entry.tmux_window);
        tmux::kill_window(&entry.tmux_session, &entry.tmux_window)?;
    }

    // Remove worktree unless --keep-worktree
    if !keep_worktree && entry.path.exists() {
        println!("Removing worktree at {:?}...", entry.path);
        git::remove_worktree(&entry.path)?;
    }

    // Delete local branch
    if git::branch_exists(&entry.branch)? {
        println!("Deleting local branch '{}'...", entry.branch);
        git::delete_branch(&entry.branch)?;
    }

    // Remove from state
    state::remove_entry(entry.id)?;

    println!("Killed worktree for branch '{}'", branch);
    Ok(())
}

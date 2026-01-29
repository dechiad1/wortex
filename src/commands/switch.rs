use crate::error::{Error, Result};
use crate::state;
use crate::tmux;

pub fn execute(branch: &str) -> Result<()> {
    // Find the entry
    let entry = state::find_by_branch(branch)?
        .ok_or_else(|| Error::EntryNotFound(branch.to_string()))?;

    // Check if window exists
    if !tmux::window_exists(&entry.tmux_session, &entry.tmux_window)? {
        return Err(Error::WindowNotFound(branch.to_string()));
    }

    // Switch to the window
    tmux::select_window(&entry.tmux_session, &entry.tmux_window)?;

    Ok(())
}

use crate::error::Result;
use crate::state;
use crate::tmux;

pub fn execute(json: bool) -> Result<()> {
    let state = state::load()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&state.entries)?);
        return Ok(());
    }

    if state.entries.is_empty() {
        println!("No tracked worktrees.");
        return Ok(());
    }

    // Print header
    println!(
        "{:<20} {:<25} {:<40} {:<10} {:<5}",
        "BRANCH", "TMUX", "PATH", "STATUS", "EXIT"
    );

    for entry in &state.entries {
        // Check if window still exists
        let window_exists =
            tmux::window_exists(&entry.tmux_session, &entry.tmux_window).unwrap_or(false);

        let status = if entry.exit_code.is_some() {
            "exited"
        } else if window_exists {
            "running"
        } else {
            "stale"
        };

        let exit_str = entry
            .exit_code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".to_string());

        let tmux_target = format!("{}:{}", entry.tmux_session, entry.tmux_window);

        // Shorten path for display
        let path_display = entry
            .path
            .to_string_lossy()
            .replace(dirs::home_dir().unwrap().to_str().unwrap(), "~");

        println!(
            "{:<20} {:<25} {:<40} {:<10} {:<5}",
            entry.branch, tmux_target, path_display, status, exit_str
        );
    }

    println!();
    println!("Tip: Use `wortex switch <branch>` or `tmux select-window -t <session>:<window>`");

    Ok(())
}

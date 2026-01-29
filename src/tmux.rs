use crate::error::{Error, Result};
use std::env;
use std::path::Path;
use std::process::Command;

pub fn is_inside_tmux() -> bool {
    env::var("TMUX").is_ok()
}

pub fn get_current_session() -> Result<String> {
    let output = Command::new("tmux")
        .args(["display-message", "-p", "#S"])
        .output()?;

    if !output.status.success() {
        return Err(Error::Tmux("Failed to get current session".to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn create_window(
    session: &str,
    window_name: &str,
    working_dir: &Path,
    command: &str,
) -> Result<()> {
    // Create the window with the command
    // Append colon to session name to avoid ambiguity with numeric window indices
    let session_target = format!("{}:", session);
    let output = Command::new("tmux")
        .args([
            "new-window",
            "-t",
            &session_target,
            "-n",
            window_name,
            "-c",
            working_dir.to_str().unwrap(),
            command,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Tmux(format!("Failed to create window: {}", stderr)));
    }

    // Set remain-on-exit for the window
    let output = Command::new("tmux")
        .args([
            "set-option",
            "-t",
            &format!("{}:{}", session, window_name),
            "remain-on-exit",
            "on",
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Tmux(format!(
            "Failed to set remain-on-exit: {}",
            stderr
        )));
    }

    Ok(())
}

pub fn window_exists(session: &str, window: &str) -> Result<bool> {
    let output = Command::new("tmux")
        .args(["list-windows", "-t", session, "-F", "#W"])
        .output()?;

    if !output.status.success() {
        // Session might not exist
        return Ok(false);
    }

    let windows = String::from_utf8_lossy(&output.stdout);
    Ok(windows.lines().any(|w| w == window))
}

pub fn kill_window(session: &str, window: &str) -> Result<()> {
    let output = Command::new("tmux")
        .args(["kill-window", "-t", &format!("{}:{}", session, window)])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Tmux(format!("Failed to kill window: {}", stderr)));
    }

    Ok(())
}

pub fn select_window(session: &str, window: &str) -> Result<()> {
    let output = Command::new("tmux")
        .args(["select-window", "-t", &format!("{}:{}", session, window)])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Tmux(format!("Failed to select window: {}", stderr)));
    }

    Ok(())
}

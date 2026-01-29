use crate::error::{Error, Result};
use crate::state::{self, Command};
use crate::tmux;
use std::process::{Command as ProcessCommand, Stdio};
use uuid::Uuid;

pub fn execute(id: &str) -> Result<()> {
    // Parse the UUID
    let uuid = Uuid::parse_str(id)
        .map_err(|_| Error::EntryNotFound(id.to_string()))?;

    // Load the entry
    let entry = state::find_by_id(uuid)?
        .ok_or_else(|| Error::EntryNotFound(id.to_string()))?;

    // Build the command
    let (program, args) = match &entry.command {
        Command::Claude { prompt, agent } => {
            let mut args = vec![prompt.clone()];
            if let Some(agent) = agent {
                args.insert(0, "--agent".to_string());
                args.insert(1, agent.clone());
            }
            ("claude".to_string(), args)
        }
        Command::Raw { cmd } => {
            // Run via shell
            ("sh".to_string(), vec!["-c".to_string(), cmd.clone()])
        }
    };

    // Execute the command
    let status = ProcessCommand::new(&program)
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .current_dir(&entry.path)
        .status()?;

    let exit_code = status.code().unwrap_or(1);

    // Check if we should kill the window
    let should_kill = entry
        .exit_kill
        .as_ref()
        .map(|ek| ek.matches(exit_code))
        .unwrap_or(false);

    if should_kill {
        // Remove entry from state
        state::remove_entry(entry.id)?;

        // Kill own tmux window
        let _ = tmux::kill_window(&entry.tmux_session, &entry.tmux_window);
    } else {
        // Update state with exit code
        state::update_exit_code(entry.id, exit_code)?;
    }

    std::process::exit(exit_code);
}

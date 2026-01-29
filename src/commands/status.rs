use crate::error::Result;
use crate::git;
use crate::state;

pub fn execute() -> Result<()> {
    let state = state::load()?;

    if state.entries.is_empty() {
        println!("No tracked worktrees.");
        return Ok(());
    }

    for entry in &state.entries {
        println!("=== {} ===", entry.branch);

        if !entry.path.exists() {
            println!("  (worktree not found)");
            println!();
            continue;
        }

        let status = git::status_short(&entry.path)?;
        if status.trim().is_empty() {
            println!("  (clean)");
        } else {
            for line in status.lines() {
                println!("  {}", line);
            }
        }
        println!();
    }

    Ok(())
}

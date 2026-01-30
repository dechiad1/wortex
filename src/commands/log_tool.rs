use crate::db;
use crate::error::{Error, Result};
use serde::Deserialize;
use std::io::{self, Read};
use uuid::Uuid;

/// Claude hook input structure for PreToolUse/PostToolUse
/// See: https://docs.anthropic.com/en/docs/claude-code/hooks
#[derive(Debug, Deserialize)]
pub struct HookInput {
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    // PostToolUse also includes tool_output, but we only log inputs
}

pub fn execute(session_id: &str, hook_type: &str) -> Result<()> {
    // Parse session ID as UUID
    let session_uuid = Uuid::parse_str(session_id)
        .map_err(|_| Error::EntryNotFound(session_id.to_string()))?;

    // Validate hook type
    if hook_type != "pre" && hook_type != "post" {
        return Err(Error::Git(format!("Invalid hook type: {}", hook_type)));
    }

    // Read hook input from stdin
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| Error::Io(e))?;

    // Parse the hook input JSON
    let hook_input: HookInput = serde_json::from_str(&input)?;

    // Convert tool_input to string for storage
    let input_str = serde_json::to_string(&hook_input.tool_input)?;

    // Ensure database is initialized
    db::init_db()?;

    // Insert tool call into database
    db::insert_tool_call(session_uuid, hook_type, &hook_input.tool_name, &input_str)?;

    Ok(())
}

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
        return Err(Error::InvalidHookType(hook_type.to_string()));
    }

    // Read hook input from stdin
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(Error::Io)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hook_input_read_tool() {
        let json = r#"{"tool_name":"Read","tool_input":{"file_path":"/home/user/test.rs"}}"#;
        let hook_input: HookInput = serde_json::from_str(json).unwrap();

        assert_eq!(hook_input.tool_name, "Read");
        assert_eq!(
            hook_input.tool_input["file_path"],
            "/home/user/test.rs"
        );
    }

    #[test]
    fn test_parse_hook_input_bash_tool() {
        let json = r#"{"tool_name":"Bash","tool_input":{"command":"git status","timeout":60000}}"#;
        let hook_input: HookInput = serde_json::from_str(json).unwrap();

        assert_eq!(hook_input.tool_name, "Bash");
        assert_eq!(hook_input.tool_input["command"], "git status");
        assert_eq!(hook_input.tool_input["timeout"], 60000);
    }

    #[test]
    fn test_parse_hook_input_with_extra_fields() {
        // PostToolUse includes tool_output, which we ignore
        let json = r#"{"tool_name":"Read","tool_input":{"file_path":"/test"},"tool_output":"file contents..."}"#;
        let hook_input: HookInput = serde_json::from_str(json).unwrap();

        assert_eq!(hook_input.tool_name, "Read");
    }

    #[test]
    fn test_parse_hook_input_nested_object() {
        let json = r#"{"tool_name":"Edit","tool_input":{"file_path":"/test.rs","old_string":"fn main()","new_string":"fn main() -> Result<()>"}}"#;
        let hook_input: HookInput = serde_json::from_str(json).unwrap();

        assert_eq!(hook_input.tool_name, "Edit");
        assert_eq!(hook_input.tool_input["old_string"], "fn main()");
    }

    #[test]
    fn test_invalid_session_id_format() {
        let result = Uuid::parse_str("not-a-uuid");
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_session_id_format() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let result = Uuid::parse_str(uuid_str);
        assert!(result.is_ok());
    }
}

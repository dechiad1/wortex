use crate::db::{self, ToolCall};
use crate::error::Result;
use crate::state;

pub struct ToolsArgs {
    pub branch: Option<String>,
    pub json: bool,
    pub hook_type: Option<String>,
    pub limit: Option<usize>,
}

pub fn execute(args: ToolsArgs) -> Result<()> {
    // Ensure database is initialized
    db::init_db()?;

    let mut calls: Vec<ToolCall> = if let Some(ref branch) = args.branch {
        // Get tool calls for specific session
        let entry = state::find_by_branch(branch)?
            .ok_or_else(|| crate::error::Error::EntryNotFound(branch.clone()))?;
        db::get_tool_calls_by_session(entry.id)?
    } else {
        // Get all tool calls
        db::get_all_tool_calls()?
    };

    // Filter by hook type if specified
    if let Some(ref hook_type) = args.hook_type {
        calls.retain(|c| c.hook_type == *hook_type);
    }

    // Apply limit if specified
    if let Some(limit) = args.limit {
        calls.truncate(limit);
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&calls)?);
    } else {
        if calls.is_empty() {
            println!("No tool calls found.");
            return Ok(());
        }

        for call in &calls {
            let timestamp = call.timestamp.format("%Y-%m-%d %H:%M:%S");
            let hook_badge = if call.hook_type == "pre" { "PRE " } else { "POST" };

            println!(
                "[{}] {} {} {}",
                timestamp, hook_badge, call.tool_name, call.session_id
            );

            // Parse and pretty-print the input (truncated if too long)
            if let Ok(input_value) = serde_json::from_str::<serde_json::Value>(&call.input) {
                let input_str = format_input(&input_value);
                for line in input_str.lines() {
                    println!("    {}", line);
                }
            }
            println!();
        }

        println!("Total: {} tool call(s)", calls.len());
    }

    Ok(())
}

fn format_input(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut lines = Vec::new();
            for (key, val) in map {
                let val_str = match val {
                    serde_json::Value::String(s) => {
                        if s.len() > 100 {
                            format!("\"{}...\" ({} chars)", &s[..100], s.len())
                        } else {
                            format!("\"{}\"", s)
                        }
                    }
                    _ => {
                        let s = val.to_string();
                        if s.len() > 100 {
                            format!("{}... ({} chars)", &s[..100], s.len())
                        } else {
                            s
                        }
                    }
                };
                lines.push(format!("{}: {}", key, val_str));
            }
            lines.join("\n")
        }
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn test_format_input_simple_object() {
        let value = json!({"command": "ls -la"});
        let result = format_input(&value);
        assert_eq!(result, "command: \"ls -la\"");
    }

    #[test]
    fn test_format_input_multiple_keys() {
        let value = json!({"file_path": "/test.rs", "limit": 100});
        let result = format_input(&value);
        // Keys may be in any order
        assert!(result.contains("file_path: \"/test.rs\""));
        assert!(result.contains("limit: 100"));
    }

    #[test]
    fn test_format_input_truncates_long_strings() {
        let long_string = "a".repeat(150);
        let value = json!({"content": long_string});
        let result = format_input(&value);
        assert!(result.contains("(150 chars)"));
        assert!(result.contains("..."));
    }

    #[test]
    fn test_format_input_non_object() {
        let value = json!("simple string");
        let result = format_input(&value);
        assert_eq!(result, "\"simple string\"");
    }

    #[test]
    fn test_format_input_number() {
        let value = json!(42);
        let result = format_input(&value);
        assert_eq!(result, "42");
    }

    #[test]
    fn test_filter_by_hook_type() {
        let session_id = Uuid::new_v4();
        let mut calls = vec![
            ToolCall {
                id: 1,
                session_id,
                hook_type: "pre".to_string(),
                tool_name: "Read".to_string(),
                input: "{}".to_string(),
                timestamp: Utc::now(),
            },
            ToolCall {
                id: 2,
                session_id,
                hook_type: "post".to_string(),
                tool_name: "Read".to_string(),
                input: "{}".to_string(),
                timestamp: Utc::now(),
            },
            ToolCall {
                id: 3,
                session_id,
                hook_type: "pre".to_string(),
                tool_name: "Write".to_string(),
                input: "{}".to_string(),
                timestamp: Utc::now(),
            },
        ];

        // Simulate filter logic from execute()
        let hook_type = Some("pre".to_string());
        if let Some(ref ht) = hook_type {
            calls.retain(|c| c.hook_type == *ht);
        }

        assert_eq!(calls.len(), 2);
        assert!(calls.iter().all(|c| c.hook_type == "pre"));
    }

    #[test]
    fn test_limit_truncates_results() {
        let session_id = Uuid::new_v4();
        let mut calls: Vec<ToolCall> = (0..10)
            .map(|i| ToolCall {
                id: i,
                session_id,
                hook_type: "pre".to_string(),
                tool_name: format!("Tool{}", i),
                input: "{}".to_string(),
                timestamp: Utc::now(),
            })
            .collect();

        // Simulate limit logic from execute()
        let limit = Some(3);
        if let Some(l) = limit {
            calls.truncate(l);
        }

        assert_eq!(calls.len(), 3);
    }
}

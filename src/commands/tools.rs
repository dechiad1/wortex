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

mod cli;
mod commands;
mod db;
mod error;
mod git;
mod state;
mod tmux;

use clap::Parser;
use cli::{Cli, Commands, ExitKillArg};
use commands::new::NewArgs;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init => commands::init::execute(),
        Commands::New {
            branch,
            prompt,
            cmd,
            agent,
            exit_kill,
            remote,
            base,
        } => {
            if let Err(e) = state::ensure_initialized() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            commands::new::execute(NewArgs {
                branch,
                prompt,
                cmd,
                agent,
                exit_kill: ExitKillArg::parse(exit_kill),
                remote,
                base,
            })
        }
        Commands::Run { id } => {
            if let Err(e) = state::ensure_initialized() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            commands::run::execute(&id)
        }
        Commands::List { json } => {
            if let Err(e) = state::ensure_initialized() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            commands::list::execute(json)
        }
        Commands::Switch { branch } => {
            if let Err(e) = state::ensure_initialized() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            commands::switch::execute(&branch)
        }
        Commands::Kill {
            branch,
            keep_worktree,
        } => {
            if let Err(e) = state::ensure_initialized() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            commands::kill::execute(&branch, keep_worktree)
        }
        Commands::Cleanup { dry_run } => {
            if let Err(e) = state::ensure_initialized() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            commands::cleanup::execute(dry_run)
        }
        Commands::Status => {
            if let Err(e) = state::ensure_initialized() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            commands::status::execute()
        }
        Commands::LogTool {
            session_id,
            hook_type,
        } => {
            if let Err(e) = state::ensure_initialized() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            commands::log_tool::execute(&session_id, &hook_type)
        }
        Commands::Tools {
            branch,
            json,
            hook_type,
            limit,
        } => {
            if let Err(e) = state::ensure_initialized() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            commands::tools::execute(commands::tools::ToolsArgs {
                branch,
                json,
                hook_type,
                limit,
            })
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

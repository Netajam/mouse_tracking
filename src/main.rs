// src/main.rs

// Declare modules
mod commands;
mod config;
mod errors;
mod persistence;
mod utils;
#[cfg(target_os = "windows")]
mod windows_api;

// --- Use items needed in main ---
use errors::{AppError, AppResult}; 
use config::{AppConfig,load_configuration}; 
use clap::Parser;
use log::info;

// --- Define CLI Structure (remains the same) ---
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Run the activity tracker in the foreground.
    Run,
    /// Display usage statistics.
    Stats,
    /// Check for updates and self-update the application.
    Update,
}
// --- End CLI Structure ---


// --- Main Function (Dispatching Commands) ---
fn main() -> AppResult<()> {
    let cli = Cli::parse(); 
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    info!("Logger initialized. Starting application."); // Example INFO log

    // Get Data Path (needed by run and stats)
    let app_config : AppConfig = load_configuration().map_err(|e| AppError::Config(e.to_string()))?; 

    // Conditionally compile the command handling for Windows
    #[cfg(target_os = "windows")]
    {
        // Use the commands module to execute actions
        // Use '?' to propagate AppResult from execute functions
        match cli.command {
            Commands::Run => {
                commands::run::execute(&app_config)?;
            }
            Commands::Stats => {
                commands::stats::execute(&app_config.database_path)?;
            }
            Commands::Update => {
                commands::update::execute(&app_config)?;
            }
        }
    }

    // Handle non-Windows platforms
    #[cfg(not(target_os = "windows"))]
    {
        match cli.command {
             Commands::Run | Commands::Stats | Commands::Update => {
                // Return a specific error instead of exiting directly
                // This allows potential higher-level error handling or logging later
                return Err(AppError::Platform(
                    "Command not supported on this platform".to_string()
                ));
            }
        }
    }

    Ok(()) // Return Ok(()) on successful command execution
}
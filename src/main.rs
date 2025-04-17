// src/main.rs
mod commands;
mod config;
mod errors;
mod persistence;
mod utils;
mod detection;

#[cfg(target_os = "windows")]
mod windows_api;

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
    Run,
    Stats,
    Update,
}
// --- End CLI Structure ---


// --- Main Function (Dispatching Commands) ---
fn main() -> AppResult<()> {
    let cli = Cli::parse(); 
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    let app_config : AppConfig = load_configuration().map_err(|e| AppError::Config(e.to_string()))?; 

    // Conditionally compile the command handling for Windows
    
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
    Ok(()) 
}
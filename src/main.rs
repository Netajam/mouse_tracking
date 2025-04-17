// Declare the modules at the top level of the binary crate root
pub mod commands;
pub mod config;
pub mod errors;
// ACTION REQUIRED: Ensure only src/persistence.rs OR src/persistence/mod.rs exists, not both!
pub mod persistence;
pub mod types;
pub mod utils;
pub mod detection; // Assuming you have this
#[cfg(target_os = "windows")]
mod windows_api;
// Now import items needed specifically in main.rs
use clap::Parser;
// use std::path::PathBuf; // REMOVED - Unused in main.rs scope
use crate::{
    // We only import specific items needed for convenience or type annotations in main.rs itself.
    errors::AppResult, // Keep AppResult as it's used for the return type
    // errors::AppError, // REMOVED - Not used directly by name, only implicitly by `?` and AppResult
    types::AggregationLevel, // Keep as it's used in Commands enum definition
    // config::AppConfig, // REMOVED - Not used directly by name in this scope
};
// ACTION REQUIRED: Add 'simple_logger = "..."' to your Cargo.toml dependencies
use simple_logger;
use log::LevelFilter; // Keep LevelFilter as it's used in setup_logging

#[derive(Parser, Debug)]
#[command(author, version, about = "Tracks application and window usage time.", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Increase logging verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Start tracking application usage
    Track,
    /// Show usage statistics
    Stats {
        #[arg(short, long, value_enum, default_value_t = AggregationLevel::ByApplication)]
        level: AggregationLevel,
    },
    /// Aggregate old data and cleanup database (usually run automatically)
    Aggregate,
    /// Initialize or update the database schema
    InitDb,
}

fn setup_logging(verbosity: u8) {
    let level = match verbosity {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    // Ensure simple_logger is in Cargo.toml
    simple_logger::SimpleLogger::new().with_level(level).init().expect("Failed to initialize logger");
    log::info!("Logging initialized with level: {}", level);
}


fn main() -> AppResult<()> {
    let cli = Cli::parse();
    setup_logging(cli.verbose);
    let app_config = config::load_configuration()?;
    log::debug!("Using configuration: {:?}", app_config);

    // Note: We remove the database initialization from *here* because
    // the track::execute function (formerly run::execute) handles its
    // own connection setup and initialization.
    // let data_path = app_config.database_path.clone(); // No longer needed here

    match cli.command {
        Commands::Track => {
            // This now correctly calls the implementation in src/commands/track.rs
            log::info!("Starting tracking mode...");
            commands::track::execute(&app_config)?;
        }
        Commands::Stats { level } => {
            log::info!("Executing stats command with level: {:?}", level);
             // Need data_path for stats
             commands::stats::execute(&app_config.database_path, level)?;
        }
         Commands::Aggregate => {
             log::info!("Executing aggregation and cleanup command...");
             // Need data_path for aggregate
             let mut conn = persistence::open_connection_ensure_path(&app_config.database_path)?;
             persistence::aggregate_and_cleanup(&mut conn)?;
             log::info!("Aggregation finished.");
         }
         Commands::InitDb => {
             log::info!("Executing database initialization command...");
             // Need data_path for InitDb
             let mut conn = persistence::open_connection_ensure_path(&app_config.database_path)?;
             persistence::initialize_db(&mut conn)?;
             log::info!("Database initialization check complete.");
         }
    }

    Ok(())
}
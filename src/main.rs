// src/main.rs

mod commands; 
mod config;
mod persistence;
mod utils;
#[cfg(target_os = "windows")]
mod windows_api;

use persistence::get_data_file_path; 
use clap::Parser;

// --- Define CLI Structure ---
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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Get Data Path (needed by run and stats)
    let data_path = get_data_file_path().map_err(|e| e.to_string())?;

    // Conditionally compile the command handling for Windows
    #[cfg(target_os = "windows")]
    {
        // Use the commands module to execute actions
        match cli.command {
            Commands::Run => {
                // Pass data_path to the run command's execute function
                commands::run::execute(&data_path)?;
            }
            Commands::Stats => {
                // Pass data_path to the stats command's execute function
                commands::stats::execute(&data_path)?;
            }
            Commands::Update => {
                // Update command doesn't need data_path
                commands::update::execute()?;
            }
        }
    }

    // Handle non-Windows platforms
    #[cfg(not(target_os = "windows"))]
    {
        match cli.command {
             Commands::Run | Commands::Stats | Commands::Update => {
                eprintln!("Error: This command currently only supports Windows.");
                // Use std::process::exit to return non-zero code
                std::process::exit(1);
            }
        }
    }

    Ok(())
}


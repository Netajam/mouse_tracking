// src/main.rs

// Declare modules
mod persistence;
mod utils;
#[cfg(target_os = "windows")]
mod windows_api;

// Use items from modules
// Import persistence items needed by BOTH run and stats
use persistence::{get_data_file_path, open_connection};
// Import items specifically for stats (if separated later) or run
// ...

#[cfg(target_os = "windows")]
use windows_api::get_app_under_cursor; // Only needed for run

// --- General Imports ---
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use chrono::Utc;
use clap::Parser; // Import clap

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
    // Add other commands later (e.g., Config, Aggregate)
}
// --- End CLI Structure ---


// --- Main Function (Dispatching Commands) ---
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Get Data Path (needed by both commands potentially)
    let data_path = get_data_file_path().map_err(|e| e.to_string())?;

    // Conditionally compile the command handling for Windows
    #[cfg(target_os = "windows")]
    {
        match cli.command {
            Commands::Run => {
                // Call the function containing the tracking loop
                run_tracker(&data_path)?;
            }
            Commands::Stats => {
                 // Call the function to display statistics
                show_stats(&data_path)?;
            }
        }
    }

    // Handle non-Windows platforms
    #[cfg(not(target_os = "windows"))]
    {
        match cli.command {
             Commands::Run => {
                eprintln!("Error: The 'run' command currently only supports Windows.");
                std::process::exit(1); // Exit with error
            }
             Commands::Stats => {
                eprintln!("Error: The 'stats' command currently only supports Windows (as it reads Windows data).");
                 std::process::exit(1); // Exit with error
            }
        }
    }


    Ok(())
}


// --- run_tracker Function (Contains the original main loop logic) ---
#[cfg(target_os = "windows")]
fn run_tracker(data_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting tracker (run command)...");
    println!("Logs events to SQLite DB. Press Ctrl+C to stop.");
    println!("Database path: {:?}", data_path);

    // Uses from persistence module need to be imported at the top or here
    use persistence::{initialize_db, insert_new_interval, finalize_interval, finalize_dangling_intervals, aggregate_and_cleanup};

    // --- Open DB & Initialize ---
    let mut conn = open_connection(data_path)?;
    initialize_db(&mut conn)?;
    let startup_timestamp = Utc::now().timestamp();
    finalize_dangling_intervals(&conn, startup_timestamp)?;
    aggregate_and_cleanup(&mut conn)?;

    // --- Ctrl+C Handling ---
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("\nCtrl+C detected. Shutting down tracker...");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    // --- State Variables for Active Interval ---
    let mut current_cursor_target: Option<(String, Instant, i64)> = None;
    let check_interval = Duration::from_secs(1);

    // --- Main Loop ---
    while running.load(Ordering::SeqCst) {
        let loop_start_time = Instant::now();
        let detection_result = windows_api::get_app_under_cursor();
        let now_instant = Instant::now();
        let now_timestamp = Utc::now().timestamp();

        let current_app_name_option: Option<String> = match detection_result {
            Ok(Some(name)) => Some(name),
            Ok(None) => None,
            Err(_e) => None,
        };

        match current_cursor_target.as_mut() {
             Some((last_app, _start_instant, last_row_id)) => {
                if current_app_name_option.as_ref() != Some(last_app) {
                    let row_id_to_finalize = *last_row_id;
                     match persistence::finalize_interval(&conn, row_id_to_finalize, now_timestamp) {
                        Ok(0) => {}, // Ignore warning in run mode for less noise
                        Ok(_) => { },
                        Err(e) => eprintln!("[Run] Error finalizing interval ID {}: {}", row_id_to_finalize, e),
                    }

                    if let Some(new_app) = &current_app_name_option {
                        match persistence::insert_new_interval(&conn, new_app, now_timestamp) {
                            Ok(new_row_id) => {
                                current_cursor_target = Some((new_app.clone(), now_instant, new_row_id));
                            }
                            Err(e) => {
                                eprintln!("[Run] Error starting interval for {}: {}", new_app, e);
                                current_cursor_target = None;
                            }
                        }
                    } else {
                        current_cursor_target = None;
                    }
                }
            }
            None => {
                if let Some(new_app) = &current_app_name_option {
                     match persistence::insert_new_interval(&conn, new_app, now_timestamp) {
                        Ok(new_row_id) => {
                            current_cursor_target = Some((new_app.clone(), now_instant, new_row_id));
                        }
                        Err(e) => {
                             eprintln!("[Run] Error starting interval for {}: {}", new_app, e);
                        }
                    }
                }
            }
        }

        let elapsed = loop_start_time.elapsed();
        if elapsed < check_interval {
            thread::sleep(check_interval - elapsed);
        }
    }

    // --- Shutdown ---
    println!("Stopping tracker...");
    if let Some((_last_app, _start_instant, last_row_id)) = current_cursor_target {
        let shutdown_timestamp = Utc::now().timestamp();
        match persistence::finalize_interval(&conn, last_row_id, shutdown_timestamp) {
             Ok(0) => {}, // Ignore warning
             Ok(_) => println!("Finalized last active interval {}.", last_row_id),
             Err(e) => eprintln!("[Run] Error finalizing last interval ID {} on shutdown: {}", last_row_id, e),
        }
    }
    println!("Tracker stopped.");
    Ok(())
} // --- End run_tracker ---


// --- show_stats Function ---
#[cfg(target_os = "windows")]
fn show_stats(data_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
     println!("Showing statistics...");
     println!("Database path: {:?}", data_path);

     // Need to import the specific function for displaying stats
     use persistence::calculate_and_print_stats;

     let conn = open_connection(data_path)?;

     // Call the function in persistence to query and print
     calculate_and_print_stats(&conn)?;

     Ok(())
} //--- End show_stats ---
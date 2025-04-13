// src/main.rs

// Declare modules
mod persistence;
mod utils;
#[cfg(target_os = "windows")]
mod windows_api;

// Use items from modules
use persistence::{open_connection, initialize_db, finalize_dangling_intervals, get_data_file_path};

use std::{
    // collections::HashMap, // No longer needed here
    // ops::AddAssign, // No longer needed here
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant}, // Instant still used for loop timing
};
use chrono::Utc; // Import Utc for timestamps

#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> { // Return Result for error handling
    println!("Tracking time based on app under the mouse cursor (Windows only)...");
    println!("Logs events to SQLite DB. Press Ctrl+C to stop.");

    // --- Get Data Path & Open DB Connection ---
    let data_path = get_data_file_path().map_err(|e| e.to_string())?; // Convert path error
    println!("Database path: {:?}", data_path);
    let conn = open_connection(&data_path)?; // Use '?' to propagate SqlResult errors
    initialize_db(&conn)?;

    // --- Finalize old intervals on startup ---
    let startup_timestamp = Utc::now().timestamp();
    finalize_dangling_intervals(&conn, startup_timestamp)?;


    // --- Ctrl+C Handling ---
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("\nCtrl+C detected. Shutting down...");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");


    // --- State Variables for Active Interval ---
    // Store (AppName, StartInstant, DatabaseRowId)
    let mut current_cursor_target: Option<(String, Instant, i64)> = None;
    let check_interval = Duration::from_secs(1);


    // --- Main Loop ---
    while running.load(Ordering::SeqCst) {
        let loop_start_time = Instant::now();
        let detection_result = windows_api::get_app_under_cursor();
        let now_instant = Instant::now();
        let now_timestamp = Utc::now().timestamp(); // Timestamp for DB

        let current_app_name_option: Option<String> = match detection_result {
            Ok(Some(name)) => Some(name),
            Ok(None) => None,
            Err(_e) => None, // Ignore API errors for now
        };

        // --- Logic to Start/Stop DB Intervals ---
        match current_cursor_target.as_mut() {
             // Currently tracking an app
            Some((last_app, _start_instant, last_row_id)) => {
                // Check if the app under cursor changed
                if current_app_name_option.as_ref() != Some(last_app) {
                    // App changed or cursor moved off window. Finalize the last interval.
                    let row_id_to_finalize = *last_row_id; // Copy row_id before current_cursor_target potentially changes
                     match persistence::finalize_interval(&conn, row_id_to_finalize, now_timestamp) {
                        Ok(0) => eprintln!("[Warning] Tried to finalize interval ID {} but it was already finalized or not found.", row_id_to_finalize),
                        Ok(_) => { /*println!("Finalized interval {}", row_id_to_finalize);*/ }, // Optional success log
                        Err(e) => eprintln!("Error finalizing interval ID {}: {}", row_id_to_finalize, e),
                    }


                    // Check if cursor moved to a *new* app
                    if let Some(new_app) = &current_app_name_option {
                        match persistence::insert_new_interval(&conn, new_app, now_timestamp) {
                            Ok(new_row_id) => {
                                // Successfully inserted new interval, update state
                                // println!("Started interval {} for {}", new_row_id, new_app); // Optional log
                                current_cursor_target = Some((new_app.clone(), now_instant, new_row_id));
                            }
                            Err(e) => {
                                eprintln!("Error starting interval for {}: {}", new_app, e);
                                current_cursor_target = None; // Failed to insert, reset state
                            }
                        }
                    } else {
                        // Cursor moved off to nothing
                        current_cursor_target = None;
                    }
                }
                // Else: Still on the same app, do nothing to the DB until it changes
            }
             // Not currently tracking an app
            None => {
                if let Some(new_app) = &current_app_name_option {
                    // Cursor moved onto a new app. Start a new interval.
                     match persistence::insert_new_interval(&conn, new_app, now_timestamp) {
                        Ok(new_row_id) => {
                            // println!("Started interval {} for {}", new_row_id, new_app); // Optional log
                            current_cursor_target = Some((new_app.clone(), now_instant, new_row_id));
                        }
                        Err(e) => {
                             eprintln!("Error starting interval for {}: {}", new_app, e);
                             // Stay in None state
                        }
                    }
                }
                // Else: Still not on any app, do nothing
            }
        }
        // --- End DB Interval Logic ---

        // --- Sleep ---
        let elapsed = loop_start_time.elapsed();
        if elapsed < check_interval {
            thread::sleep(check_interval - elapsed);
        }
    } // --- End Main Loop ---


    // --- Shutdown ---
    println!("Stopping tracker...");
    // Finalize the currently active interval, if any
    if let Some((_last_app, _start_instant, last_row_id)) = current_cursor_target {
        let shutdown_timestamp = Utc::now().timestamp();
        match persistence::finalize_interval(&conn, last_row_id, shutdown_timestamp) {
             Ok(0) => eprintln!("[Warning] Tried to finalize interval ID {} on shutdown but it was already finalized or not found.", last_row_id),
             Ok(_) => println!("Finalized last active interval {}.", last_row_id),
             Err(e) => eprintln!("Error finalizing last interval ID {} on shutdown: {}", last_row_id, e),
        }
    }

    // --- No Summary Printed Here ---
    // Summary calculation will be done by the `stats` command later

    println!("Database connection closed implicitly.");
    println!("Run '[your_app_name] stats' to see usage summary."); // Placeholder message

    Ok(()) // Indicate success

} // --- End main ---


// Keep the non-windows main function
#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("This example currently only supports Windows.");
}

// We need a dummy implementation of format_duration for non-windows build to compile utils::format_duration call
#[cfg(not(target_os = "windows"))]
mod utils {
     use std::time::Duration;
     pub fn format_duration(_duration: Duration) -> String {
         "00:00:00".to_string()
     }
}
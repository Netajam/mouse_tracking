// src/main.rs

// Declare modules
mod persistence;
mod utils;
#[cfg(target_os = "windows")]
mod windows_api;

// Use items from modules
use persistence::{
    open_connection, initialize_db, insert_new_interval, finalize_interval,
    finalize_dangling_intervals, get_data_file_path,
    aggregate_and_cleanup, // Keep import
};
use utils::format_duration;
#[cfg(target_os = "windows")]
use windows_api::get_app_under_cursor;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use chrono::Utc;

#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Tracking time based on app under the mouse cursor (Windows only)...");
    println!("Logs events to SQLite DB. Press Ctrl+C to stop.");

    // --- Get Data Path & Open DB Connection ---
    let data_path = get_data_file_path().map_err(|e| e.to_string())?;
    println!("Database path: {:?}", data_path);
    // FIX: Make connection mutable
    let mut conn = open_connection(&data_path)?;
    // FIX: Pass mutable reference
    initialize_db(&mut conn)?;

    // --- Finalize old intervals on startup ---
    let startup_timestamp = Utc::now().timestamp();
    // finalize_dangling_intervals only needs immutable ref
    finalize_dangling_intervals(&conn, startup_timestamp)?;

    // --- Aggregate data from previous runs ---
    // FIX: Pass mutable reference
    aggregate_and_cleanup(&mut conn)?;

    // --- Ctrl+C Handling ---
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("\nCtrl+C detected. Shutting down...");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    let mut current_cursor_target: Option<(String, Instant, i64)> = None;
    let check_interval = Duration::from_secs(1);

    // --- Main Loop ---
    while running.load(Ordering::SeqCst) {
        let loop_start_time = Instant::now();
        // Note: insert/finalize only need immutable reference according to rusqlite docs
        // for execute calls. If performance becomes an issue, prepared statements
        // might be better, but keep simple for now.
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
                        Ok(0) => eprintln!("[Warning] Tried to finalize interval ID {} but it was already finalized or not found.", row_id_to_finalize),
                        Ok(_) => { },
                        Err(e) => eprintln!("Error finalizing interval ID {}: {}", row_id_to_finalize, e),
                    }

                    if let Some(new_app) = &current_app_name_option {
                        match persistence::insert_new_interval(&conn, new_app, now_timestamp) {
                            Ok(new_row_id) => {
                                current_cursor_target = Some((new_app.clone(), now_instant, new_row_id));
                            }
                            Err(e) => {
                                eprintln!("Error starting interval for {}: {}", new_app, e);
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
                             eprintln!("Error starting interval for {}: {}", new_app, e);
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
             Ok(0) => eprintln!("[Warning] Tried to finalize interval ID {} on shutdown but it was already finalized or not found.", last_row_id),
             Ok(_) => println!("Finalized last active interval {}.", last_row_id),
             Err(e) => eprintln!("Error finalizing last interval ID {} on shutdown: {}", last_row_id, e),
        }
    }

    // --- Removed optional aggregation on shutdown ---

    println!("Database connection closed implicitly when 'conn' goes out of scope.");
    println!("Run '[your_app_name] stats' to see usage summary."); // Placeholder message

    Ok(())
}


// --- Non-Windows main and utils remain the same ---
#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("This example currently only supports Windows.");
}
#[cfg(not(target_os = "windows"))]
mod utils {
     use std::time::Duration;
     pub fn format_duration(_duration: Duration) -> String {
         "00:00:00".to_string()
     }
}
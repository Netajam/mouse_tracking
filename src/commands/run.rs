// src/commands/run.rs

use crate::{persistence, windows_api, config, errors::{AppError, AppResult}};
use std::path::Path;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use std::thread;
use std::time::Instant;
use chrono::Utc;



pub fn execute(data_path: &Path) -> AppResult<()> {
    println!("Starting {} tracker (run command)...", config::APP_NAME); 
    println!("Logs events to SQLite DB. Press Ctrl+C to stop.");
    println!("Database path: {:?}", data_path);

    use persistence::{
        open_connection, initialize_db, finalize_dangling_intervals,
        aggregate_and_cleanup, insert_new_interval, finalize_interval
    };

    // --- Open DB & Initialize ---
    // Use '?' - rusqlite::Error will be automatically converted by #[from]
    let mut conn = open_connection(data_path)?;
    initialize_db(&mut conn)?;
    let startup_timestamp = Utc::now().timestamp();
    finalize_dangling_intervals(&conn, startup_timestamp)?;
    aggregate_and_cleanup(&mut conn)?;

    // --- Ctrl+C Handling ---
    // Use '?' - ctrlc::Error will be automatically converted by #[from]
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        // Keep println, errors can't easily propagate from here
        println!("\nCtrl+C detected. Shutting down tracker...");
        r.store(false, Ordering::SeqCst);
    })?;

    // --- State Variables for Active Interval ---
    let mut current_cursor_target: Option<(String, Instant, i64)> = None;
    // Use config constant for check interval
    let check_interval = config::CHECK_INTERVAL;

    // --- Main Loop ---
    while running.load(Ordering::SeqCst) {
        let loop_start_time = Instant::now();


        let detection_result: Result<Option<String>, AppError> = windows_api::get_app_under_cursor()
            .map_err(|e| AppError::Platform(format!("Failed to get app under cursor: {}", e)));

        let now_instant = Instant::now();
        let now_timestamp = Utc::now().timestamp();

        // FIX: Handle the Result FIRST to get the Option<String>
        let current_app_name_option: Option<String> = match detection_result {
            Ok(opt_name) => opt_name, // If platform call succeeded, use the Option<String> it returned
            Err(e) => {
                // If platform call failed, log the error and treat as None for tracking
                eprintln!("[Run] Platform API Error: {}", e);
                None
            }
        };

        // Use '?' for database operations where possible, log errors otherwise
        match current_cursor_target.as_mut() {
            Some((last_app, _start_instant, last_row_id)) => {
                if current_app_name_option.as_ref() != Some(last_app) {
                    let row_id_to_finalize = *last_row_id;
                    // Log errors from finalize_interval, but don't stop the tracker loop
                    if let Err(e) = finalize_interval(&conn, row_id_to_finalize, now_timestamp) {
                        // Check if it's just a "no rows updated" scenario (might happen if already finalized)
                        // We could potentially ignore certain rusqlite errors if needed, but logging is safer.
                        eprintln!("[Run] Warning/Error finalizing interval ID {}: {}", row_id_to_finalize, e);
                    }

                    // Start new interval or clear target
                    if let Some(new_app) = &current_app_name_option {
                        match insert_new_interval(&conn, new_app, now_timestamp) {
                            Ok(new_row_id) => {
                                current_cursor_target = Some((new_app.clone(), now_instant, new_row_id));
                            }
                            Err(e) => {
                                // Log error, don't stop loop, clear target
                                eprintln!("[Run] Error starting interval for {}: {}", new_app, e);
                                current_cursor_target = None;
                            }
                        }
                    } else {
                        current_cursor_target = None;
                    }
                }
                // Else: Still on the same app, do nothing to DB
            }
            None => {
                // Not tracking, check if we should start
                if let Some(new_app) = &current_app_name_option {
                     match insert_new_interval(&conn, new_app, now_timestamp) {
                        Ok(new_row_id) => {
                            current_cursor_target = Some((new_app.clone(), now_instant, new_row_id));
                        }
                        Err(e) => {
                             // Log error, remain in None state
                             eprintln!("[Run] Error starting interval for {}: {}", new_app, e);
                        }
                    }
                }
                // Else: Still not on any app, do nothing
            }
        } // end match current_cursor_target

        // --- Sleep ---
        let elapsed = loop_start_time.elapsed();
        if elapsed < check_interval {
            thread::sleep(check_interval - elapsed);
        }
    } // end while loop

    // --- Shutdown ---
    println!("Stopping tracker...");
    if let Some((_last_app, _start_instant, last_row_id)) = current_cursor_target {
        let shutdown_timestamp = Utc::now().timestamp();
        // Log errors from finalize_interval, but don't fail the shutdown
        match finalize_interval(&conn, last_row_id, shutdown_timestamp) {
            Ok(0) => {}, // Ok, might have already been finalized somehow
            Ok(_) => println!("Finalized last active interval {}.", last_row_id),
            Err(e) => eprintln!("[Run] Error finalizing last interval ID {} on shutdown: {}", last_row_id, e),
        }
    }
    println!("Tracker stopped.");
    Ok(())
}
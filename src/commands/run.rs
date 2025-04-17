// src/commands/run.rs

use crate::config::AppConfig;
use crate::{persistence, windows_api, errors::AppResult};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use std::thread;
use std::time::{Instant,Duration};
use chrono::Utc;




pub fn execute(app_config:&AppConfig) -> AppResult<()> {
    println!("Starting {} tracker (run command)...", app_config.app_name); 
    println!("Logs events to SQLite DB. Press Ctrl+C to stop.");
    println!("Database path: {:?}", &app_config.database_path);

    use persistence::{
        open_connection, initialize_db, finalize_dangling_intervals,
        aggregate_and_cleanup, insert_new_interval, finalize_interval
    };

    // --- Open DB & Initialize ---
    // Use '?' - rusqlite::Error will be automatically converted by #[from]
    let mut conn = open_connection(&app_config.database_path)?;
    initialize_db(&mut conn)?;
    let startup_timestamp = Utc::now().timestamp();
    finalize_dangling_intervals(&conn, startup_timestamp,app_config.dangling_threshold_secs)?;
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
    let mut current_cursor_target: Option<(String, String, Instant, i64)> = None;
    // Use config constant for check interval
    let check_interval : Duration = app_config.check_interval;

    // --- Main Loop ---
    while running.load(Ordering::SeqCst) {
        let loop_start_time = Instant::now();

        let detection_result_option: Option<(String, String)> = windows_api::get_app_under_cursor()?;
        // If we reach here, get_app_under_cursor returned Ok(Some(...)) or Ok(None)

        let now_instant = Instant::now();
        let now_timestamp = Utc::now().timestamp();

        // --- State machine logic ---
        match current_cursor_target.as_mut() {
            // Currently tracking an app/title
            Some((last_app, last_title, _start_instant, last_row_id)) => {

                // Check if the target info changed (app OR title OR became None)
                // Compare current Option<(String, String)> with stored (String, String)
                let target_changed = match &detection_result_option {
                    Some((current_app, current_title)) => {
                        // Changed if current app/title doesn't match last app/title
                        current_app != last_app || current_title != last_title
                    }
                    None => true, // Changed if current target is None
                };

                if target_changed {
                    // Target changed or became None. Finalize the last interval.
                    let row_id_to_finalize = *last_row_id;
                    if let Err(e) = finalize_interval(&conn, row_id_to_finalize, now_timestamp) {
                        eprintln!("[Run] Warning/Error finalizing interval ID {}: {}", row_id_to_finalize, e);
                    }

                    // Check if we moved to a NEW valid target
                    if let Some((new_app, new_title)) = detection_result_option { // Consume the option
                        match insert_new_interval(&conn, &new_app, &new_title, now_timestamp) {
                            Ok(new_row_id) => {
                                // Update state to the new app/title
                                current_cursor_target = Some((new_app, new_title, now_instant, new_row_id));
                            }
                            Err(e) => {
                                eprintln!("[Run] Error starting interval for {} - {}: {}", new_app, new_title, e);
                                current_cursor_target = None; // Clear state on DB error
                            }
                        }
                    } else {
                        // Moved off to nothing, clear state
                        current_cursor_target = None;
                    }
                }
                // Else: Still on the same app AND same title, do nothing
            }
            None => {
                // Not currently tracking anything
                // Check if we moved onto a valid target
                 if let Some((new_app, new_title)) = detection_result_option { // Consume the option
                    // Moved onto an app/title, start tracking
                    match insert_new_interval(&conn, &new_app, &new_title, now_timestamp) {
                        Ok(new_row_id) => {
                            current_cursor_target = Some((new_app, new_title, now_instant, new_row_id));
                        }
                        Err(e) => {
                            eprintln!("[Run] Error starting interval for {} - {}: {}", new_app, new_title, e);
                            // Remain in None state
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
    if let Some((_last_app, _last_title, _start_instant, last_row_id)) = current_cursor_target {
        let shutdown_timestamp = Utc::now().timestamp();
        match finalize_interval(&conn, last_row_id, shutdown_timestamp) {
            Ok(0) => {},
            Ok(_) => println!("Finalized last active interval {}.", last_row_id),
            Err(e) => eprintln!("[Run] Error finalizing last interval ID {} on shutdown: {}", last_row_id, e),
        }
    }
    println!("Tracker stopped.");
    Ok(())
}
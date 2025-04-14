// src/commands/run.rs

use crate::{persistence, windows_api}; 
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use std::thread;
use std::time::{Duration, Instant};
use chrono::Utc;
use std::path::Path;


pub fn execute(data_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting tracker (run command)...");
    println!("Logs events to SQLite DB. Press Ctrl+C to stop.");
    println!("Database path: {:?}", data_path);

    use persistence::{initialize_db, finalize_dangling_intervals, aggregate_and_cleanup};

    // --- Open DB & Initialize ---
    let mut conn = persistence::open_connection(data_path)?; 
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
                        Ok(0) => {},
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
             Ok(0) => {},
             Ok(_) => println!("Finalized last active interval {}.", last_row_id),
             Err(e) => eprintln!("[Run] Error finalizing last interval ID {} on shutdown: {}", last_row_id, e),
        }
    }
    println!("Tracker stopped.");
    Ok(())
}
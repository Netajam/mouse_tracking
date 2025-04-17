// src/commands/run.rs

use crate::{persistence, windows_api, config::AppConfig, errors::AppResult};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use std::thread;
use std::time::Instant; // Need Duration for TrackerState
use chrono::Utc;
use rusqlite::Connection; // Need Connection for passing to TrackerState methods

// --- Helper Structs for State Management ---

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrackedTarget {
    app_name: String,
    main_title: String,
    detailed_title: String,
}

#[derive(Debug)]
struct TrackerState {
    current_target: Option<(TrackedTarget, Instant, i64)>,
}

impl TrackerState {
    fn new() -> Self {
        TrackerState { current_target: None }
    }

    fn update(
        &mut self,
        conn: &Connection,
        detection_result_option: Option<(String, String, String)>,
        now_instant: Instant,
        now_timestamp: i64,
    ) {
        let new_target_option: Option<TrackedTarget> =
            detection_result_option.map(|(app, main, detailed)| TrackedTarget {
                app_name: app,
                main_title: main,
                detailed_title: detailed,
            });

        let target_changed = match &self.current_target {
            Some((tracked_target, _, _)) => new_target_option.as_ref() != Some(tracked_target),
            None => new_target_option.is_some(),
        };

        if target_changed {
            if let Some((_target, _start_instant, row_id)) = self.current_target.take() {
                if let Err(e) = persistence::finalize_interval(conn, row_id, now_timestamp) {
                    eprintln!("[TrackerState] Warning/Error finalizing interval ID {}: {}", row_id, e);
                }
            }

            if let Some(new_target) = new_target_option {
                match persistence::insert_new_interval(
                    conn,
                    &new_target.app_name,
                    &new_target.main_title,
                    &new_target.detailed_title,
                    now_timestamp,
                ) {
                    Ok(new_row_id) => {
                        self.current_target = Some((new_target, now_instant, new_row_id));
                    }
                    Err(e) => {
                        eprintln!(
                            "[TrackerState] Error starting interval for '{}' - '{}' - '{}': {}",
                            new_target.app_name, new_target.main_title, new_target.detailed_title, e
                        );
                        self.current_target = None;
                    }
                }
            }
        }
    }

    fn finalize(&mut self, conn: &Connection, shutdown_timestamp: i64) {
         if let Some((target, _start, row_id)) = self.current_target.take() {
             match persistence::finalize_interval(conn, row_id, shutdown_timestamp) {
                 Ok(0) => {},
                 Ok(_) => println!("Finalized last active interval {} for app '{}'.", row_id, target.app_name),
                 Err(e) => eprintln!("[TrackerState] Error finalizing last interval ID {} on shutdown: {}", row_id, e),
             }
         }
    }
}
// --- End Helper Structs ---


// --- Main execute Function ---
pub fn execute(app_config: &AppConfig) -> AppResult<()> {
    let data_path = &app_config.database_path;
    let check_interval = app_config.check_interval;
    let dangling_threshold_secs = app_config.dangling_threshold_secs;

    println!("Starting {} tracker (run command)...", app_config.app_name);
    println!("Logs events to SQLite DB. Press Ctrl+C to stop.");
    println!("Database path: {:?}", data_path);

    // Import persistence functions needed here
    use persistence::{
        // FIX: Use open_connection_ensure_path and initialize_db
        initialize_db,open_connection_ensure_path,
        finalize_dangling_intervals, aggregate_and_cleanup
    };

    // --- Setup Database using the correct functions ---
    // FIX: Call open_connection_ensure_path then initialize_db
    let mut conn = open_connection_ensure_path(data_path)?;
    initialize_db(&mut conn)?;
    // --- End Setup Database ---

    // --- Run startup tasks ---
    let startup_timestamp = Utc::now().timestamp();
    finalize_dangling_intervals(&conn, startup_timestamp, dangling_threshold_secs)?;
    aggregate_and_cleanup(&mut conn)?; // Pass mut conn here

    // Ctrl+C Handling
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("\nCtrl+C detected. Shutting down tracker...");
        r.store(false, Ordering::SeqCst);
    })?;

    // Initialize Tracker State
    let mut tracker_state = TrackerState::new();

    println!("--- Starting Live Detection Loop ---");
    while running.load(Ordering::SeqCst) {
        let loop_start_time = Instant::now();

        // 1. Detect current target
        let detection_result_option = match windows_api::get_detailed_window_info() {
             Ok(opt_info) => opt_info,
             Err(e) => {
                 eprintln!("[Run] Platform API Error: {}", e);
                 None
             }
         };

        // Optional: Live Logging
        match &detection_result_option {
            Some((app, main_title, detailed_title)) => {
                 if tracker_state.current_target.as_ref().map_or(true, |(tgt, _, _)| tgt.app_name != *app || tgt.main_title != *main_title || tgt.detailed_title != *detailed_title) {
                     println!("[Detected] App: '{}', MainTitle: '{}', DetailTitle: '{}'", app, main_title, detailed_title);
                 }
            }
            None => {
                 if tracker_state.current_target.is_some() { println!("[Detected] App: <None>, Titles: <None>"); }
            }
        }

        // Get current time info
        let now_instant = Instant::now();
        let now_timestamp = Utc::now().timestamp();

        // 2. Update State (handles DB interactions)
        tracker_state.update(&conn, detection_result_option, now_instant, now_timestamp);

        // 3. Sleep
        let elapsed = loop_start_time.elapsed();
        if elapsed < check_interval {
            thread::sleep(check_interval - elapsed);
        }
    } // end while loop

    // --- Shutdown ---
    println!("--- Stopping Live Detection Loop ---");
    println!("Stopping tracker...");
    // Finalize the last interval using the state method
    let shutdown_timestamp = Utc::now().timestamp();
    tracker_state.finalize(&conn, shutdown_timestamp);

    println!("Tracker stopped.");
    Ok(())
}
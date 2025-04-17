// src/commands/run.rs

// Remove: use crate::windows_api;
use crate::{
    persistence,
    config::AppConfig,
    errors::AppResult,
    detection::{self, ActivityDetector, ActivityInfo}, // Import detection trait/struct
};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use std::thread;
use std::time::Instant;
use chrono::Utc;
use rusqlite::Connection;

// --- Helper Structs (TrackedTarget can now use ActivityInfo) ---

// Option 1: Keep TrackedTarget separate if it might diverge later
#[derive(Debug, Clone, PartialEq, Eq)]
struct TrackedTarget {
    app_name: String,
    main_title: String,
    detailed_title: String,
}

// Option 2: Use ActivityInfo directly (if identical)
// type TrackedTarget = ActivityInfo; // Simpler if they are the same

impl From<ActivityInfo> for TrackedTarget { // Helper conversion
    fn from(info: ActivityInfo) -> Self {
        TrackedTarget {
            app_name: info.app_name,
            main_title: info.main_title,
            detailed_title: info.detailed_title,
        }
    }
}


#[derive(Debug)]
struct TrackerState {
    // Store TrackedTarget or ActivityInfo depending on choice above
    current_target: Option<(TrackedTarget, Instant, i64)>,
}

impl TrackerState {
    fn new() -> Self {
        TrackerState { current_target: None }
    }

    // Update signature to take Option<ActivityInfo>
    fn update(
        &mut self,
        conn: &Connection,
        detection_result_option: Option<ActivityInfo>, // Changed type
        now_instant: Instant,
        now_timestamp: i64,
    ) {
        // Convert ActivityInfo to TrackedTarget if needed
        let new_target_option: Option<TrackedTarget> =
            detection_result_option.map(TrackedTarget::from); // Use conversion

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

             if let Some(new_target) = new_target_option { // This is now TrackedTarget
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
    // --- Create the appropriate detector ---
    // This call now handles the platform check internally
    let detector = detection::create_detector()?;
    // If create_detector returns Err, execute stops here - no need for #[cfg] in this file

    let data_path = &app_config.database_path;
    let check_interval = app_config.check_interval;
    let dangling_threshold_secs = app_config.dangling_threshold_secs;

    println!("Starting {} tracker (run command)...", app_config.app_name);
    println!("Logs events to SQLite DB. Press Ctrl+C to stop.");
    println!("Database path: {:?}", data_path);

    use persistence::{
        initialize_db, open_connection_ensure_path,
        finalize_dangling_intervals, aggregate_and_cleanup
    };

    let mut conn = open_connection_ensure_path(data_path)?;
    initialize_db(&mut conn)?;

    let startup_timestamp = Utc::now().timestamp();
    finalize_dangling_intervals(&conn, startup_timestamp, dangling_threshold_secs)?;
    aggregate_and_cleanup(&mut conn)?;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("\nCtrl+C detected. Shutting down tracker...");
        r.store(false, Ordering::SeqCst);
    })?;

    let mut tracker_state = TrackerState::new();

    println!("--- Starting Live Detection Loop ---");
    while running.load(Ordering::SeqCst) {
        let loop_start_time = Instant::now();

        // 1. Detect current target using the abstraction
        let detection_result_option = match detector.get_current_activity() {
             Ok(opt_info) => opt_info, // Now returns Option<ActivityInfo>
             Err(e) => {
                 // Handle detection errors - maybe log differently than other errors?
                 eprintln!("[Run] Detection Error: {}", e);
                 // Decide if you want to stop, or just skip this cycle
                 None // Treat as no detection for this cycle
             }
         };

        // Optional: Live Logging (needs adjustment for ActivityInfo)
        match &detection_result_option {
            Some(info) => { // info is ActivityInfo
                let current_tracked = tracker_state.current_target.as_ref().map(|(t, _, _)| t);
                // Compare ActivityInfo with TrackedTarget
                if current_tracked.map_or(true, |t| t.app_name != info.app_name || t.main_title != info.main_title || t.detailed_title != info.detailed_title) {
                    println!("[Detected] App: '{}', MainTitle: '{}', DetailTitle: '{}'", info.app_name, info.main_title, info.detailed_title);
                }
            }
            None => {
                 if tracker_state.current_target.is_some() { println!("[Detected] App: <None>, Titles: <None>"); }
            }
        }

        let now_instant = Instant::now();
        let now_timestamp = Utc::now().timestamp();

        // 2. Update State (pass ActivityInfo)
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
    let shutdown_timestamp = Utc::now().timestamp();
    tracker_state.finalize(&conn, shutdown_timestamp);

    println!("Tracker stopped.");
    Ok(())
}
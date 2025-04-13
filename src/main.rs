// src/main.rs

// Declare modules
mod persistence;
mod utils;
#[cfg(target_os = "windows")]
mod windows_api; // Conditionally compile/declare this module



use std::{
    collections::HashMap,
    ops::AddAssign, // Keep AddAssign for the session map logic
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "windows")]
fn main() {
    println!("Tracking time based on app under the mouse cursor (Windows only)...");
    println!("Press Ctrl+C to stop and save the summary.");

    // --- Get Data Path (using persistence module) ---
    let data_path = match persistence::get_data_file_path() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Fatal Error: {}", e);
            return;
        }
    };
    println!("Data will be loaded/saved at: {:?}", data_path);

    // --- Load Initial Data (using persistence module) ---
    let mut historical_data = persistence::load_data(&data_path);

    // --- Ctrl+C Handling ---
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("\nCtrl+C detected. Shutting down...");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    // --- State Variables for *Current Session* ---
    let mut session_app_times: HashMap<String, Duration> = HashMap::new();
    let mut current_cursor_target: Option<(String, Instant)> = None;
    let check_interval = Duration::from_secs(1);

    // --- Main Loop ---
    while running.load(Ordering::SeqCst) {
        let loop_start_time = Instant::now();
        // Use windows_api module function
        let detection_result = windows_api::get_app_under_cursor();
        let now = Instant::now();

        let current_app_name_option: Option<String> = match detection_result {
            Ok(Some(name)) => Some(name),
            Ok(None) => None,
            Err(_e) => None, // Treat API errors as None for tracking
        };

        // --- Logic to track *session* time (remains in main) ---
        match current_cursor_target.as_mut() {
            Some((last_app, start_time)) => {
                if current_app_name_option.as_ref() != Some(last_app) {
                    let duration = now.duration_since(*start_time);
                    session_app_times.entry(last_app.clone())
                        .or_insert(Duration::ZERO)
                        .add_assign(duration);

                    match current_app_name_option {
                        Some(new_app) => {
                            *last_app = new_app.clone();
                            *start_time = now;
                        }
                        None => {
                            current_cursor_target = None;
                        }
                    }
                }
            }
            None => {
                if let Some(new_app) = current_app_name_option {
                    current_cursor_target = Some((new_app.clone(), now));
                }
            }
        }
        // --- End session tracking logic ---

        // --- Sleep ---
        let elapsed = loop_start_time.elapsed();
        if elapsed < check_interval {
            thread::sleep(check_interval - elapsed);
        }
    } // --- End Main Loop ---


    // --- Shutdown and Merge Data ---
    let final_time = Instant::now();
    if let Some((last_app, start_time)) = current_cursor_target {
        let duration = final_time.duration_since(start_time);
         session_app_times.entry(last_app.clone())
            .or_insert(Duration::ZERO)
            .add_assign(duration);
    }

    println!("\n--- Merging session time with historical data ---");
    for (app_name, session_duration) in session_app_times {
         historical_data.times.entry(app_name)
            .or_insert(Duration::ZERO)
            .add_assign(session_duration);
    }

    // --- Save Combined Data (using persistence module) ---
    if let Err(e) = persistence::save_data(&data_path, &historical_data) {
         eprintln!("Error saving data: {}", e);
    }

    // --- Print Final Summary (using utils module) ---
    println!("\n--- Application Time Summary (Combined) ---");
    if historical_data.times.is_empty() {
        println!("No application time recorded.");
    } else {
        let mut sorted_times: Vec<_> = historical_data.times.iter().collect();
        sorted_times.sort_by(|a, b| b.1.cmp(a.1));

        for (app_name, duration) in sorted_times {
             println!("{:<40}: {}", app_name, utils::format_duration(*duration)); // Use utils::
        }
    }
    println!("---------------------------------------------");
    println!("Stopped.");

} // --- End main ---


// Keep the non-windows main function
#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("This example currently only supports Windows.");
}
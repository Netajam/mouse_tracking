use std::{
    collections::HashMap,
    fs::{self, File}, // Added fs operations
    io::{BufReader, BufWriter}, // For efficient file IO
    ops::AddAssign, // To add Duration in HashMap easily
    path::{Path, PathBuf}, // For file paths
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

// Serde imports
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationSecondsWithFrac}; // Using fractional seconds for Duration

// --- Windows specific module (no changes needed inside) ---
#[cfg(target_os = "windows")]
mod windows_impl {
    // ... (keep the existing working windows_impl module here) ...
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::Path;
    use windows::core::Result;
    use windows::Win32::Foundation::{
        CloseHandle, MAX_PATH, HANDLE, HWND, POINT,
    };
    use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, WindowFromPoint, GetWindowThreadProcessId,
    };

    pub fn get_app_under_cursor() -> Result<Option<String>> {
        unsafe {
            let mut point = POINT { x: 0, y: 0 };
            GetCursorPos(&mut point)?;
            let hwnd = WindowFromPoint(point);
            if hwnd.is_invalid() { return Ok(None); }
            let final_hwnd: HWND = hwnd;
            let mut process_id: u32 = 0;
            let thread_id = GetWindowThreadProcessId(final_hwnd, Some(&mut process_id));
            if thread_id == 0 || process_id == 0 { return Ok(Some("[System Process or No PID]".to_string())); }
            let process_handle = match OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id) {
                Ok(handle) => handle, Err(_) => { return Ok(Some(format!("[Access Denied/Error PID {}]", process_id))); }
            };
            struct HandleGuard(HANDLE);
            impl Drop for HandleGuard { fn drop(&mut self) { if !self.0.is_invalid() { unsafe { CloseHandle(self.0); } } } }
            let _handle_guard = HandleGuard(process_handle);
            let mut exe_path_buf: Vec<u16> = vec![0; MAX_PATH as usize];
            let path_len = GetModuleFileNameExW(Some(process_handle), None, &mut exe_path_buf);
            if path_len == 0 { return Ok(Some(format!("[Unknown Path PID {}]", process_id))); }
            let os_string = OsString::from_wide(&exe_path_buf[..path_len as usize]);
            if let Some(path_str) = os_string.to_str() {
                let app_name = Path::new(path_str).file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| "[Invalid Path]".to_string());
                Ok(Some(app_name))
            } else { Ok(Some("[Non-UTF8 Path]".to_string())) }
        }
    }
}
// --- End Windows specific module ---


// Define a struct for our persistent data
#[serde_as] // Enable serde_with helpers
#[derive(Serialize, Deserialize, Default, Debug)] // Add Debug and Default
struct AppTimeData {
    // Use serde_as to specify how to serialize the HashMap values (Duration)
    #[serde_as(as = "HashMap<_, DurationSecondsWithFrac<String>>")]
    times: HashMap<String, Duration>,
}

// Helper function to format Duration nicely (remains the same)
fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

// Function to get the data file path
fn get_data_file_path() -> Result<PathBuf, String> {
    match dirs::data_dir() {
        Some(mut path) => {
            path.push("RustAppTimeTracker"); // Subdirectory for our app data
            path.push("cursor_app_times.json"); // Filename
            Ok(path)
        }
        None => Err("Could not find user data directory.".to_string()),
    }
}

// Function to load existing data
fn load_data(path: &Path) -> AppTimeData {
    if !path.exists() {
        println!("Data file not found at {:?}. Starting fresh.", path);
        return AppTimeData::default();
    }

    match File::open(path) {
        Ok(file) => {
            let reader = BufReader::new(file);
            match serde_json::from_reader(reader) {
                Ok(data) => {
                    println!("Successfully loaded data from {:?}", path);
                    data
                }
                Err(e) => {
                    eprintln!("Error parsing data file {:?}: {}. Starting fresh.", path, e);
                    AppTimeData::default() // Return default on parse error
                }
            }
        }
        Err(e) => {
            eprintln!("Error opening data file {:?}: {}. Starting fresh.", path, e);
            AppTimeData::default() // Return default on open error
        }
    }
}

// Function to save data
fn save_data(path: &Path, data: &AppTimeData) -> Result<(), String> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                return Err(format!("Failed to create data directory {:?}: {}", parent, e));
            }
        }
    }

    match File::create(path) {
        Ok(file) => {
            let writer = BufWriter::new(file);
            // Use pretty print for readability
            match serde_json::to_writer_pretty(writer, data) {
                Ok(_) => {
                    println!("Successfully saved data to {:?}", path);
                    Ok(())
                }
                Err(e) => Err(format!("Failed to serialize data to JSON: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to create/open data file {:?} for writing: {}", path, e)),
    }
}


#[cfg(target_os = "windows")]
fn main() {
    println!("Tracking time based on app under the mouse cursor (Windows only)...");
    println!("Press Ctrl+C to stop and save the summary.");

    // --- Get Data Path ---
    let data_path = match get_data_file_path() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Fatal Error: {}", e);
            return; // Exit if we can't determine data path
        }
    };
    println!("Data will be loaded/saved at: {:?}", data_path);


    // --- Load Initial Data ---
    // Load the historical data. We'll add the current session's time to this later.
    let mut historical_data = load_data(&data_path);


    // --- Ctrl+C Handling ---
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("\nCtrl+C detected. Shutting down...");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");


    // --- State Variables for *Current Session* ---
    let mut session_app_times: HashMap<String, Duration> = HashMap::new(); // Track only this session's times
    let mut current_cursor_target: Option<(String, Instant)> = None; // (App Name, Start Time)
    let check_interval = Duration::from_secs(1);


    // --- Main Loop ---
    while running.load(Ordering::SeqCst) {
        let loop_start_time = Instant::now();
        let detection_result = windows_impl::get_app_under_cursor();
        let now = Instant::now(); // Time after detection call

        let current_app_name_option: Option<String> = match detection_result {
            Ok(Some(name)) => Some(name),
            Ok(None) => None,
            Err(_e) => None, // Treat API errors as None for tracking
        };

        // --- Logic to track *session* time ---
        match current_cursor_target.as_mut() {
            Some((last_app, start_time)) => {
                if current_app_name_option.as_ref() != Some(last_app) {
                    let duration = now.duration_since(*start_time);
                    // Add duration to *session* map
                    session_app_times.entry(last_app.clone())
                        .or_insert(Duration::ZERO)
                        .add_assign(duration); // Use AddAssign

                    match current_app_name_option {
                        Some(new_app) => {
                            // Update current target
                            *last_app = new_app.clone();
                            *start_time = now;
                        }
                        None => {
                            // Reset current target
                            current_cursor_target = None;
                        }
                    }
                }
            }
            None => {
                if let Some(new_app) = current_app_name_option {
                    // Start tracking new target
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

    // Add time for the very last targeted application of the session
    if let Some((last_app, start_time)) = current_cursor_target {
        let duration = final_time.duration_since(start_time);
         session_app_times.entry(last_app.clone())
            .or_insert(Duration::ZERO)
            .add_assign(duration);
    }

    println!("\n--- Merging session time with historical data ---");
    // Merge session data into historical data
    for (app_name, session_duration) in session_app_times {
         historical_data.times.entry(app_name)
            .or_insert(Duration::ZERO)
            .add_assign(session_duration); // Accumulate time
    }

    // --- Save Combined Data ---
    if let Err(e) = save_data(&data_path, &historical_data) {
         eprintln!("Error saving data: {}", e);
    }


    // --- Print Final Summary (from combined data) ---
    println!("\n--- Application Time Summary (Combined) ---");
    if historical_data.times.is_empty() {
        println!("No application time recorded.");
    } else {
        let mut sorted_times: Vec<_> = historical_data.times.iter().collect(); // Borrow from historical_data
        sorted_times.sort_by(|a, b| b.1.cmp(a.1)); // Sort by duration

        for (app_name, duration) in sorted_times {
             println!("{:<40}: {}", app_name, format_duration(*duration)); // Deref duration
        }
    }
    println!("---------------------------------------------");
    println!("Stopped.");

} // --- End main ---


#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("This example currently only supports Windows.");
}
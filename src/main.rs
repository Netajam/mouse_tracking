use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant}, // Corrected import
};


// This code is specific to Windows.
#[cfg(target_os = "windows")]
mod windows_impl {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::Path;
    // FIX: Remove unused Error import
    use windows::core::Result; // Error removed
    use windows::Win32::Foundation::{
        CloseHandle, MAX_PATH, HANDLE, HWND, POINT,
    };
    use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, WindowFromPoint, GetWindowThreadProcessId,
    };

    // Function remains the same as the simplified version
    pub fn get_app_under_cursor() -> Result<Option<String>> {
        // SAFETY: Calls to Windows API functions are unsafe.
        unsafe {
            let mut point = POINT { x: 0, y: 0 };
            GetCursorPos(&mut point)?;
            let hwnd = WindowFromPoint(point);
            if hwnd.is_invalid() {
                return Ok(None);
            }
            let final_hwnd: HWND = hwnd;
            let mut process_id: u32 = 0;
            let thread_id = GetWindowThreadProcessId(final_hwnd, Some(&mut process_id));
             if thread_id == 0 || process_id == 0 {
                // Capture potential error for logging if needed, but return specific string
                // let _err = windows::core::Error::from_win32();
                return Ok(Some("[System Process or No PID]".to_string()));
            }
             let process_handle = match OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id) {
                 Ok(handle) => handle,
                 Err(_) => {
                    // let _err = e; // Capture if needed for logging
                    return Ok(Some(format!("[Access Denied/Error PID {}]", process_id)));
                 }
             };
             struct HandleGuard(HANDLE);
             impl Drop for HandleGuard {
                 fn drop(&mut self) {
                     if !self.0.is_invalid() {
                         unsafe { CloseHandle(self.0); }
                     }
                 }
             }
             let _handle_guard = HandleGuard(process_handle);
             let mut exe_path_buf: Vec<u16> = vec![0; MAX_PATH as usize];
             let path_len = GetModuleFileNameExW(Some(process_handle), None, &mut exe_path_buf);
             if path_len == 0 {
                // let _err = windows::core::Error::from_win32(); // Capture if needed
                 return Ok(Some(format!("[Unknown Path PID {}]", process_id)));
             }
             let os_string = OsString::from_wide(&exe_path_buf[..path_len as usize]);
             if let Some(path_str) = os_string.to_str() {
                 let app_name = Path::new(path_str)
                     .file_name()
                     .map(|name| name.to_string_lossy().into_owned())
                     .unwrap_or_else(|| "[Invalid Path]".to_string());
                 Ok(Some(app_name))
             } else {
                 Ok(Some("[Non-UTF8 Path]".to_string()))
             }
        }
    }
}

// Helper function to format Duration nicely
fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}


#[cfg(target_os = "windows")]
fn main() {
    println!("Tracking time based on app under the mouse cursor (Windows only)...");
    println!("Press Ctrl+C to stop and see the summary.");

    // --- Ctrl+C Handling ---
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("\nCtrl+C detected. Shutting down...");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    // --- State Variables ---
    let mut app_times: HashMap<String, Duration> = HashMap::new();
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
            Err(_e) => { // Change 'e' to '_e' if not used, or log it
                // eprintln!("[Error] Failed to get window under cursor: {:?}", _e);
                None // Treat API errors as None for tracking
            }
        };

        // --- FIXES START HERE ---
        match current_cursor_target.as_mut() {
            Some((last_app, start_time)) => { // No 'ref mut' needed
                // We were previously tracking an app
                if current_app_name_option.as_ref() != Some(last_app) {
                    // Target changed (or became None)
                    let duration = now.duration_since(*start_time);
                    let app_key = last_app.clone(); // Clone before borrow in entry()
                    let total_time = app_times.entry(app_key).or_insert(Duration::ZERO); // Use cloned key
                    *total_time += duration;

                    // Use a nested match for clarity on the new state
                    match current_app_name_option {
                        Some(new_app) => {
                            // Switched to a new app
                            println!(
                                "Cursor moved from '{}' to '{}'. Time on '{}': {} (Total: {})",
                                last_app, // Print &mut String directly
                                new_app,
                                last_app,
                                format_duration(duration),
                                format_duration(*total_time) // Deref needed
                            );
                            // Update current target
                            *last_app = new_app.clone(); // Assign new String
                            *start_time = now;          // Assign new Instant
                        }
                        None => {
                            // Moved off all apps
                            println!(
                                "Cursor left '{}'. Time spent: {} (Total: {})",
                                last_app,
                                format_duration(duration),
                                format_duration(*total_time)
                            );
                            // Reset current target
                            current_cursor_target = None; // Assign None to the outer variable
                        }
                    }
                }
                // Else: Still on the same app, do nothing (time accrues implicitly)
            }
            None => {
                // We were not tracking any app previously
                // Corrected: Check if current_app_name_option is Some
                if let Some(new_app) = current_app_name_option { // Borrow the value inside the option
                    // Cursor moved onto a new app
                    println!("Cursor entered: {}", new_app);
                    // Assign Some to the outer variable
                    current_cursor_target = Some((new_app.clone(), now)); // Clone the string
                }
                // Else: Still not on any app, do nothing
            }
        }
        // --- FIXES END HERE ---


        // --- Sleep ---
        let elapsed = loop_start_time.elapsed();
        if elapsed < check_interval {
            thread::sleep(check_interval - elapsed);
        }
    }

    // --- Shutdown ---
    let final_time = Instant::now();
    if let Some((last_app, start_time)) = current_cursor_target { // Takes ownership
        let duration = final_time.duration_since(start_time);
        let total_time = app_times.entry(last_app.clone()).or_insert(Duration::ZERO);
        *total_time += duration;
         println!(
            "Recording final time for '{}': {} (Total: {})",
            last_app, format_duration(duration), format_duration(*total_time)
        );
    }

    // --- Print Summary ---
    println!("\n--- Application Time Summary (Cursor Based) ---");
    if app_times.is_empty() {
        println!("No application time recorded.");
    } else {
        let mut sorted_times: Vec<_> = app_times.into_iter().collect();
        sorted_times.sort_by(|a, b| b.1.cmp(&a.1));

        for (app_name, duration) in sorted_times {
                 println!("{:<40}: {}", app_name, format_duration(duration));
        }
    }
    println!("---------------------------------------------");
    println!("Stopped.");

}

// Dummy main for non-Windows
#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("This example currently only supports Windows.");
}
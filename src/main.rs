use std::{
    thread,
    time::Duration,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

// This code is specific to Windows.
#[cfg(target_os = "windows")]
mod windows_impl {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::Path;
    use windows::core::{Error, Result};
    use windows::Win32::Foundation::{
        CloseHandle, MAX_PATH, HANDLE, HWND, POINT,
    };
    use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, WindowFromPoint, GetWindowThreadProcessId,
    };


    pub fn get_app_under_cursor() -> Result<Option<String>> {
        // SAFETY: Calls to Windows API functions are unsafe.
        unsafe {
            // 1. Get cursor position
            let mut point = POINT { x: 0, y: 0 };
            GetCursorPos(&mut point)?; // Propagate error if this fails

            // 2. Get window handle from point
            let hwnd = WindowFromPoint(point); // hwnd is HWND
            if hwnd.is_invalid() {
                // println!("No window found at point: {:?}", point); // Optional: reduce noise
                return Ok(None); // Return None if no window found
            }

            // Directly use the HWND from WindowFromPoint
            let final_hwnd: HWND = hwnd;

            // 4. Get Process ID (PID) from window handle
            let mut process_id: u32 = 0;
            let thread_id = GetWindowThreadProcessId(final_hwnd, Some(&mut process_id));
            if thread_id == 0 || process_id == 0 { // Treat PID 0 as failure to ID
                // Optional: reduce noise by removing print
                // println!("Failed to get valid PID for HWND({:?}): {:?}", final_hwnd, Error::from_win32());
                return Ok(Some("[System Process or No PID]".to_string())); // Indicate unable to ID
            }

            // 5. Open the process to query information
             let process_handle = match OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id) {
                 Ok(handle) => handle,
                 Err(_e) => { // Ignore the specific error details for cleaner output
                    // println!("Failed to open process (PID: {}): {:?}", process_id, e);
                    return Ok(Some(format!("[Access Denied/Error PID {}]", process_id)));
                 }
             };

            // RAII Handle Guard
            struct HandleGuard(HANDLE);
            impl Drop for HandleGuard {
                fn drop(&mut self) {
                    if !self.0.is_invalid() {
                        unsafe { CloseHandle(self.0); }
                    }
                }
            }
            let _handle_guard = HandleGuard(process_handle);

            // 6. Get executable path from process handle
            let mut exe_path_buf: Vec<u16> = vec![0; MAX_PATH as usize];
            let path_len = GetModuleFileNameExW(
                Some(process_handle),
                None,
                &mut exe_path_buf
            );

            if path_len == 0 {
                // let err = Error::from_win32();
                // println!("Error getting module filename (PID: {}): {:?}", process_id, err);
                return Ok(Some(format!("[Unknown Path PID {}]", process_id)));
            }

            // 7. Convert path buffer to Rust String
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

#[cfg(target_os = "windows")]
fn main() {
    println!("Continuously detecting application under the mouse cursor (Windows only)...");
    println!("Press Ctrl+C to stop.");

    // --- Ctrl+C Handling Setup ---
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("\nCtrl+C detected. Exiting...");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    // --- State for Change Detection ---
    let mut last_detected_app: Option<String> = None;
    let check_interval = Duration::from_secs(1);

    // --- Main Loop ---
    while running.load(Ordering::SeqCst) {
        let loop_start_time = std::time::Instant::now(); // For consistent sleep

        match windows_impl::get_app_under_cursor() {
            Ok(Some(app_name)) => {
                // Check if app changed since last detection
                if last_detected_app.as_ref() != Some(&app_name) {
                    println!("App under cursor: {}", app_name);
                    last_detected_app = Some(app_name);
                }
                // Else: Same app, do nothing
            }
            Ok(None) => {
                // No window detected under cursor
                if last_detected_app.is_some() { // Only print if it changed from something
                    println!("App under cursor: <None>");
                    last_detected_app = None;
                }
            }
            Err(e) => {
                // An error occurred (likely GetCursorPos)
                // Consider only printing errors if they persist or change
                if last_detected_app.as_deref() != Some("[API Error]") {
                    eprintln!("Error detecting window: {:?}", e);
                    last_detected_app = Some("[API Error]".to_string()); // Track error state
                }
            }
        }

        // --- Sleep ---
        let elapsed = loop_start_time.elapsed();
        if elapsed < check_interval {
            thread::sleep(check_interval - elapsed);
        }
    }

    println!("Stopped.");
}

// Dummy main for non-Windows
#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("This example currently only supports Windows.");
}
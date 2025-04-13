// This code is specific to Windows.
#[cfg(target_os = "windows")]
mod windows_impl {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::Path;
    use windows::core::{Error, Result}; // Ensure both Error and Result are imported
    use windows::Win32::Foundation::{
        CloseHandle, MAX_PATH, HANDLE, HWND, POINT,
    };
    use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, WindowFromPoint, GetWindowThreadProcessId, // Removed GetAncestor, GetParent, GA_ROOTOWNER
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
                println!("No window found at point: {:?}", point);
                return Ok(None);
            }

            // --- Simplification ---
            // Directly use the HWND from WindowFromPoint. Removed GetAncestor/GetParent logic.
            let final_hwnd: HWND = hwnd;
            // --- End Simplification ---


            // 4. Get Process ID (PID) from window handle
            let mut process_id: u32 = 0;
            // GetWindowThreadProcessId returns thread ID, 0 means failure. PID is output param.
            let thread_id = GetWindowThreadProcessId(final_hwnd, Some(&mut process_id));
            if thread_id == 0 {
                // Use Error::from_win32() to capture potential error code.
                println!("Failed to get Thread/Process ID for HWND({:?}): {:?}", final_hwnd, Error::from_win32());
                // Return Ok(Some(...)) indicating we couldn't get details for this specific HWND
                return Ok(Some(format!("[System Process or Error Getting PID: {:?}]", Error::from_win32())));
            }
             if process_id == 0 {
                 // Sometimes thread_id is non-zero but process_id remains 0 for certain system windows.
                 println!("Got valid thread ID ({}) but Process ID is 0 for HWND({:?})", thread_id, final_hwnd);
                 return Ok(Some("[System Process - No PID]".to_string()));
             }


            // 5. Open the process to query information
             let process_handle = match OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id) {
                 Ok(handle) => handle,
                 Err(e) => {
                    println!("Failed to open process (PID: {}): {:?}", process_id, e);
                    // Return Ok(Some(...)) indicating we found the PID but couldn't open it (e.g., access denied)
                    return Ok(Some(format!("[Access Denied or Error Opening PID {}: {:?}]", process_id, e)));
                 }
            };


            // RAII Handle Guard
            struct HandleGuard(HANDLE);
            impl Drop for HandleGuard {
                fn drop(&mut self) {
                    // Ensure handle is valid before attempting to close
                    if !self.0.is_invalid() {
                        unsafe { CloseHandle(self.0); }
                    }
                }
            }
            let _handle_guard = HandleGuard(process_handle); // Handle is closed when guard goes out of scope


            // 6. Get executable path from process handle
            let mut exe_path_buf: Vec<u16> = vec![0; MAX_PATH as usize];
            // Call GetModuleFileNameExW
            let path_len = GetModuleFileNameExW(
                Some(process_handle), // Option<HANDLE>
                None,                 // Option<HMODULE> - None gets the main executable
                &mut exe_path_buf     // &mut [u16]
            );


            if path_len == 0 {
                 // Failed to get path, capture error
                 let err = Error::from_win32();
                 println!("Error getting module filename (PID: {}): {:?}", process_id, err);
                 // Return Ok(Some(...)) indicating failure to get path for this PID
                 return Ok(Some(format!("[Unknown App Path PID {} - Error: {:?}]", process_id, err)));
            }


            // 7. Convert path buffer to Rust String
            let os_string = OsString::from_wide(&exe_path_buf[..path_len as usize]);
            if let Some(path_str) = os_string.to_str() {
                // Extract just the filename
                let app_name = Path::new(path_str)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "[Invalid Path]".to_string());
                Ok(Some(app_name)) // Successfully got the app name
            } else {
                Ok(Some("[Non-UTF8 Path]".to_string())) // Path wasn't valid UTF-8
            }
        }
    }
}

// Main function remains the same
#[cfg(target_os = "windows")]
fn main() {
    println!("Detecting application under the mouse cursor (Windows only)...");
    match windows_impl::get_app_under_cursor() {
        Ok(Some(app_name)) => {
             println!("----------------------------------------");
             println!("App under cursor: {}", app_name);
             println!("----------------------------------------");
        }
        Ok(None) => {
            println!("----------------------------------------");
            println!("Could not determine window or app under cursor.");
            println!("----------------------------------------");
        }
        Err(e) => {
             // This should now primarily catch errors propagated by '?' from GetCursorPos
             eprintln!("----------------------------------------");
             eprintln!("A Windows API error occurred (likely GetCursorPos): {:?}", e);
             eprintln!("----------------------------------------");
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("This example currently only supports Windows.");
    eprintln!("Detecting the window under the cursor is platform-specific and complex, especially on Linux/Wayland and macOS.");
}
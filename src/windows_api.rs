// src/windows_api.rs

use crate::errors::{AppError, AppResult}; // Import AppError and AppResult
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
// Remove windows::core::Result import if no longer needed directly
// use windows::core::Result;
// Remove PWSTR import as it's handled by the wrapper
// use windows::core::PWSTR;
use windows::Win32::Foundation::{
    CloseHandle, MAX_PATH, HANDLE, HWND, POINT, // Keep essential Foundation types
};
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, WindowFromPoint, GetWindowThreadProcessId,
    GetWindowTextW, // Keep this import
};


// Define a reasonable max length for window titles
const MAX_TITLE_LENGTH: usize = 512;

// Function to get the app name AND window title under the cursor
// Returns AppResult containing Option<(AppName, WindowTitle)>
pub fn get_app_under_cursor() -> AppResult<Option<(String, String)>> {
    // SAFETY: Calls to Windows API functions are unsafe.
    unsafe {
        let mut point = POINT { x: 0, y: 0 };
        // Use map_err to convert windows::core::Error to AppError::Platform
        GetCursorPos(&mut point)
            .map_err(|e| AppError::Platform(format!("GetCursorPos failed: {}", e)))?; // Now '?' propagates AppError

        let hwnd = WindowFromPoint(point);
        if hwnd.is_invalid() {
            // Return Ok(None) if no window is found - this isn't an error condition
            return Ok(None);
        }
        // Use the HWND directly returned by WindowFromPoint
        let final_hwnd: HWND = hwnd;

        // --- Get Window Title ---
        let mut title_buf: Vec<u16> = vec![0; MAX_TITLE_LENGTH];
        // Call the GetWindowTextW wrapper, passing the mutable slice directly.
        // It returns the number of characters copied (as i32) or 0 on failure/empty title.
        let title_len = GetWindowTextW(
            final_hwnd,
            &mut title_buf // Pass the buffer as &mut [u16]
        );

        let window_title = if title_len > 0 {
            // Convert the valid part of the buffer to a String
            let title_os = OsString::from_wide(&title_buf[..title_len as usize]);
            title_os.to_string_lossy().into_owned()
        } else {
            // Could check GetLastError here if needed, but often returns 0 even for empty titles.
            // Default to empty string for simplicity.
            String::new()
        };
        // --- End Get Window Title ---


        // --- Get Process ID and App Name ---
        let mut process_id: u32 = 0;
        let thread_id = GetWindowThreadProcessId(final_hwnd, Some(&mut process_id));

        // Determine App Name or return special strings for errors/system processes
        let app_name_or_error_str: String = if thread_id == 0 || process_id == 0 {
            let win_err = windows::core::Error::from_win32();
            format!("[System Process or No PID: {:?}]", win_err)
        } else {
            // Try to open process and get executable name
            match OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id) {
                Ok(process_handle) => {
                    // Use RAII guard for the handle
                    struct HandleGuard(HANDLE);
                    impl Drop for HandleGuard {
                        fn drop(&mut self) {
                            if !self.0.is_invalid() {
                                let _ = unsafe { CloseHandle(self.0) };
                            }
                        }
                    }
                    let _handle_guard = HandleGuard(process_handle); // Handle closed when guard drops

                    let mut exe_path_buf: Vec<u16> = vec![0; MAX_PATH as usize];
                    let path_len = GetModuleFileNameExW(Some(process_handle), None, &mut exe_path_buf);

                    if path_len == 0 {
                        let win_err = windows::core::Error::from_win32();
                        format!("[Unknown Path PID {} - Detail: {:?}]", process_id, win_err)
                    } else {
                        let os_string = OsString::from_wide(&exe_path_buf[..path_len as usize]);
                        if let Some(path_str) = os_string.to_str() {
                            Path::new(path_str)
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "[Invalid Path]".to_string())
                        } else {
                            "[Non-UTF8 Path]".to_string()
                        }
                    }
                }
                Err(e) => {
                    format!("[Access Denied/Error PID {} - Detail: {:?}]", process_id, e)
                }
            }
        };
        // --- End Get Process ID and App Name ---

        // Return tuple (app_name_or_error_str, window_title) wrapped in Ok(Some(...))
        // The caller (run.rs) now receives this tuple within the Ok variant.
        Ok(Some((app_name_or_error_str, window_title)))
    }
}
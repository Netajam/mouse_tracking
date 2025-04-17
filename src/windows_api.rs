// src/windows_api.rs

use crate::errors::{AppError, AppResult};
use std::ffi::OsString; // Keep ptr and mem if used by EnumWindows callback data pointer
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
use windows::core::BOOL;
use windows::Win32::Foundation::{
    CloseHandle, MAX_PATH, HANDLE, HWND, LPARAM // Keep LPARAM/BOOL for EnumWindows
};
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, WindowFromPoint, GetWindowThreadProcessId,
    GetWindowTextW, GetAncestor, GA_ROOTOWNER,
    EnumWindows, IsWindowVisible // Keep EnumWindows imports
};
use log::{debug, warn}; // Import log macros

const MAX_TITLE_LENGTH: usize = 512;

// --- EnumWindows Callback Setup ---
// Keep this struct as it's needed for enumeration
#[derive(Debug)] // Add Debug for logging if needed
struct EnumWindowsCallbackData {
    pid: u32,
    windows: Vec<(HWND, String)>, // Store HWND and Title
}

// Keep the callback function
extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let data = &mut *(lparam.0 as *mut EnumWindowsCallbackData);
        let mut window_pid: u32 = 0;
        let thread_id = GetWindowThreadProcessId(hwnd, Some(&mut window_pid));

        if thread_id != 0 && window_pid == data.pid && IsWindowVisible(hwnd).as_bool() {
            let title = get_hwnd_title(hwnd); // Use helper
            // Filter generic titles found during enumeration
            if !title.is_empty() && !is_generic_title(&title) {
                // Optionally log found sibling titles
                // debug!("  -> Found sibling HWND {:?} with title: '{}'", hwnd, title);
                data.windows.push((hwnd, title));
            }
        }
        BOOL(1) // Continue enumeration
    }
}

// Keep the helper to check for common generic/internal titles
fn is_generic_title(title: &str) -> bool {
    matches!(title, "Chrome Legacy Window" | "MSCTFIME UI" | "Default IME" | "") // Add more if needed
}
// --- End EnumWindows Callback Setup ---


// --- Main Public Function ---
pub fn get_detailed_window_info() -> AppResult<Option<(String, String, String)>> { // (app, main_title, detailed_title)
    unsafe {
        let mut point = Default::default();
        GetCursorPos(&mut point).map_err(|e| AppError::Platform(format!("GetCursorPos failed: {}", e)))?;
        debug!("Cursor position: ({}, {})", point.x, point.y);

        let hwnd_under_cursor = WindowFromPoint(point);
        if hwnd_under_cursor.is_invalid() {
            debug!("No window found under cursor.");
            return Ok(None);
        }
        debug!("HWND under cursor: {:?}", hwnd_under_cursor);

        // --- Get Title Directly Under Cursor (First candidate for detailed) ---
        let title_under_cursor = get_hwnd_title(hwnd_under_cursor);
        debug!("Title directly under cursor: '{}'", title_under_cursor);

        // --- Find Ancestor for PID and Main Title candidate ---
        let ancestor_hwnd = match GetAncestor(hwnd_under_cursor, GA_ROOTOWNER) {
           root_hwnd if !root_hwnd.is_invalid() => {
               debug!("Ancestor HWND found: {:?}", root_hwnd);
               root_hwnd
           },
           _ => {
               debug!("No valid ancestor found, using HWND under cursor.");
               hwnd_under_cursor
           },
        };

        // --- Get PID from Ancestor ---
        let mut process_id: u32 = 0;
        let thread_id = GetWindowThreadProcessId(ancestor_hwnd, Some(&mut process_id));
        debug!("PID from ancestor HWND: {} (Thread ID: {})", process_id, thread_id);

        // --- Get App Name ---
        let app_name = get_process_executable_name(process_id, thread_id)?;
        debug!("App name from PID {}: '{}'", process_id, app_name);

        // --- Get Ancestor Window Title (Candidate for Main) ---
        let ancestor_title = get_hwnd_title(ancestor_hwnd);
        debug!("Ancestor title: '{}'", ancestor_title);
        // Assign placeholder if empty
        let final_main_title = if ancestor_title.is_empty() {
            "[No Main Title]".to_string()
        } else {
            ancestor_title
        };
        debug!("Using final main title: '{}'", final_main_title);
        // --- End Main Window Title ---


        // --- Find Best Detailed Title via Enumeration ---
        let mut enum_title = String::new(); // Candidate from enumeration
        if process_id != 0 {
            debug!("Enumerating windows for PID {}...", process_id);
            let mut callback_data = EnumWindowsCallbackData { pid: process_id, windows: Vec::new() };
            EnumWindows(Some(enum_windows_callback), LPARAM(&mut callback_data as *mut _ as isize));
            debug!("Enumeration found {} potential sibling windows.", callback_data.windows.len());

            // Heuristic: Find the longest non-generic title among enumerated siblings
             if let Some((hwnd, title)) = callback_data.windows.iter().max_by_key(|(_, t)| t.len()) {
                 debug!("Longest sibling title found ('{}' on {:?}).", title, hwnd);
                 enum_title = title.clone(); // Use this candidate
             } else {
                  debug!("No suitable sibling title found via enumeration.");
             }
        }
        // --- End Enumeration ---


        // --- Determine Final Detailed Title ---
        let final_detailed_title = if !enum_title.is_empty() && enum_title != final_main_title {
            // Use title from enumeration if it's valid and different from main
            debug!("Using enumerated title for detailed: '{}'", enum_title);
            enum_title
        } else if !title_under_cursor.is_empty() && !is_generic_title(&title_under_cursor) && title_under_cursor != final_main_title {
            // Fallback 1: Use title under cursor if valid and different from main
            debug!("Using title under cursor for detailed: '{}'", title_under_cursor);
            title_under_cursor
        } else {
            // Fallback 2: Use the main title if others aren't suitable/different
            debug!("Using main title as detailed title fallback.");
            final_main_title.clone()
        };
        // --- End Detailed Title ---


        Ok(Some((app_name, final_main_title, final_detailed_title)))
    }
}
// --- Helper Function to Get Title for a specific HWND ---
unsafe fn get_hwnd_title(hwnd: HWND) -> String {
    let mut title_buf: Vec<u16> = vec![0; MAX_TITLE_LENGTH];
    let title_len = GetWindowTextW(hwnd, &mut title_buf);
    if title_len > 0 {
        OsString::from_wide(&title_buf[..title_len as usize]).to_string_lossy().into_owned()
    } else {
        String::new()
    }
}

// --- Helper Function to Get Process Executable Name ---
unsafe fn get_process_executable_name(process_id: u32, thread_id: u32) -> AppResult<String> {
    if thread_id == 0 || process_id == 0 {
        let win_err = windows::core::Error::from_win32();
        warn!("Could not get valid PID/ThreadID: {:?}", win_err);
        Ok(format!("[System Process or No PID: {:?}]", win_err))
    } else {
        match OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id) {
            Ok(process_handle) => {
                struct HandleGuard(HANDLE);
                impl Drop for HandleGuard { fn drop(&mut self) { if !self.0.is_invalid() { let _ = unsafe { CloseHandle(self.0) }; } } }
                let _handle_guard = HandleGuard(process_handle);

                let mut exe_path_buf: Vec<u16> = vec![0; MAX_PATH as usize];
                let path_len = GetModuleFileNameExW(Some(process_handle), None, &mut exe_path_buf);

                if path_len == 0 {
                    let win_err = windows::core::Error::from_win32();
                    warn!("GetModuleFileNameExW failed for PID {}: {:?}", process_id, win_err);
                    Ok(format!("[Unknown Path PID {} - Detail: {:?}]", process_id, win_err))
                } else {
                    let os_string = OsString::from_wide(&exe_path_buf[..path_len as usize]);
                    if let Some(path_str) = os_string.to_str() {
                        Ok(Path::new(path_str).file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| "[Invalid Path]".to_string()))
                    } else {
                         warn!("Executable path for PID {} is not valid UTF-8.", process_id);
                        Ok("[Non-UTF8 Path]".to_string())
                    }
                }
            }
            Err(e) => {
                warn!("OpenProcess failed for PID {}: {}", process_id, e);
                 Ok(format!("[Access Denied/Error PID {} - Detail: {:?}]", process_id, e))
            }
        }
    }
}
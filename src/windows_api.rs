// src/windows_api.rs

// Keep necessary imports within this file
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
use windows::core::Result; // Error might not be needed here if not used
use windows::Win32::Foundation::{
    CloseHandle, MAX_PATH, HANDLE, HWND, POINT,
};
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, WindowFromPoint, GetWindowThreadProcessId,
};

// Function to get the app under the cursor (Windows specific)
pub fn get_app_under_cursor() -> Result<Option<String>> {
    // SAFETY: Calls to Windows API functions are unsafe.
    unsafe {
        let mut point = POINT { x: 0, y: 0 };
        GetCursorPos(&mut point)?;
        let hwnd = WindowFromPoint(point);
        if hwnd.is_invalid() { return Ok(None); }
        let final_hwnd: HWND = hwnd;
        let mut process_id: u32 = 0;
        let thread_id = GetWindowThreadProcessId(final_hwnd, Some(&mut process_id));
         if thread_id == 0 || process_id == 0 {
            return Ok(Some("[System Process or No PID]".to_string()));
        }
         let process_handle = match OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id) {
             Ok(handle) => handle, Err(_) => { return Ok(Some(format!("[Access Denied/Error PID {}]", process_id))); }
         }; 
         // RAII Handle Guard
         struct HandleGuard(HANDLE);
         impl Drop for HandleGuard {
             fn drop(&mut self) {
                 if !self.0.is_invalid() {
                     // Explicitly ignore the result of CloseHandle
                     let _ = unsafe { CloseHandle(self.0) }; // Add `let _ = `
                 }
             }
         }
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
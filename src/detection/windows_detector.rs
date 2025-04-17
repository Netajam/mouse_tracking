// src/detection/windows_detector.rs
#![cfg(target_os = "windows")] // Only compile this file on Windows

use super::{ActivityDetector, ActivityInfo}; // Use trait/struct from parent mod
use crate::errors::AppResult;
use crate::windows_api; // Use the existing windows_api module

pub struct WindowsDetector; // Simple struct, might hold state later if needed

impl WindowsDetector {
    pub fn new() -> AppResult<Self> {
        // Add any Windows-specific initialization if required
        Ok(Self)
    }
}

impl ActivityDetector for WindowsDetector {
    fn get_current_activity(&self) -> AppResult<Option<ActivityInfo>> {
        // Call the existing windows_api function
        let detection_result = windows_api::get_detailed_window_info()?; // Propagate errors

        // Map the result to the common ActivityInfo struct
        Ok(detection_result.map(|(app, main, detailed)| ActivityInfo {
            app_name: app,
            main_title: main,
            detailed_title: detailed,
        }))
    }
}
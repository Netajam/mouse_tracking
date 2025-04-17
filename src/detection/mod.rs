// src/detection/mod.rs
use crate::errors::AppResult; // Or define a more specific DetectionError
#[cfg(target_os = "windows")] // Optional: Only compile the file if targeting windows
mod windows_detector;
// Define the data structure the detector should return
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityInfo {
   pub app_name: String,
   pub main_title: String,
   pub detailed_title: String,
}

// Define the trait
pub trait ActivityDetector {
    // Returns Ok(None) if no relevant activity detected (e.g., desktop, screen saver)
    // Returns Ok(Some(ActivityInfo)) if an app/window is detected
    // Returns Err on platform API errors
    fn get_current_activity(&self) -> AppResult<Option<ActivityInfo>>;
}

// Factory function to create the appropriate detector
pub fn create_detector() -> AppResult<Box<dyn ActivityDetector>> {
    cfg_if::cfg_if! {
        if #[cfg(target_os = "windows")] {
            // Conditionally compile the windows module import
            Ok(Box::new(windows_detector::WindowsDetector::new()?))
        } else if #[cfg(target_os = "macos")] {
             // Placeholder for macOS
             // mod macos_detector;
             // Ok(Box::new(macos_detector::MacosDetector::new()?))
             Err(crate::errors::AppError::Platform("macOS detection not yet implemented".to_string()))
        } else if #[cfg(target_os = "linux")] {
             // Placeholder for Linux
             // mod linux_detector;
             // Ok(Box::new(linux_detector::LinuxDetector::new()?))
             Err(crate::errors::AppError::Platform("Linux detection not yet implemented".to_string()))
        } else {
            Err(crate::errors::AppError::Platform("Unsupported platform for activity detection".to_string()))
        }
    }
}
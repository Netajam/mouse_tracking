// src/utils.rs
use std::time::Duration;

// Formats std::time::Duration
pub fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

// Formats total seconds (i64)
pub fn format_duration_secs(total_seconds: i64) -> String {
    if total_seconds < 0 { return "Invalid".to_string(); }
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
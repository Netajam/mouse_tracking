// src/config.rs

use std::time::Duration;

// === Application Info ===
// Used by the self-update mechanism.
// These should match your GitHub repository.
pub const GITHUB_REPO_OWNER: &'static str = "Netajam";
pub const GITHUB_REPO_NAME: &'static str = "mouse_tracking";
pub const APP_NAME: &'static str = "mouse_tracking";
// === Tracker Settings ===
// How often to check the window under the cursor (in seconds).
pub const CHECK_INTERVAL_SECONDS: u64 = 1;

// Pre-calculated Duration version for convenience.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(CHECK_INTERVAL_SECONDS);

// === Persistence Settings ===
// Filename for the SQLite database.
pub const DATABASE_FILENAME: &'static str = "app_usage.sqlite";
pub const DANGLING_INTERVAL_RECENT_THRESHOLD_SECS: i64 = 24 * 60 * 60; // 1 day (in seconds)

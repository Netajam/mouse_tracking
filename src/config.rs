// src/config.rs

use std::path::PathBuf;
use std::time::Duration;
use crate::errors::{AppError, AppResult}; // Use AppResult for loading errors

pub const KEYRING_SERVICE_NAME_PREFIX: &str = "llm-cli-"; // Or your preferred prefix

// Define the struct to hold application configuration
#[derive(Debug, Clone)]
pub struct AppConfig {
    // Persistence
    pub database_path: PathBuf,
    pub dangling_threshold_secs: i64,

    // Update
    pub repo_owner: String,
    pub repo_name: String,

    // Tracking
    pub check_interval: Duration,

    // General App Info (can still be derived or stored here)
    pub app_name: String,
    pub app_version: String,

    //Api keys
    pub keyring_service_name: String, 

}

// Function to determine and load the application configuration
// This is where we'll centralize logic for finding paths,
// reading files (later), parsing args (later), etc.
pub fn load_configuration() -> AppResult<AppConfig> { // Return AppResult

    // --- Determine Base Values (Compile time) ---
    let base_app_name = env!("CARGO_PKG_NAME").to_string();
    let app_version = env!("CARGO_PKG_VERSION").to_string();

    // --- Determine Runtime Values ---

    // Database Path (using build profile for dev/release differentiation for now)
    let mut dir_name = base_app_name.clone(); // Clone base name
    let is_dev_build = cfg!(debug_assertions);
    let mut unique_name_part = base_app_name.clone(); 

    if is_dev_build {
        dir_name.push_str("-dev"); // Append suffix for debug builds
        println!("[Debug Build Detected] Using data directory suffix: -dev");
        unique_name_part.push_str("-dev"); // Append suffix for debug builds

    }

    let mut db_dir_path = dirs::data_dir()
        // Map Option error to our custom error type
        .ok_or_else(|| AppError::DataDir("Could not find user data directory.".to_string()))?;

    db_dir_path.push(&dir_name); // Use determined directory name

    // Ensure the directory exists before adding filename
    if !db_dir_path.exists() {
        std::fs::create_dir_all(&db_dir_path)
            // Map IO error to our custom error type, including context
            .map_err(|e| AppError::Io { path: db_dir_path.clone(), source: e })?;
    }

    let database_path = db_dir_path.join("app_usage.sqlite"); // Use a filename constant?
 
    // Other Config Values (currently hardcoded, could load from file/env later)
    let repo_owner = "Netajam".to_string(); // Replace with your owner
    let repo_name = base_app_name.clone(); // Use base name for repo too
    let check_interval_secs = 1;
    let check_interval = Duration::from_secs(check_interval_secs);
    let dangling_threshold_secs = 24 * 60 * 60; // 1 day
    
    let keyring_service_name = format!("{}{}", KEYRING_SERVICE_NAME_PREFIX, unique_name_part);
    log::debug!("Derived keyring service name: {}", keyring_service_name); // Log derived name
    // --- Construct the AppConfig struct ---
    Ok(AppConfig {
        database_path,
        dangling_threshold_secs,
        repo_owner,
        repo_name,
        check_interval,
        app_name: base_app_name, // Store derived app name
        app_version,             // Store derived version
        keyring_service_name, 
    })
}

// Optional: Define constants for default values if needed elsewhere
// pub const DEFAULT_CHECK_INTERVAL_SECS: u64 = 1;
// pub const DEFAULT_DATABASE_FILENAME: &'static str = "app_usage.sqlite";
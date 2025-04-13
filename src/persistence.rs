// src/persistence.rs
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationSecondsWithFrac};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
    time::Duration,
}; // Add necessary std imports

// Define a struct for our persistent data
#[serde_as]
#[derive(Serialize, Deserialize, Default, Debug, Clone)] // Add Clone
pub struct AppTimeData {
    #[serde_as(as = "HashMap<_, DurationSecondsWithFrac<String>>")]
    pub times: HashMap<String, Duration>, // Make field public if needed elsewhere, or add methods
}

// Function to get the data file path
pub fn get_data_file_path() -> Result<PathBuf, String> {
    match dirs::data_dir() {
        Some(mut path) => {
            path.push("RustAppTimeTracker"); // Subdirectory for our app data
            path.push("cursor_app_times.json"); // Filename
            Ok(path)
        }
        None => Err("Could not find user data directory.".to_string()),
    }
}

// Function to load existing data
pub fn load_data(path: &Path) -> AppTimeData {
    if !path.exists() {
        println!("Data file not found at {:?}. Starting fresh.", path);
        return AppTimeData::default();
    }
    match File::open(path) {
        Ok(file) => {
            let reader = BufReader::new(file);
            match serde_json::from_reader(reader) {
                Ok(data) => {
                    println!("Successfully loaded data from {:?}", path);
                    data
                }
                Err(e) => {
                    eprintln!("Error parsing data file {:?}: {}. Starting fresh.", path, e);
                    AppTimeData::default()
                }
            }
        }
        Err(e) => {
            eprintln!("Error opening data file {:?}: {}. Starting fresh.", path, e);
            AppTimeData::default()
        }
    }
}

// Function to save data
pub fn save_data(path: &Path, data: &AppTimeData) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                return Err(format!("Failed to create data directory {:?}: {}", parent, e));
            }
        }
    }
    match File::create(path) {
        Ok(file) => {
            let writer = BufWriter::new(file);
            match serde_json::to_writer_pretty(writer, data) {
                Ok(_) => {
                    println!("Successfully saved data to {:?}", path);
                    Ok(())
                }
                Err(e) => Err(format!("Failed to serialize data to JSON: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to create/open data file {:?} for writing: {}", path, e)),
    }
}
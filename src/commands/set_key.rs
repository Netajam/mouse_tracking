// src/commands/set_key.rs 

use crate::config::AppConfig;
use crate::errors::{AppError, AppResult};
use crate::types::{ApiKeyType, ConfigCommand}; // Import the ApiKeyType enum
use keyring::Entry;
use rpassword::prompt_password;
use log; // Use the log crate facade
use clap::ValueEnum; // <--- Added based on Problem 2

// --- Main Execution Function ---

/// Execute configuration-related commands (currently only set-key)
pub fn execute_config_command(app_config: &AppConfig, command: ConfigCommand) -> AppResult<()> { // Renamed function example
    match command {
        ConfigCommand::SetKey { key_type } => {
            log::info!("Executing set-key command for type: {:?}", key_type);
            set_api_key(app_config, key_type)?;
        }
    }
    Ok(())
}

// --- Helper Functions ---

/// Prompts the user for an API key of the specified type and saves it securely.
fn set_api_key(app_config: &AppConfig, key_type: ApiKeyType) -> AppResult<()> {
    log::debug!("Attempting to set API key for type: {}", key_type);
    let keyring_username = key_type.keyring_username();
    log::debug!(
        "Using keyring service: '{}', username: '{}'",
        app_config.keyring_service_name,
        keyring_username
    );

    // Create keyring entry - ? now works because AppError implements From<keyring::Error>
    let entry = Entry::new(&app_config.keyring_service_name, keyring_username)?;

    println!(
        "Enter your {} API Key (input will be hidden, press Enter when done):",
        key_type
    );
    // Prompt password - ? now works because AppError implements From<std::io::Error> via PasswordInput
    let api_key = prompt_password("API Key: ")?;

    if api_key.trim().is_empty() {
        log::warn!("User provided an empty API key for type: {}", key_type);
        eprintln!("Error: API Key cannot be empty.");
        return Err(AppError::Config("API key cannot be empty.".to_string()));
    }

    log::info!("Attempting to save {} API Key to keyring...", key_type);
    // Set password - ? now works because AppError implements From<keyring::Error>
    entry.set_password(&api_key)?;

    drop(api_key);
    log::info!("{} API Key saved successfully to keyring.", key_type);
    println!("✅ {} API Key saved successfully.", key_type);

    Ok(())
}

/// Loads the API key of the specified type from the secure credential store.
pub fn load_api_key(app_config: &AppConfig, key_type: ApiKeyType) -> AppResult<String> {
    log::debug!("Attempting to load API key for type: {}", key_type);
    let keyring_username = key_type.keyring_username();
    log::debug!(
        "Looking in keyring service: '{}', username: '{}'",
        app_config.keyring_service_name,
        keyring_username
    );
    let entry = Entry::new(&app_config.keyring_service_name, keyring_username)?;

    match entry.get_password() {
        Ok(key) => {
            log::debug!("API Key type '{}' loaded successfully from keyring.", key_type);
            Ok(key)
        }
        Err(keyring::Error::NoEntry) => {
            log::warn!("API Key type '{}' not found in keyring.", key_type);

            // --- MODIFIED HERE ---
            // Get the CLI argument name as an owned String
            let cli_value_name: String = key_type
                .to_possible_value()
                .map(|pv| pv.get_name().to_string()) // Convert the &str to String
                .unwrap_or_else(|| { // Use unwrap_or_else for lazy evaluation of the fallback
                    log::error!("Could not get possible value name for ApiKeyType: {:?}", key_type);
                    "unknown".to_string() // Convert the fallback literal to String
                });

            // Pass the owned String to the error variant
            Err(AppError::ApiKeyNotFound(key_type, cli_value_name))
        }
        Err(e) => {
            log::error!(
                "Error loading API key type '{}' from keyring: {}",
                key_type,
                e
            );
            Err(AppError::Keyring(e)) // Propagate other keyring errors
        }
    }
}
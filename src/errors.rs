// src/errors.rs
use thiserror::Error;
use std::path::PathBuf;
use crate::types::ApiKeyType;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Data directory error: {0}")]
    DataDir(String),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("I/O error accessing path '{path}': {source}")]
    Io { path: PathBuf, #[source] source: std::io::Error },

    // ---> ADDED For rpassword errors <---
    #[error("Password input error: {0}")]
    PasswordInput(#[from] std::io::Error), // Use this specific variant for rpassword

    // ---> ADDED For keyring errors <---
    #[error("Keyring error: {0}")]
    Keyring(#[from] keyring::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Update check/download error: {0}")]
    Update(#[from] self_update::errors::Error),

    #[error("Platform API error (e.g., getting cursor/window info): {0}")]
    Platform(String),
    #[error("Argument parsing error: {0}")]
    CliArgs(#[from] clap::Error),

    #[error("Failed to set Ctrl-C handler: {0}")]
    CtrlC(#[from] ctrlc::Error),

    #[error("An unexpected error occurred: {0}")]
    Unexpected(String),

    #[error("{0} API Key not found. Please set it using the 'config set-key {1}' command.")]
    ApiKeyNotFound(ApiKeyType, String),
}

pub type AppResult<T> = Result<T, AppError>;
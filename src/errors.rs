// src/errors.rs 
use thiserror::Error;
use std::path::PathBuf;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Data directory error: {0}")]
    DataDir(String),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("I/O error accessing path '{path}': {source}")]
    Io { path: PathBuf, #[source] source: std::io::Error },

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
}

pub type AppResult<T> = Result<T, AppError>;
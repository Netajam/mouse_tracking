// src/commands/update.rs

use crate::config::AppConfig;
use crate::errors::AppResult;
use self_update;

// Change return type to AppResult<()>
pub fn execute(app_config:&AppConfig) -> AppResult<()> {
    println!("Checking for updates...");

    // Getting version with env! is fine, no error expected here
    let current_version = env!("CARGO_PKG_VERSION");
    println!("Current version: {}", current_version);

    // Use '?' - self_update::errors::Error will be automatically converted
    // to AppError::Update by the #[from] attribute in errors.rs
    let status = self_update::backends::github::Update::configure()
        .repo_owner(&app_config.repo_owner)
        .repo_name(&app_config.repo_name)
        .target(self_update::get_target())
        .bin_name(env!("CARGO_PKG_NAME")) 
        .show_download_progress(true)
        .current_version(current_version)
        .build()? 
        .update()?; 

    match status {
        self_update::Status::UpToDate(v) => {
            println!("Already running the latest version: {}", v);
        }
        self_update::Status::Updated(v) => {
            println!("Successfully updated to version: {}", v);
            println!("Please restart the application if it was running.");
        }
    }

    // Return Ok(()) on successful completion (either up-to-date or updated)
    Ok(())
}
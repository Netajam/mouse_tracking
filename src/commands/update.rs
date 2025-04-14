// src/commands/update.rs

use crate::config; 
use self_update;

pub fn execute() -> Result<(), Box<dyn std::error::Error>> {
    println!("Checking for updates...");

    let current_version = env!("CARGO_PKG_VERSION");
    println!("Current version: {}", current_version);

    let status = self_update::backends::github::Update::configure()
        .repo_owner(config::GITHUB_REPO_OWNER)
        .repo_name(config::GITHUB_REPO_NAME)
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

    Ok(())
}
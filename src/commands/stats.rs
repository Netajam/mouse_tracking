// src/commands/stats.rs

use crate::persistence::{self};
use crate::utils::format_duration_secs;
use crate::errors::AppResult; 
use std::path::Path;

// Change return type to AppResult<()>
pub fn execute(data_path: &Path) -> AppResult<()> {
     println!("Showing statistics...");
     println!("Database path: {:?}", data_path);

     // Use '?' - rusqlite::Error will be automatically converted to AppError::Database by #[from]
     let conn = persistence::open_connection(data_path)?;

     // Use '?' - rusqlite::Error will be automatically converted to AppError::Database by #[from]
     let stats_data = persistence::query_aggregated_stats(&conn)?;

     // --- Printing logic remains the same ---

     println!("\n--- Today's Summary ---");
     if stats_data.today.is_empty() {
         println!("No aggregated data found for today yet.");
     } else {
         for record in stats_data.today {
             println!("{:<40}: {}", record.app_name, format_duration_secs(record.total_duration_secs));
         }
     }

     println!("\n--- Last Completed Hour Summary ---");
     if stats_data.last_hour.is_empty() {
          println!("No aggregated data found for the last completed hour.");
     } else {
          for record in stats_data.last_hour {
              println!("{:<40}: {}", record.app_name, format_duration_secs(record.total_duration_secs));
          }
     }

     println!("\n--- Current Hour Summary (approximate) ---");
      if stats_data.current_hour.is_empty() {
          println!("No activity recorded yet for the current hour.");
     } else {
         for record in stats_data.current_hour {
            println!("{:<40}: {}", record.app_name, format_duration_secs(record.total_duration_secs));
         }
     }
     println!("---------------------------------------------");

     // Return Ok(()) on success
     Ok(())
}
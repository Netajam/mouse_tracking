// src/commands/stats.rs

use crate::persistence::{self, AppUsageRecord, StatsData}; // Import structs and module
use crate::utils::format_duration_secs; // Import the formatting function
use std::path::Path;

pub fn execute(data_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
     println!("Showing statistics...");
     println!("Database path: {:?}", data_path);

     let conn = persistence::open_connection(data_path)?;

     // Call the function in persistence to query data
     let stats_data = persistence::query_aggregated_stats(&conn)?;

     // --- Print Today's Summary ---
     println!("\n--- Today's Summary ---");
     if stats_data.today.is_empty() {
         println!("No aggregated data found for today yet.");
     } else {
         for record in stats_data.today {
             println!("{:<40}: {}", record.app_name, format_duration_secs(record.total_duration_secs));
         }
     }

     // --- Print Last Hour's Summary ---
     println!("\n--- Last Completed Hour Summary ---");
     if stats_data.last_hour.is_empty() {
          println!("No aggregated data found for the last completed hour.");
     } else {
          for record in stats_data.last_hour {
              println!("{:<40}: {}", record.app_name, format_duration_secs(record.total_duration_secs));
          }
     }

     // --- Print Current Hour's Summary ---
     println!("\n--- Current Hour Summary (approximate) ---");
      if stats_data.current_hour.is_empty() {
          println!("No activity recorded yet for the current hour.");
     } else {
         for record in stats_data.current_hour {
            println!("{:<40}: {}", record.app_name, format_duration_secs(record.total_duration_secs));
         }
     }
     println!("---------------------------------------------");


     Ok(())
}
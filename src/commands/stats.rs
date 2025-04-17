// src/commands/stats.rs

use crate::persistence::{self, DetailedUsageRecord, StatsData}; // Use DetailedUsageRecord
use crate::utils::format_duration_secs;
use crate::errors::AppResult;
use std::path::Path;
use std::collections::HashMap; // Needed for summarizing

// Helper function to summarize detailed records by app name
fn summarize_by_app(records: &[DetailedUsageRecord]) -> HashMap<String, i64> {
    let mut summary = HashMap::new();
    for record in records {
        *summary.entry(record.app_name.clone()).or_insert(0) += record.total_duration_secs;
    }
    summary
}

// Helper function to print a summarized map
fn print_summary_map(summary: HashMap<String, i64>) {
     if summary.is_empty() {
         println!("  No activity recorded.");
         return;
     }
    let mut sorted_summary: Vec<_> = summary.into_iter().collect();
    sorted_summary.sort_by(|a, b| b.1.cmp(&a.1)); // Sort descending by duration
    for (app, secs) in sorted_summary {
        println!("  {:<40}: {}", app, format_duration_secs(secs));
    }
}


pub fn execute(data_path: &Path) -> AppResult<()> {
     println!("Showing statistics...");
     println!("Database path: {:?}", data_path);

     let conn = persistence::open_connection_ensure_path(data_path)?; // Use correct open function

     let stats_data = persistence::query_aggregated_stats(&conn)?;

     // --- Print Summaries (Summarized by App Name) ---

     println!("\n--- Today's Summary (by App) ---");
     let today_summary = summarize_by_app(&stats_data.today);
     print_summary_map(today_summary);


     println!("\n--- Last Completed Hour Summary (by App) ---");
      let last_hour_summary = summarize_by_app(&stats_data.last_hour);
      print_summary_map(last_hour_summary);


     println!("\n--- Current Hour Summary (approximate, by App) ---");
      let current_hour_summary = summarize_by_app(&stats_data.current_hour);
      print_summary_map(current_hour_summary);


     // --- Optional: Add flag or section to print detailed view ---
     // println!("\n--- Today's Detailed View ---");
     // if stats_data.today.is_empty() { /* ... */ } else {
     //     for record in stats_data.today {
     //         println!("  App: {:<30} Title: {:<50} Time: {}", record.app_name, record.detailed_title, format_duration_secs(record.total_duration_secs));
     //     }
     // }

     println!("---------------------------------------------");

     Ok(())
}
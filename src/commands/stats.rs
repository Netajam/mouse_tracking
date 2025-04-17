// src/commands/stats.rs

use crate::persistence;
use crate::types::{AggregationLevel, AggregatedResult, DetailedUsageRecord, TimePeriod, AppResult}; // Make sure AppResult is imported
use crate::errors::AppError; // Import AppError if used in map_err
use crate::utils::format_duration_secs;
use std::path::Path;
use log::{error, info}; // Keep needed log macros

// --- Display helper functions (print_aggregated_by_app, print_detailed_view) ---
// (Keep the definitions for these functions here as provided before)

fn print_aggregated_by_app(results: &mut Vec<(String, i64)>) {
    if results.is_empty() { println!("  No activity recorded for this period."); return; }
    results.sort_by(|a, b| b.1.cmp(&a.1));
    let max_len = results.iter().map(|(name, _)| name.len()).max().unwrap_or(20).max(20);
    println!("  {:<width$} : {}", "Application", "Duration", width = max_len);
    println!("  {:-<width$} :----------", "", width = max_len);
    for (app, secs) in results { println!("  {:<width$} : {}", app, format_duration_secs(*secs), width = max_len); }
}

fn print_detailed_view(records: &mut Vec<DetailedUsageRecord>) {
     if records.is_empty() { println!("  No activity recorded for this period."); return; }
    records.sort_by(|a, b| b.total_duration_secs.cmp(&a.total_duration_secs));
    let max_app_len = records.iter().map(|r| r.app_name.len()).max().unwrap_or(20).max(15);
    let max_title_len = records.iter().map(|r| r.detailed_title.len()).max().unwrap_or(40).max(20);
    println!( "  {:<app_width$} | {:<title_width$} | {}", "Application", "Window Title", "Duration", app_width = max_app_len, title_width = max_title_len );
    println!( "  {:-<app_width$}-+-{:-<title_width$}-+----------", "", "", app_width = max_app_len, title_width = max_title_len );
    for record in records { println!( "  {:<app_width$} | {:<title_width$} | {}", record.app_name, record.detailed_title, format_duration_secs(record.total_duration_secs), app_width = max_app_len, title_width = max_title_len ); }
}


/// Helper function to display a section of stats based on the query result.
fn display_stats_section(
    title: &str,
    result: Result<AggregatedResult, rusqlite::Error>, // Receive SqlResult
    level: AggregationLevel,
) {
    println!("\n--- {} ({}) ---", title, level);

    match result {
        Ok(mut agg_result) => { // Make mutable for sorting
             if agg_result.is_empty() {
                 println!("  No activity recorded for this period.");
                 return;
             }
            match &mut agg_result{ // Match on mutable ref
                 AggregatedResult::ByApp(summary) => print_aggregated_by_app(summary),
                 AggregatedResult::Detailed(records) => print_detailed_view(records),
             }
        }
        Err(e) => {
            // Use log::error, not just error!
            log::error!("  Failed to query statistics for \"{}\": {}", title, e);
            println!("  Error retrieving data for this period.");
        }
    }
}

// --- The Command Execution Function ---
// *** ENSURE 'pub' IS PRESENT HERE ***
pub fn execute(data_path: &Path, level: AggregationLevel) -> AppResult<()> {
    // Use log::info, not just info!
    log::info!("Showing statistics with level: {:?}", level);
    println!("Statistics Level: {}", level);
    println!("Database path: {:?}", data_path);

    // Use the AppError type defined in errors.rs for mapping
    let conn = persistence::open_connection_ensure_path(data_path)
        .map_err(|e| AppError::Database(e))?; // Use #[from] implicitly via ? or map specifically

    let periods_to_display = [
        TimePeriod::Today,
        TimePeriod::LastCompletedHour,
        TimePeriod::CurrentHour,
    ];

    for period in periods_to_display {
        let result = persistence::query_stats(&conn, period, level);
        display_stats_section(&period.to_string(), result, level);
    }

    println!("\n---------------------------------------------");

    Ok(())
}
// src/persistence.rs
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult}; // Removed Transaction (not directly needed after fix)
use std::path::{Path, PathBuf};
use std::time::Duration; // Corrected import
use chrono::{Utc, TimeZone, Datelike, Timelike}; // Removed DateTime, ChronoDuration
use crate::utils::format_duration_secs; // Make sure format_duration_secs is accessible

// get_data_file_path remains the same
pub fn get_data_file_path() -> Result<PathBuf, String> {
    match dirs::data_dir() {
        Some(mut path) => {
            path.push("RustAppTimeTracker");
            path.push("app_usage.sqlite");
            if let Some(parent) = path.parent() {
                 if !parent.exists() {
                     if let Err(e) = std::fs::create_dir_all(parent) {
                         return Err(format!("Failed to create data directory {:?}: {}", parent, e));
                     }
                 }
            }
            Ok(path)
        }
        None => Err("Could not find user data directory.".to_string()),
    }
}

// open_connection remains the same
pub fn open_connection(path: &Path) -> SqlResult<Connection> {
    Connection::open(path)
}

// Updated initialize_db to take &mut Connection
pub fn initialize_db(conn: &mut Connection) -> SqlResult<()> { // <-- Takes &mut Connection
    // Use a transaction for schema changes. `transaction()` borrows conn mutably.
    let tx = conn.transaction()?;

    tx.execute( // Raw intervals table
        "CREATE TABLE IF NOT EXISTS app_intervals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            app_name TEXT NOT NULL,
            start_time INTEGER NOT NULL, -- Unix timestamp (seconds)
            end_time INTEGER            -- Unix timestamp (seconds), NULLable
        )", [],
    )?;
    tx.execute("CREATE INDEX IF NOT EXISTS idx_app_intervals_app_name ON app_intervals (app_name);", [])?;
    tx.execute("CREATE INDEX IF NOT EXISTS idx_app_intervals_start_time ON app_intervals (start_time);", [])?;
    tx.execute("CREATE INDEX IF NOT EXISTS idx_app_intervals_end_time ON app_intervals (end_time);", [])?;

    tx.execute( // Hourly summary table
        "CREATE TABLE IF NOT EXISTS hourly_summary (
            app_name TEXT NOT NULL,
            hour_timestamp INTEGER NOT NULL, -- Start of the hour timestamp
            total_duration_secs INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (app_name, hour_timestamp)
        )", [],
    )?;

     tx.execute( // Daily summary table
        "CREATE TABLE IF NOT EXISTS daily_summary (
            app_name TEXT NOT NULL,
            day_timestamp INTEGER NOT NULL, -- Start of the day timestamp
            total_duration_secs INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (app_name, day_timestamp)
        )", [],
    )?;

    tx.commit() // Commit transaction
}

// finalize_dangling_intervals takes &Connection (only reads/updates)
pub fn finalize_dangling_intervals(conn: &Connection, shutdown_time: i64) -> SqlResult<usize> {
     println!("Checking for dangling intervals from previous sessions...");
     let one_day_ago = shutdown_time - (24 * 60 * 60);
     // These execute calls don't require a mutable transaction
     let updated_old = conn.execute(
         "UPDATE app_intervals SET end_time = start_time WHERE end_time IS NULL AND start_time < ?",
         params![one_day_ago],
     )?;
     let updated_recent = conn.execute(
         "UPDATE app_intervals SET end_time = ? WHERE end_time IS NULL AND start_time >= ?",
         params![shutdown_time, one_day_ago],
     )?;
     let total_updated = updated_old + updated_recent;
     if total_updated > 0 {
        println!("Finalized {} dangling interval(s).", total_updated);
     }
     Ok(total_updated)
 }


// insert_new_interval takes &Connection (INSERT implicitly uses transactions)
pub fn insert_new_interval(conn: &Connection, app_name: &str, start_time: i64) -> SqlResult<i64> {
    conn.execute(
        "INSERT INTO app_intervals (app_name, start_time, end_time) VALUES (?1, ?2, NULL)",
        params![app_name, start_time],
    )?;
    Ok(conn.last_insert_rowid())
}

// finalize_interval takes &Connection (UPDATE implicitly uses transactions)
pub fn finalize_interval(conn: &Connection, row_id: i64, end_time: i64) -> SqlResult<usize> {
    conn.execute(
        "UPDATE app_intervals SET end_time = ?1 WHERE id = ?2 AND end_time IS NULL",
        params![end_time, row_id],
    )
}


pub fn aggregate_and_cleanup(conn: &mut Connection) -> SqlResult<()> {
    println!("Starting aggregation and cleanup...");
    let tx = conn.transaction()?;

    let now = Utc::now();
    let current_hour_start = now.date_naive().and_hms_opt(now.hour(), 0, 0).unwrap().and_utc().timestamp();

    let max_end_time_to_process: Option<i64> = tx.query_row(
        "SELECT MAX(end_time) FROM app_intervals WHERE end_time < ?",
        params![current_hour_start],
        |row| row.get(0),
    )?;

    if let Some(aggregate_until) = max_end_time_to_process {
         if aggregate_until >= current_hour_start {
             println!("Warning: MAX(end_time) {} is not less than current hour start {}. Skipping aggregation cycle.", aggregate_until, current_hour_start);
             return Ok(());
         }

         println!("Aggregating intervals completed before: {}", Utc.timestamp_opt(aggregate_until, 0).unwrap());

        // --- Aggregate into hourly_summary ---
        let hourly_rows = tx.execute(
            "INSERT INTO hourly_summary (app_name, hour_timestamp, total_duration_secs)
             SELECT
                 app_name,
                 -- Calculate hour_start here
                 CAST(strftime('%s', DATETIME(start_time, 'unixepoch', 'start of hour')) AS INTEGER) as hour_start,
                 SUM(end_time - start_time) as duration
             FROM app_intervals
             WHERE end_time IS NOT NULL AND end_time <= ?1 -- Process intervals ending before the cutoff
             -- FIX: Add condition to filter out rows where strftime result is NULL
               AND hour_start IS NOT NULL
             GROUP BY app_name, hour_start
             ON CONFLICT(app_name, hour_timestamp) DO UPDATE SET
                 total_duration_secs = total_duration_secs + excluded.total_duration_secs",
            params![aggregate_until], // Only one parameter needed here now
        )?;
        println!("-> Aggregated {} rows into hourly summary.", hourly_rows);

         // --- Aggregate into daily_summary ---
        let daily_rows = tx.execute(
            "INSERT INTO daily_summary (app_name, day_timestamp, total_duration_secs)
             SELECT
                 app_name,
                 -- Calculate day_start here
                 CAST(strftime('%s', DATETIME(start_time, 'unixepoch', 'start of day')) AS INTEGER) as day_start,
                 SUM(end_time - start_time) as duration
             FROM app_intervals
             WHERE end_time IS NOT NULL AND end_time <= ?1 -- Process same intervals
             -- FIX: Add condition to filter out rows where strftime result is NULL
               AND day_start IS NOT NULL
             GROUP BY app_name, day_start
             ON CONFLICT(app_name, day_timestamp) DO UPDATE SET
                 total_duration_secs = total_duration_secs + excluded.total_duration_secs",
            params![aggregate_until], // Only one parameter needed here now
        )?;
         println!("-> Aggregated {} rows into daily summary.", daily_rows);


        // --- Delete aggregated raw intervals ---
        // This remains the same, it only depends on aggregate_until
        let deleted_rows = tx.execute(
            "DELETE FROM app_intervals WHERE end_time IS NOT NULL AND end_time <= ?1",
            params![aggregate_until],
        )?;
        println!("-> Deleted {} processed raw interval rows.", deleted_rows);

    } else {
        println!("No completed intervals found to aggregate.");
    }

    tx.commit()?;
    println!("Aggregation and cleanup finished.");
    Ok(())
}

// calculate_and_print_stats (Enhanced Placeholder)
pub fn calculate_and_print_stats(conn: &Connection) -> SqlResult<()> {
    println!("\n--- Usage Statistics ---");

    // --- Today's Stats ---
    let today_start_ts = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
    println!("\n--- Today's Summary (from daily_summary) ---");
    let mut stmt_today = conn.prepare(
        "SELECT app_name, total_duration_secs
         FROM daily_summary
         WHERE day_timestamp = ?1
         ORDER BY total_duration_secs DESC"
    )?;
    let today_rows = stmt_today.query_map(params![today_start_ts], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut found_today = false;
    for row_result in today_rows {
        if let Ok((app, secs)) = row_result {
            println!("{:<40}: {}", app, format_duration_secs(secs));
            found_today = true;
        }
    }
     if !found_today { println!("No aggregated data found for today yet."); }

    // --- Last Completed Hour ---
    let now = Utc::now();
    // Timestamp for the start of the hour *before* the current one
    let last_completed_hour_start_ts = (now - chrono::Duration::hours(1))
                                       .date_naive()
                                       .and_hms_opt( (now - chrono::Duration::hours(1)).hour(), 0, 0).unwrap()
                                       .and_utc().timestamp();
    println!("\n--- Last Completed Hour Summary (from hourly_summary) ---");
    let mut stmt_last_hour = conn.prepare(
         "SELECT app_name, total_duration_secs
          FROM hourly_summary
          WHERE hour_timestamp = ?1
          ORDER BY total_duration_secs DESC"
     )?;
     let last_hour_rows = stmt_last_hour.query_map(params![last_completed_hour_start_ts], |row| {
         Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
     })?;
     let mut found_last_hour = false;
     for row_result in last_hour_rows {
         if let Ok((app, secs)) = row_result {
             println!("{:<40}: {}", app, format_duration_secs(secs));
             found_last_hour = true;
         }
     }
      if !found_last_hour { println!("No aggregated data found for the last completed hour."); }


    // --- Current Hour (Approximate: Aggregated + Raw) ---
    let current_hour_start_ts = now.date_naive().and_hms_opt(now.hour(), 0, 0).unwrap().and_utc().timestamp();
    println!("\n--- Current Hour Summary (approximate) ---");
    let mut hourly_totals: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    // 1. Get from hourly_summary (if aggregation ran mid-hour)
    let mut stmt_hour_sum = conn.prepare(
        "SELECT app_name, total_duration_secs
         FROM hourly_summary
         WHERE hour_timestamp = ?1"
    )?;
    let hour_sum_rows = stmt_hour_sum.query_map(params![current_hour_start_ts], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    for row_result in hour_sum_rows { if let Ok((app, secs)) = row_result { hourly_totals.insert(app, secs); } }

    // 2. Get raw intervals overlapping this hour
    let current_time_ts = now.timestamp();
    let mut stmt_hour_raw = conn.prepare( // Query adjusted slightly for clarity/correctness
        "SELECT app_name,
                SUM(
                    MIN(COALESCE(end_time, ?1), ?2) -- End time clamped to now and end of hour
                    -
                    MAX(start_time, ?3)             -- Start time clamped to start of hour
                ) as duration
         FROM app_intervals
         WHERE start_time < ?2           -- Interval started before end of current hour
           AND COALESCE(end_time, ?1) > ?3 -- Interval ended after start of current hour (or is still running)
         GROUP BY app_name
         HAVING duration > 0" // Exclude intervals completely outside the hour after clamping
    )?;
    // Parameters: 1: now, 2: end_of_current_hour, 3: start_of_current_hour
    let end_of_current_hour_ts = current_hour_start_ts + 3600; // Add 1 hour
    let hour_raw_rows = stmt_hour_raw.query_map(params![current_time_ts, end_of_current_hour_ts, current_hour_start_ts], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)) // Duration might be NULL if SUM is empty, handle this? Let's assume i64 for now.
    })?;

    // 3. Combine results
    for row_result in hour_raw_rows {
        if let Ok((app, secs)) = row_result {
            *hourly_totals.entry(app).or_insert(0) += secs;
        }
    }

    // Print combined hourly totals
    if hourly_totals.is_empty() {
         println!("No activity recorded yet for the current hour.");
    } else {
        let mut sorted_hourly: Vec<_> = hourly_totals.into_iter().collect();
        sorted_hourly.sort_by(|a, b| b.1.cmp(&a.1));
        for (app, secs) in sorted_hourly {
            println!("{:<40}: {}", app, format_duration_secs(secs));
        }
    }

    println!("---------------------------------------------");
    Ok(())
}

// Needs access to utils::format_duration_secs
// Ensure format_duration_secs is public in utils.rs or move it here
mod utils {
   use std::time::Duration;
   pub fn format_duration_secs(total_seconds: i64) -> String {
       if total_seconds < 0 { return "Invalid".to_string(); }
       let hours = total_seconds / 3600;
       let minutes = (total_seconds % 3600) / 60;
       let seconds = total_seconds % 60;
       format!("{:02}:{:02}:{:02}", hours, minutes, seconds);
   

    if total_seconds < 0 { return "Invalid".to_string(); }
    // let duration = Duration::from_secs(total_seconds as u64); // Unused
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
}
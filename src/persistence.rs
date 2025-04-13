// src/persistence.rs
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult}; // Removed Transaction (not directly needed after fix)
use std::path::{Path, PathBuf};
use std::time::Duration; // Corrected import
use chrono::{Utc, TimeZone, Datelike, Timelike}; // Removed DateTime, ChronoDuration

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


// Updated aggregate_and_cleanup to take &mut Connection
pub fn aggregate_and_cleanup(conn: &mut Connection) -> SqlResult<()> { // <-- Takes &mut Connection
    println!("Starting aggregation and cleanup...");
    let tx = conn.transaction()?;

    let now = Utc::now();
    let current_hour_start = now.date_naive().and_hms_opt(now.hour(), 0, 0).unwrap().and_utc().timestamp();

    // FIX: Explicitly handle NULL result from MAX() using Option<i64> in the closure
    let max_end_time_to_process: Option<i64> = tx.query_row(
        // Select the MAX, which might be NULL if no rows match
        "SELECT MAX(end_time) FROM app_intervals WHERE end_time < ?",
        params![current_hour_start],
        // The closure now maps the row to a Result<Option<i64>, rusqlite::Error>
        |row| row.get(0), // row.get attempts to convert SQL value (including NULL) to Option<i64>
    )?; // Use '?' to propagate potential rusqlite errors

    // Now max_end_time_to_process is correctly an Option<i64> representing the MAX value OR None if MAX was NULL.

    if let Some(aggregate_until) = max_end_time_to_process {
         // Ensure aggregate_until is actually before current_hour_start, although the WHERE clause should guarantee this.
         if aggregate_until >= current_hour_start {
             println!("Warning: MAX(end_time) {} is not less than current hour start {}. Skipping aggregation cycle.", aggregate_until, current_hour_start);
             // This case shouldn't really happen with the WHERE clause, but good to be safe.
             return Ok(());
         }

         println!("Aggregating intervals completed before: {}", Utc.timestamp_opt(aggregate_until, 0).unwrap());

        let hourly_rows = tx.execute(
            "INSERT INTO hourly_summary (app_name, hour_timestamp, total_duration_secs)
             SELECT
                 app_name,
                 CAST(strftime('%s', DATETIME(start_time, 'unixepoch', 'start of hour')) AS INTEGER) as hour_start,
                 SUM(end_time - start_time) as duration
             FROM app_intervals
             WHERE end_time IS NOT NULL AND end_time <= ?1
             GROUP BY app_name, hour_start
             ON CONFLICT(app_name, hour_timestamp) DO UPDATE SET
                 total_duration_secs = total_duration_secs + excluded.total_duration_secs",
            params![aggregate_until],
        )?;
        println!("-> Aggregated {} rows into hourly summary.", hourly_rows);

        let daily_rows = tx.execute(
            "INSERT INTO daily_summary (app_name, day_timestamp, total_duration_secs)
             SELECT
                 app_name,
                 CAST(strftime('%s', DATETIME(start_time, 'unixepoch', 'start of day')) AS INTEGER) as day_start,
                 SUM(end_time - start_time) as duration
             FROM app_intervals
             WHERE end_time IS NOT NULL AND end_time <= ?1
             GROUP BY app_name, day_start
             ON CONFLICT(app_name, day_timestamp) DO UPDATE SET
                 total_duration_secs = total_duration_secs + excluded.total_duration_secs",
            params![aggregate_until],
        )?;
         println!("-> Aggregated {} rows into daily summary.", daily_rows);

        let deleted_rows = tx.execute(
            "DELETE FROM app_intervals WHERE end_time IS NOT NULL AND end_time <= ?1",
            params![aggregate_until],
        )?;
        println!("-> Deleted {} processed raw interval rows.", deleted_rows);

    } else {
        // This case means MAX(end_time) was NULL -> no completed intervals before current_hour_start
        println!("No completed intervals found to aggregate.");
    }

    tx.commit()?;
    println!("Aggregation and cleanup finished.");
    Ok(())}
// Updated calculate_and_print_stats (placeholder)
pub fn calculate_and_print_stats(conn: &Connection) -> SqlResult<()> { // <-- Takes &Connection (only reads)
     println!("\n--- Stats Calculation (Using Summary Tables - Placeholder) ---");

     // FIX: Use .and_utc().timestamp()
     let today_start_ts = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
     println!("Today's Summary (from daily_summary):");
      let mut stmt_today = conn.prepare(
         "SELECT app_name, total_duration_secs
          FROM daily_summary
          WHERE day_timestamp = ?1
          ORDER BY total_duration_secs DESC"
     )?;
     let today_rows = stmt_today.query_map(params![today_start_ts], |row| {
         Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
     })?;

      for row_result in today_rows {
          if let Ok((app, secs)) = row_result {
              println!("{:<40}: {}", app, format_duration_secs(secs));
          }
      }

     // FIX: Use .and_utc().timestamp()
     let current_hour_start_ts = Utc::now().date_naive().and_hms_opt(Utc::now().hour(), 0, 0).unwrap().and_utc().timestamp();
     println!("\nCurrent Hour's Summary (from hourly_summary + current raw):");
      let mut hourly_totals: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
       let mut stmt_hour_sum = conn.prepare(
         "SELECT app_name, total_duration_secs
          FROM hourly_summary
          WHERE hour_timestamp = ?1"
     )?;
     let hour_sum_rows = stmt_hour_sum.query_map(params![current_hour_start_ts], |row| {
         Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
     })?;
      for row_result in hour_sum_rows {
          if let Ok((app, secs)) = row_result {
              hourly_totals.insert(app, secs);
          }
      }

      // FIX: Use .and_utc().timestamp()
      let current_time_ts = Utc::now().timestamp(); // Direct timestamp is fine here
       let mut stmt_hour_raw = conn.prepare(
         "SELECT app_name, SUM(COALESCE(end_time, ?1) - MAX(start_time, ?2)) as duration
          FROM app_intervals
          WHERE MAX(start_time, ?2) < COALESCE(end_time, ?1)
          AND (end_time IS NULL OR end_time >= ?2)
          GROUP BY app_name"
     )?;
      let hour_raw_rows = stmt_hour_raw.query_map(params![current_time_ts, current_hour_start_ts], |row| {
         Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
     })?;

      for row_result in hour_raw_rows {
          if let Ok((app, secs)) = row_result {
             *hourly_totals.entry(app).or_insert(0) += secs;
          }
      }

      let mut sorted_hourly: Vec<_> = hourly_totals.into_iter().collect();
      sorted_hourly.sort_by(|a, b| b.1.cmp(&a.1));
       for (app, secs) in sorted_hourly {
          println!("{:<40}: {}", app, format_duration_secs(secs));
      }

     println!("-----------------------------------------");
     Ok(())
 }


// format_duration_secs: Fix unused variable
fn format_duration_secs(total_seconds: i64) -> String {
    if total_seconds < 0 { return "Invalid".to_string(); }
    // let duration = Duration::from_secs(total_seconds as u64); // Unused
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
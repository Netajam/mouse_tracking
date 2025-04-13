// src/persistence.rs
use rusqlite::{params, Connection, Result as SqlResult};
use std::path::{Path, PathBuf};


// Function to get the database file path
pub fn get_data_file_path() -> Result<PathBuf, String> {
    match dirs::data_dir() {
        Some(mut path) => {
            path.push("RustAppTimeTracker"); // Subdirectory for our app data
            path.push("app_usage.sqlite"); // Database filename
            // Ensure parent directory exists
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

// Function to open or create the database connection
pub fn open_connection(path: &Path) -> SqlResult<Connection> {
    Connection::open(path)
    // Optional: configure connection, e.g., WAL mode for better concurrency if needed later
    // conn.pragma_update(None, "journal_mode", "WAL")?;
}

// Function to initialize the database schema
pub fn initialize_db(conn: &Connection) -> SqlResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS app_intervals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            app_name TEXT NOT NULL,
            start_time INTEGER NOT NULL, -- Unix timestamp (seconds)
            end_time INTEGER            -- Unix timestamp (seconds), NULLable
        )",
        [], // No parameters
    )?;
     // Optional indexes
     conn.execute("CREATE INDEX IF NOT EXISTS idx_app_name ON app_intervals (app_name);", [])?;
     conn.execute("CREATE INDEX IF NOT EXISTS idx_start_time ON app_intervals (start_time);", [])?;
    Ok(())
}

// Function called on startup to close any intervals left open from a crash
pub fn finalize_dangling_intervals(conn: &Connection, shutdown_time: i64) -> SqlResult<usize> {
     println!("Checking for dangling intervals from previous sessions...");
     // Set end_time to be the same as start_time for very old dangling intervals,
     // or to the current time for recent ones. This avoids infinitely growing intervals from crashes.
     // Intervals dangling for more than, say, 1 day are likely crashes.
     let one_day_ago = shutdown_time - (24 * 60 * 60);
     let updated_old = conn.execute(
         "UPDATE app_intervals SET end_time = start_time WHERE end_time IS NULL AND start_time < ?",
         params![one_day_ago],
     )?;
     let updated_recent = conn.execute(
         "UPDATE app_intervals SET end_time = ? WHERE end_time IS NULL AND start_time >= ?", // Finalize recent ones to now
         params![shutdown_time, one_day_ago],
     )?;
     let total_updated = updated_old + updated_recent;
     if total_updated > 0 {
        println!("Finalized {} dangling interval(s).", total_updated);
     }
     Ok(total_updated)
 }


// Function to insert a new interval when cursor enters an app
// Returns the rowid of the inserted row
pub fn insert_new_interval(conn: &Connection, app_name: &str, start_time: i64) -> SqlResult<i64> {
    conn.execute(
        "INSERT INTO app_intervals (app_name, start_time, end_time) VALUES (?1, ?2, NULL)",
        params![app_name, start_time],
    )?;
    Ok(conn.last_insert_rowid())
}

// Function to update the end_time of an interval when cursor leaves
pub fn finalize_interval(conn: &Connection, row_id: i64, end_time: i64) -> SqlResult<usize> {
    conn.execute(
        "UPDATE app_intervals SET end_time = ?1 WHERE id = ?2 AND end_time IS NULL", // Only update if not already finalized
        params![end_time, row_id],
    )
}

// --- Placeholder for stats function (to be implemented with `stats` command) ---
// This would query the database and calculate/format totals.
pub fn calculate_and_print_stats(conn: &Connection) -> SqlResult<()> {
     println!("\n--- Stats Calculation (Placeholder) ---");
     // Example Query: Get total time per app today
     let today_start = chrono::Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().timestamp();
     let mut stmt = conn.prepare(
         "SELECT app_name, SUM(COALESCE(end_time, CAST(strftime('%s', 'now') AS INTEGER)) - start_time) as total_seconds
          FROM app_intervals
          WHERE start_time >= ?1 OR (end_time IS NULL AND start_time < ?1) -- Include intervals starting today or currently running ones started before today
          GROUP BY app_name
          ORDER BY total_seconds DESC"
     )?;
     let rows = stmt.query_map(params![today_start], |row| {
         Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
     })?;

     println!("Total Time Per App (Today - approximate):");
     for row_result in rows {
         match row_result {
            Ok((app_name, total_seconds)) => {
                println!("{:<40}: {}", app_name, format_duration_secs(total_seconds));
            }
            Err(e) => eprintln!("Error processing row: {}", e),
         }
     }
     println!("-----------------------------------------");

     Ok(())
 }

 // Helper specifically for formatting seconds directly
 fn format_duration_secs(total_seconds: i64) -> String {
    if total_seconds < 0 { return "Invalid".to_string(); } // Handle potential edge cases from COALESCE
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
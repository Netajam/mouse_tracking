// src/persistence.rs
use crate::config; 
use rusqlite::{params, Connection, Result as SqlResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
// Removed std::time::Duration, unused here now
use chrono::{Utc, TimeZone, Timelike};

// --- Structs for returning stats data (public) ---

#[derive(Debug, Clone)]
pub struct AppUsageRecord {
    pub app_name: String,
    pub total_duration_secs: i64,
}

#[derive(Debug, Default, Clone)]
pub struct StatsData {
    pub today: Vec<AppUsageRecord>,
    pub last_hour: Vec<AppUsageRecord>,
    pub current_hour: Vec<AppUsageRecord>,
}

// --- File Path and Connection ---

pub fn get_data_file_path() -> Result<PathBuf, String> {
    match dirs::data_dir() {
        Some(mut path) => {
            let app_name = env!("CARGO_PKG_NAME");
            path.push(app_name);
            // Assuming config module exists or using hardcoded name for now
            // path.push(crate::config::DATABASE_FILENAME);
            path.push(config::DATABASE_FILENAME); 
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

pub fn open_connection(path: &Path) -> SqlResult<Connection> {
    Connection::open(path)
}

// --- Schema Initialization ---

pub fn initialize_db(conn: &mut Connection) -> SqlResult<()> {
    let tx = conn.transaction()?;

    // Create Tables
    tx.execute(include_str!("../sql/initialize_db_app_intervals.sql"), [])?;
    tx.execute(include_str!("../sql/initialize_db_hourly_summary.sql"), [])?;
    tx.execute(include_str!("../sql/initialize_db_daily_summary.sql"), [])?;

    // Create Indexes
    tx.execute("CREATE INDEX IF NOT EXISTS idx_app_intervals_app_name ON app_intervals (app_name);", [])?;
    tx.execute("CREATE INDEX IF NOT EXISTS idx_app_intervals_start_time ON app_intervals (start_time);", [])?;
    tx.execute("CREATE INDEX IF NOT EXISTS idx_app_intervals_end_time ON app_intervals (end_time);", [])?;

    tx.commit()
}

// --- Interval Management ---

pub fn finalize_dangling_intervals(conn: &Connection, shutdown_time: i64) -> SqlResult<usize> {
    println!("Checking for dangling intervals from previous sessions...");
    let threshold_secs = config::DANGLING_INTERVAL_RECENT_THRESHOLD_SECS;
    let cutoff_time = shutdown_time - threshold_secs;

    let updated_old = conn.execute(
        include_str!("../sql/finalize_dangling_old.sql"),
        params![cutoff_time],
    )?;
    let updated_recent = conn.execute(
        include_str!("../sql/finalize_dangling_recent.sql"),
        params![shutdown_time, cutoff_time],
    )?;
    let total_updated = updated_old + updated_recent;
    if total_updated > 0 {
        println!("Finalized {} dangling interval(s).", total_updated);
    }
    Ok(total_updated)
}

pub fn insert_new_interval(conn: &Connection, app_name: &str, window_title: &str,start_time: i64) -> SqlResult<i64> {
    conn.execute(
        include_str!("../sql/insert_interval.sql"),
        params![app_name,window_title, start_time],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn finalize_interval(conn: &Connection, row_id: i64, end_time: i64) -> SqlResult<usize> {
    conn.execute(
        include_str!("../sql/finalize_interval.sql"),
        params![end_time, row_id],
    )
}

// --- Aggregation and Cleanup ---

pub fn aggregate_and_cleanup(conn: &mut Connection) -> SqlResult<()> {
    println!("Starting aggregation and cleanup...");
    let tx = conn.transaction()?;

    let now = Utc::now();
    let current_hour_start = now.date_naive().and_hms_opt(now.hour(), 0, 0).unwrap().and_utc().timestamp();

    let max_end_time_to_process: Option<i64> = tx.query_row(
        include_str!("../sql/query_max_end_time.sql"),
        params![current_hour_start],
        |row| row.get(0),
    )?;

    if let Some(aggregate_until) = max_end_time_to_process {
         if aggregate_until >= current_hour_start {
             println!("Warning: MAX(end_time) {} is not less than current hour start {}. Skipping aggregation cycle.", aggregate_until, current_hour_start);
             return Ok(());
         }

         println!("Aggregating intervals completed before: {}", Utc.timestamp_opt(aggregate_until, 0).unwrap());

        let hourly_rows = tx.execute(
            include_str!("../sql/aggregate_hourly.sql"),
            params![aggregate_until],
        )?;
        println!("-> Aggregated {} rows into hourly summary.", hourly_rows);

        let daily_rows = tx.execute(
             include_str!("../sql/aggregate_daily.sql"),
             params![aggregate_until],
        )?;
         println!("-> Aggregated {} rows into daily summary.", daily_rows);

        let deleted_rows = tx.execute(
            include_str!("../sql/delete_aggregated.sql"),
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

// --- Statistics Querying ---

pub fn query_aggregated_stats(conn: &Connection) -> SqlResult<StatsData> {
    let mut stats_data = StatsData::default();
    let now = Utc::now();

    // --- Today's Stats (from daily_summary) ---
    let today_start_ts = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
    let mut stmt_today = conn.prepare(include_str!("../sql/query_stats_today.sql"))?;
    let today_iter = stmt_today.query_map(params![today_start_ts], |row| {
        // This closure returns Result<AppUsageRecord, Error>
        Ok(AppUsageRecord {
            app_name: row.get(0)?,
            total_duration_secs: row.get(1)?,
        })
    })?;
    // Collect handles the inner Results, or use a loop with explicit handling:
    // stats_data.today = today_iter.collect::<SqlResult<Vec<AppUsageRecord>>>()?;
    // Alternative loop approach:
    for record_result in today_iter {
        match record_result {
            Ok(record) => stats_data.today.push(record),
            Err(e) => eprintln!("Error processing today's stats row: {}", e), // Log error or handle differently
        }
    }


    // --- Last Completed Hour (from hourly_summary) ---
    let last_completed_hour_start_ts = (now - chrono::Duration::hours(1))
                                       .date_naive()
                                       .and_hms_opt( (now - chrono::Duration::hours(1)).hour(), 0, 0).unwrap()
                                       .and_utc().timestamp();
    let mut stmt_last_hour = conn.prepare(include_str!("../sql/query_stats_last_hour.sql"))?;
     let last_hour_iter = stmt_last_hour.query_map(params![last_completed_hour_start_ts], |row| {
        Ok(AppUsageRecord {
            app_name: row.get(0)?,
            total_duration_secs: row.get(1)?,
        })
     })?;
     // stats_data.last_hour = last_hour_iter.collect::<SqlResult<Vec<AppUsageRecord>>>()?;
     // Alternative loop approach:
     for record_result in last_hour_iter {
        match record_result {
            Ok(record) => stats_data.last_hour.push(record),
            Err(e) => eprintln!("Error processing last hour stats row: {}", e),
        }
    }


    // --- Current Hour (Approximate: Aggregated + Raw) ---
    let current_hour_start_ts = now.date_naive().and_hms_opt(now.hour(), 0, 0).unwrap().and_utc().timestamp();
    let mut current_hour_totals: HashMap<String, i64> = HashMap::new();

    // 1. Get from hourly_summary
    let mut stmt_hour_sum = conn.prepare(include_str!("../sql/query_stats_current_hour_agg.sql"))?;
    let hour_sum_iter = stmt_hour_sum.query_map(params![current_hour_start_ts], |row| {
        // Closure returns Result<(String, i64), Error>
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    for row_result in hour_sum_iter {
        // FIX: Handle Result from each iteration
        match row_result {
            Ok((app, secs)) => {
                *current_hour_totals.entry(app).or_insert(0) += secs;
            }
            Err(e) => {
                 eprintln!("Error processing current hour aggregated row: {}", e);
            }
        }
    }

    // 2. Get raw intervals overlapping this hour
    let current_time_ts = now.timestamp();
    let mut stmt_hour_raw = conn.prepare(include_str!("../sql/query_stats_current_hour_raw.sql"))?;
    let end_of_current_hour_ts = current_hour_start_ts + 3600;
    let hour_raw_iter = stmt_hour_raw.query_map(params![current_time_ts, end_of_current_hour_ts, current_hour_start_ts], |row| {
         // Closure returns Result<(String, i64), Error>
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?.unwrap_or(0)))
    })?;

    // 3. Combine results
    for row_result in hour_raw_iter {
         // FIX: Handle Result from each iteration
        match row_result {
            Ok((app, secs)) => {
                *current_hour_totals.entry(app).or_insert(0) += secs;
            }
             Err(e) => {
                 eprintln!("Error processing current hour raw row: {}", e);
            }
        }
    }

    // 4. Convert HashMap to Vec<AppUsageRecord> and sort
    let mut sorted_hourly: Vec<AppUsageRecord> = current_hour_totals
        .into_iter()
        .map(|(app, secs)| AppUsageRecord { app_name: app, total_duration_secs: secs })
        .collect();
    sorted_hourly.sort_by(|a, b| b.total_duration_secs.cmp(&a.total_duration_secs));
    stats_data.current_hour = sorted_hourly;

    Ok(stats_data)
}

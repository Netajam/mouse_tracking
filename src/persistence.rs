// src/persistence.rs
use crate::config;
use rusqlite::{params, Connection, Result as SqlResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use chrono::{Utc, TimeZone, Datelike, Timelike}; // Added Datelike back
use log::{debug, info, warn, error}; // Use log macros

// --- Structs ---
// This now represents a detailed record from summary tables or combined current hour
#[derive(Debug, Clone)]
pub struct DetailedUsageRecord {
    pub app_name: String,
    pub detailed_title: String, // Changed from AppUsageRecord
    pub total_duration_secs: i64,
}

// StatsData now holds Vecs of the detailed records
#[derive(Debug, Default, Clone)]
pub struct StatsData {
    pub today: Vec<DetailedUsageRecord>,
    pub last_hour: Vec<DetailedUsageRecord>,
    pub current_hour: Vec<DetailedUsageRecord>,
    // Add later: pub historical_by_app: Vec<AppUsageRecord> (from days_summary_by_app)
}

pub fn open_connection_ensure_path(path: &Path) -> SqlResult<Connection> {
    if let Some(parent_dir) = path.parent() {
        if !parent_dir.exists() {
            info!("Data directory not found. Creating: {:?}", parent_dir);
            // Use map_err to provide slightly more context if needed, though From works
            fs::create_dir_all(parent_dir).map_err(|io_err|
               rusqlite::Error::FromSqlConversionFailure(
                   0, // Box<dyn Error> doesn't fit well here, use a specific code?
                   rusqlite::types::Type::Null,
                   Box::new(io_err) // Box the io::Error
            ))?; // Propagate as SqlResult::Err
            info!("Successfully created data directory.");
        } else {
             debug!("Data directory already exists: {:?}", parent_dir);
        }
    } else {
         warn!("Could not determine parent directory for database path: {:?}", path);
    }
    debug!("Opening database connection at: {:?}", path);
    Connection::open(path) // Creates file if not exists
}




// --- Schema Initialization ---
pub fn initialize_db(conn: &mut Connection) -> SqlResult<()> {
    info!("Initializing database schema if needed...");
    let tx = conn.transaction()?;
    tx.execute(include_str!("../sql/initialize_db_app_intervals.sql"), [])?;
    tx.execute(include_str!("../sql/initialize_db_hourly_summary.sql"), [])?;
    tx.execute(include_str!("../sql/initialize_db_daily_summary.sql"), [])?;
    tx.execute(include_str!("../sql/initialize_db_days_summary_by_app.sql"), [])?; // Create new table

    // Indexes (Add indexes for new columns/tables if desired)
    tx.execute("CREATE INDEX IF NOT EXISTS idx_app_intervals_app_name ON app_intervals (app_name);", [])?;
    tx.execute("CREATE INDEX IF NOT EXISTS idx_app_intervals_main_title ON app_intervals (main_window_title);", [])?; // Optional
    tx.execute("CREATE INDEX IF NOT EXISTS idx_app_intervals_detailed_title ON app_intervals (detailed_window_title);", [])?; // Optional
    tx.execute("CREATE INDEX IF NOT EXISTS idx_app_intervals_start_time ON app_intervals (start_time);", [])?;
    tx.execute("CREATE INDEX IF NOT EXISTS idx_app_intervals_end_time ON app_intervals (end_time);", [])?;
    // Indexes for summary tables (covered by PKs, but explicit indexes might help sometimes)
    // tx.execute("CREATE INDEX IF NOT EXISTS idx_hourly_summary_app ON hourly_summary (app_name);", [])?;
    // tx.execute("CREATE INDEX IF NOT EXISTS idx_daily_summary_app ON daily_summary (app_name);", [])?;
    // tx.execute("CREATE INDEX IF NOT EXISTS idx_days_summary_by_app_app ON days_summary_by_app (app_name);", [])?;

    tx.commit()
}

// --- Interval Management ---

pub fn insert_new_interval( conn: &Connection, app_name: &str, main_title: &str, detailed_title: &str, start_time: i64) -> SqlResult<i64> {
    // Ensure this matches the SQL file with 4 placeholders
    conn.execute(
        include_str!("../sql/insert_interval.sql"),
        params![app_name, main_title, detailed_title, start_time],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn finalize_interval(conn: &Connection, row_id: i64, end_time: i64) -> SqlResult<usize> {
    conn.execute(
        include_str!("../sql/finalize_interval.sql"),
        params![end_time, row_id],
    )
}
pub fn finalize_dangling_intervals(
    conn: &Connection,
    shutdown_time: i64, // Typically the timestamp when the app starts *this* time
    threshold_secs: i64 // How far back to consider "recent" (passed from config)
) -> SqlResult<usize> {
    info!("Checking for dangling intervals from previous sessions (threshold: {} seconds)...", threshold_secs);
    // Calculate the cutoff time based on the passed threshold
    let cutoff_time = shutdown_time - threshold_secs;
    debug!("Dangling interval cutoff time (before this = old): {}", cutoff_time);

    // Finalize "old" dangling intervals (likely crashes): Set end_time = start_time
    let updated_old = conn.execute(
        include_str!("../sql/finalize_dangling_old.sql"),
        params![cutoff_time], // SQL expects cutoff_time as ?1
    )?;
    if updated_old > 0 {
        debug!("-> Finalized {} old dangling interval(s) by setting end_time = start_time.", updated_old);
    }

    // Finalize "recent" dangling intervals (likely unclean shutdown): Set end_time = current shutdown_time
    let updated_recent = conn.execute(
        include_str!("../sql/finalize_dangling_recent.sql"),
        // SQL expects shutdown_time as ?1, cutoff_time as ?2
        params![shutdown_time, cutoff_time],
    )?;
     if updated_recent > 0 {
        debug!("-> Finalized {} recent dangling interval(s) by setting end_time = now.", updated_recent);
    }

    let total_updated = updated_old + updated_recent;
    if total_updated > 0 {
        info!("Finalized a total of {} dangling interval(s).", total_updated);
    } else {
         debug!("No dangling intervals found to finalize.");
    }
    Ok(total_updated)
}

// --- Aggregation and Cleanup (Updated) ---
pub fn aggregate_and_cleanup(conn: &mut Connection) -> SqlResult<()> {
    info!("Starting aggregation and cleanup...");
    let tx = conn.transaction()?;
    let now = Utc::now();

    // --- Aggregate Raw Intervals ---
    let current_hour_start = now.date_naive().and_hms_opt(now.hour(), 0, 0).unwrap().and_utc().timestamp();
    let max_end_time_to_process: Option<i64> = tx.query_row( include_str!("../sql/query_max_end_time.sql"), params![current_hour_start], |row| row.get(0), )?;

    if let Some(aggregate_until) = max_end_time_to_process {
         if aggregate_until >= current_hour_start {
             // Silently skip if no full past hours to aggregate
             return Ok(());
         }
         debug!("Aggregating raw intervals completed before: {}", Utc.timestamp_opt(aggregate_until, 0).unwrap());

        let hourly_rows = tx.execute( include_str!("../sql/aggregate_hourly.sql"), params![aggregate_until], )?;
        if hourly_rows > 0 { debug!("-> Aggregated {} rows into hourly summary.", hourly_rows); }

        let daily_rows = tx.execute( include_str!("../sql/aggregate_daily.sql"), params![aggregate_until], )?;
        if daily_rows > 0 { debug!("-> Aggregated {} rows into daily summary.", daily_rows); }

        let deleted_raw = tx.execute( include_str!("../sql/delete_aggregated.sql"), params![aggregate_until], )?;
        if deleted_raw > 0 { debug!("-> Deleted {} processed raw interval rows.", deleted_raw); }
    } else {
        debug!("No completed raw intervals found to aggregate.");
    }

    // --- Aggregate Detailed Summaries & Cleanup Old ---
    // Define cutoff for deletion (e.g., start of yesterday)
    let cutoff_day_ts = (now.date_naive() - chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
    debug!("Aggregating detailed summaries older than timestamp: {}", cutoff_day_ts);

    // Aggregate daily detailed data into daily app summary
    let aggregated_days = tx.execute( include_str!("../sql/aggregate_days_summary.sql"), params![cutoff_day_ts], )?;
    if aggregated_days > 0 { debug!("-> Aggregated older daily data into days_summary_by_app."); }

    // Delete old detailed daily summaries
    let deleted_daily = tx.execute( include_str!("../sql/delete_aggregated_daily.sql"), params![cutoff_day_ts], )?;
    if deleted_daily > 0 { debug!("-> Deleted {} old daily summary rows.", deleted_daily); }

    // Delete old detailed hourly summaries
    let deleted_hourly = tx.execute( include_str!("../sql/delete_aggregated_hourly.sql"), params![cutoff_day_ts], )?;
    if deleted_hourly > 0 { debug!("-> Deleted {} old hourly summary rows.", deleted_hourly); }

    tx.commit()?;
    info!("Aggregation and cleanup finished.");
    Ok(())
}

// --- Statistics Querying (Updated) ---
pub fn query_aggregated_stats(conn: &Connection) -> SqlResult<StatsData> {
    // Ensure summary tables exist before querying them
    conn.execute(include_str!("../sql/initialize_db_hourly_summary.sql"), [])?;
    conn.execute(include_str!("../sql/initialize_db_daily_summary.sql"), [])?;
    // No need to create days_summary_by_app here, as it's only populated by aggregation

    let mut stats_data = StatsData::default();
    let now = Utc::now();

    // --- Today's Detailed Stats (from daily_summary) ---
    let today_start_ts = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
    let mut stmt_today = conn.prepare(include_str!("../sql/query_stats_today.sql"))?; // SQL now selects detailed_title
    let today_iter = stmt_today.query_map(params![today_start_ts], |row| {
        Ok(DetailedUsageRecord { // Use new struct
            app_name: row.get(0)?,
            detailed_title: row.get(1)?, // Get detailed title
            total_duration_secs: row.get(2)?, // Index is now 2
        })
    })?;
    for record_result in today_iter {
        match record_result {
            Ok(record) => stats_data.today.push(record),
            Err(e) => warn!("Error processing today's stats row: {}", e), // Use warn
        }
    }

    // --- Last Completed Hour Detailed Stats (from hourly_summary) ---
    let last_completed_hour_start_ts = (now - chrono::Duration::hours(1))
                                       .date_naive()
                                       .and_hms_opt( (now - chrono::Duration::hours(1)).hour(), 0, 0).unwrap()
                                       .and_utc().timestamp();
    let mut stmt_last_hour = conn.prepare(include_str!("../sql/query_stats_last_hour.sql"))?; // SQL now selects detailed_title
     let last_hour_iter = stmt_last_hour.query_map(params![last_completed_hour_start_ts], |row| {
        Ok(DetailedUsageRecord { // Use new struct
            app_name: row.get(0)?,
            detailed_title: row.get(1)?, // Get detailed title
            total_duration_secs: row.get(2)?, // Index is now 2
        })
     })?;
     for record_result in last_hour_iter {
        match record_result {
            Ok(record) => stats_data.last_hour.push(record),
            Err(e) => warn!("Error processing last hour stats row: {}", e), // Use warn
        }
    }

    // --- Current Hour (Approximate: Aggregated Detailed + Raw Detailed) ---
    let current_hour_start_ts = now.date_naive().and_hms_opt(now.hour(), 0, 0).unwrap().and_utc().timestamp();
    // Temp map now keys by (AppName, DetailedTitle)
    let mut current_hour_totals: HashMap<(String, String), i64> = HashMap::new();

    // 1. Get from hourly_summary (detailed)
    let mut stmt_hour_sum = conn.prepare(include_str!("../sql/query_stats_current_hour_agg.sql"))?; // SQL now selects detailed_title
    let hour_sum_iter = stmt_hour_sum.query_map(params![current_hour_start_ts], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?)) // Get 3 columns
    })?;
    for row_result in hour_sum_iter {
        match row_result {
            Ok((app, title, secs)) => {
                *current_hour_totals.entry((app, title)).or_insert(0) += secs;
            }
            Err(e) => { warn!("Error processing current hour aggregated row: {}", e); }
        }
    }

    // 2. Get raw intervals overlapping this hour (detailed)
    let current_time_ts = now.timestamp();
    let mut stmt_hour_raw = conn.prepare(include_str!("../sql/query_stats_current_hour_raw.sql"))?; // SQL now selects detailed_title
    let end_of_current_hour_ts = current_hour_start_ts + 3600;
    let hour_raw_iter = stmt_hour_raw.query_map(params![current_time_ts, end_of_current_hour_ts, current_hour_start_ts], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<i64>>(2)?.unwrap_or(0))) // Get 3 columns
    })?;

    // 3. Combine results
    for row_result in hour_raw_iter {
        match row_result {
            Ok((app, title, secs)) => {
                *current_hour_totals.entry((app, title)).or_insert(0) += secs;
            }
            Err(e) => { warn!("Error processing current hour raw row: {}", e); }
        }
    }

    // 4. Convert HashMap to Vec<DetailedUsageRecord> and sort
    let mut sorted_hourly: Vec<DetailedUsageRecord> = current_hour_totals
        .into_iter()
        .map(|((app, title), secs)| DetailedUsageRecord { app_name: app, detailed_title: title, total_duration_secs: secs })
        .collect();
    sorted_hourly.sort_by(|a, b| b.total_duration_secs.cmp(&a.total_duration_secs)); // Sort descending
    stats_data.current_hour = sorted_hourly;


    // NOTE: Querying days_summary_by_app for historical data is left out for now,
    // could be added as another field to StatsData or a separate function.

    Ok(stats_data)
}
// src/persistence.rs

// Keep necessary use statements
use crate::types::{AggregationLevel, AggregatedResult, DetailedUsageRecord, TimePeriod};
use rusqlite::{params, Connection, Result as SqlResult}; // Keep only needed rusqlite items
use std::collections::HashMap;
use std::path::Path; // Keep Path
use std::fs;
use chrono::{Utc, TimeZone, Timelike, Duration}; // Keep needed chrono items
use log::{debug, info, warn}; // Keep needed log items

// --- Connection & Initialization ---
pub fn open_connection_ensure_path(path: &Path) -> SqlResult<Connection> {
    if let Some(parent_dir) = path.parent() {
        if !parent_dir.exists() {
            info!("Data directory not found. Creating: {:?}", parent_dir);
            fs::create_dir_all(parent_dir).map_err(|io_err| {
                // Provide slightly better context than direct unwrap/panic
                rusqlite::Error::FromSqlConversionFailure(
                    0, // Consider a custom error code or using a dedicated error type
                    rusqlite::types::Type::Null,
                    Box::new(io_err),
                )
            })?;
            info!("Successfully created data directory.");
        } else {
            debug!("Data directory already exists: {:?}", parent_dir);
        }
    } else {
        warn!(
            "Could not determine parent directory for database path: {:?}",
            path
        );
    }
    debug!("Opening database connection at: {:?}", path);
    Connection::open(path) // Creates file if not exists
}

pub fn initialize_db(conn: &mut Connection) -> SqlResult<()> {
    info!("Initializing database schema if needed...");
    let tx = conn.transaction()?;
    // Assumes sql/ is in the project root, one level up from src/
    tx.execute(include_str!("../sql/initialize_db_app_intervals.sql"), [])?;
    tx.execute(include_str!("../sql/initialize_db_hourly_summary.sql"), [])?;
    tx.execute(include_str!("../sql/initialize_db_daily_summary.sql"), [])?;
    tx.execute(include_str!("../sql/initialize_db_days_summary_by_app.sql"), [])?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_app_intervals_app_name ON app_intervals (app_name);",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_app_intervals_main_title ON app_intervals (main_window_title);",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_app_intervals_detailed_title ON app_intervals (detailed_window_title);",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_app_intervals_start_time ON app_intervals (start_time);",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_app_intervals_end_time ON app_intervals (end_time);",
        [],
    )?;
    tx.commit()
}

// --- Interval Management ---
pub fn insert_new_interval(
    conn: &Connection,
    app_name: &str,
    main_title: &str,
    detailed_title: &str,
    start_time: i64,
) -> SqlResult<i64> {
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
    shutdown_time: i64,
    threshold_secs: i64,
) -> SqlResult<usize> {
    info!(
        "Checking for dangling intervals from previous sessions (threshold: {} seconds)...",
        threshold_secs
    );
    let cutoff_time = shutdown_time - threshold_secs;
    debug!(
        "Dangling interval cutoff time (before this = old): {}",
        cutoff_time
    );
    let updated_old = conn.execute(
        include_str!("../sql/finalize_dangling_old.sql"),
        params![cutoff_time],
    )?;
    if updated_old > 0 {
        debug!(
            "-> Finalized {} old dangling interval(s) by setting end_time = start_time.",
            updated_old
        );
    }
    let updated_recent = conn.execute(
        include_str!("../sql/finalize_dangling_recent.sql"),
        params![shutdown_time, cutoff_time],
    )?;
    if updated_recent > 0 {
        debug!(
            "-> Finalized {} recent dangling interval(s) by setting end_time = now.",
            updated_recent
        );
    }
    let total_updated = updated_old + updated_recent;
    if total_updated > 0 {
        info!(
            "Finalized a total of {} dangling interval(s).",
            total_updated
        );
    } else {
        debug!("No dangling intervals found to finalize.");
    }
    Ok(total_updated)
}

// --- Aggregation and Cleanup ---
pub fn aggregate_and_cleanup(conn: &mut Connection) -> SqlResult<()> {
    info!("Starting aggregation and cleanup...");
    let tx = conn.transaction()?;
    let now = Utc::now();
    let current_hour_start = now
        .date_naive()
        .and_hms_opt(now.hour(), 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();
    let max_end_time_to_process: Option<i64> = tx.query_row(
        include_str!("../sql/query_max_end_time.sql"),
        params![current_hour_start],
        |row| row.get(0),
    )?;

    if let Some(aggregate_until) = max_end_time_to_process {
        if aggregate_until < current_hour_start {
            debug!(
                "Aggregating raw intervals completed before: {}",
                Utc.timestamp_opt(aggregate_until, 0).unwrap() // Consider handling error
            );
            let hourly_rows = tx.execute(
                include_str!("../sql/aggregate_hourly.sql"),
                params![aggregate_until],
            )?;
            if hourly_rows > 0 {
                debug!("-> Aggregated {} rows into hourly summary.", hourly_rows);
            }
            let daily_rows = tx.execute(
                include_str!("../sql/aggregate_daily.sql"),
                params![aggregate_until],
            )?;
            if daily_rows > 0 {
                debug!("-> Aggregated {} rows into daily summary.", daily_rows);
            }
            let deleted_raw = tx.execute(
                include_str!("../sql/delete_aggregated.sql"),
                params![aggregate_until],
            )?;
            if deleted_raw > 0 {
                debug!("-> Deleted {} processed raw interval rows.", deleted_raw);
            }
        } else {
            debug!("No full hours completed since last aggregation to process.");
        }
    } else {
        debug!("No completed raw intervals found to aggregate.");
    }

    let cutoff_day_ts = (now.date_naive() - Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();
    debug!(
        "Aggregating detailed summaries older than timestamp: {}",
        cutoff_day_ts
    );
    let aggregated_days = tx.execute(
        include_str!("../sql/aggregate_days_summary.sql"),
        params![cutoff_day_ts],
    )?;
    if aggregated_days > 0 {
        debug!("-> Aggregated older daily data into days_summary_by_app.");
    }
    let deleted_daily = tx.execute(
        include_str!("../sql/delete_aggregated_daily.sql"),
        params![cutoff_day_ts],
    )?;
    if deleted_daily > 0 {
        debug!("-> Deleted {} old daily summary rows.", deleted_daily);
    }
    let deleted_hourly = tx.execute(
        include_str!("../sql/delete_aggregated_hourly.sql"),
        params![cutoff_day_ts],
    )?;
    if deleted_hourly > 0 {
        debug!("-> Deleted {} old hourly summary rows.", deleted_hourly);
    }
    tx.commit()?;
    info!("Aggregation and cleanup finished.");
    Ok(())
}

// --- Statistics Querying ---

/// Helper to calculate start (inclusive) and end (exclusive) timestamps for a period
fn calculate_timestamps(period: TimePeriod) -> (i64, i64) {
    let now_dt = Utc::now();
    let today_start = now_dt.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();

    match period {
        TimePeriod::Today => {
            let start = today_start.timestamp();
            let end = (today_start + Duration::days(1)).timestamp();
            (start, end)
        }
        TimePeriod::LastCompletedHour => {
            let current_hour_start = now_dt
                .date_naive()
                .and_hms_opt(now_dt.hour(), 0, 0)
                .unwrap()
                .and_utc();
            let end = current_hour_start.timestamp();
            let start = (current_hour_start - Duration::hours(1)).timestamp();
            (start, end)
        }
        TimePeriod::CurrentHour => {
            let start = now_dt
                .date_naive()
                .and_hms_opt(now_dt.hour(), 0, 0)
                .unwrap()
                .and_utc()
                .timestamp();
            let end = (now_dt + Duration::seconds(1)).timestamp();
            (start, end)
        }
    }
}
pub fn query_stats(
conn: &Connection,
period: TimePeriod,
level: AggregationLevel,
) -> SqlResult<AggregatedResult> {
let (period_start_ts, period_end_ts) = calculate_timestamps(period);
let now_ts = Utc::now().timestamp(); // Needed for active intervals

// Use period_end_ts unless it's in the future (can happen for 'Today' end calc)
// We want the effective 'now' for COALESCE, but the period boundary for MIN.
let effective_end_ts = now_ts.min(period_end_ts);


debug!(
    "Querying stats for period: {:?}, level: {:?}, period_start: {}, period_end: {}, now: {}",
    period, level, period_start_ts, period_end_ts, now_ts
);

match level {
    AggregationLevel::ByApplication => {
        let mut app_totals: HashMap<String, i64> = HashMap::new();

        // --- Query days_summary_by_app (if relevant for the period) ---
        // (Keep the existing query for days_summary_by_app here)
        // Example structure:
        let mut stmt_days = conn.prepare(
            "SELECT app_name, SUM(total_duration_secs)
             FROM days_summary_by_app WHERE day_timestamp >= ?1 AND day_timestamp < ?2 GROUP BY app_name",
        )?;
        let iter_days = stmt_days.query_map(params![period_start_ts, period_end_ts], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for result in iter_days {
            if let Ok((app, secs)) = result {
                *app_totals.entry(app).or_insert(0) += secs;
            } else { warn!("Error processing days_summary row: {:?}", result.err()); }
        }
        // TODO: Add queries for daily_summary and hourly_summary if needed for this level


        // --- Query app_intervals (raw, unaggregated) ---
        // *** Use the new SQL file and corrected logic ***
        let mut stmt_intervals = conn.prepare(include_str!("../sql/query_stats_intervals_by_app.sql"))?;
        let iter_intervals = stmt_intervals.query_map(
            params![period_start_ts, effective_end_ts, now_ts], // Use effective_end_ts for MIN, now_ts for COALESCE
            |row| {
                let app: String = row.get(0)?;
                let secs: i64 = row.get(1).unwrap_or(0); // SUM might be NULL if no rows
                Ok((app, secs))
         })?;
         for result in iter_intervals {
             match result {
                 Ok((app, secs)) => *app_totals.entry(app).or_insert(0) += secs,
                 Err(e) => warn!("Error processing app_intervals row (by app): {}", e),
             }
         }

        let results: Vec<(String, i64)> = app_totals.into_iter().collect();
        Ok(AggregatedResult::ByApp(results))
    }

    AggregationLevel::Detailed => {
        let mut detailed_totals: HashMap<(String, String), i64> = HashMap::new();

        // --- Query daily_summary (if relevant) ---
        // (Keep the existing query for daily_summary here)
        // Example structure:
         let mut stmt_daily = conn.prepare(
            "SELECT app_name, detailed_window_title, SUM(total_duration_secs)
             FROM daily_summary WHERE day_timestamp >= ?1 AND day_timestamp < ?2 GROUP BY app_name, detailed_window_title",
         )?;
         let iter_daily = stmt_daily.query_map(params![period_start_ts, period_end_ts], |row| {
              Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
         })?;
         for result in iter_daily {
             if let Ok((app, title, secs)) = result {
                *detailed_totals.entry((app, title)).or_insert(0) += secs;
             } else { warn!("Error processing daily_summary row: {:?}", result.err()); }
         }
         // TODO: Add query for hourly_summary if needed for this level


        // --- Query app_intervals (detailed, raw, unaggregated) ---
        // *** Use the new SQL file and corrected logic ***
        let mut stmt_intervals_det = conn.prepare(include_str!("../sql/query_stats_intervals_detailed.sql"))?;
        let iter_intervals_det = stmt_intervals_det.query_map(
            params![period_start_ts, effective_end_ts, now_ts], // Use effective_end_ts for MIN, now_ts for COALESCE
            |row| {
                let app: String = row.get(0)?;
                let title: String = row.get(1)?;
                let secs: i64 = row.get(2).unwrap_or(0); // SUM might be NULL if no rows
                Ok((app, title, secs))
        })?;
        for result in iter_intervals_det {
            match result {
                Ok((app, title, secs)) => *detailed_totals.entry((app, title)).or_insert(0) += secs,
                Err(e) => warn!("Error processing detailed app_intervals row: {}", e),
            }
        }

        let results: Vec<DetailedUsageRecord> = detailed_totals
            .into_iter()
            .map(|((app, title), secs)| DetailedUsageRecord {
                app_name: app,
                detailed_title: title,
                total_duration_secs: secs,
            })
            .collect();
        Ok(AggregatedResult::Detailed(results))
    }
}
}
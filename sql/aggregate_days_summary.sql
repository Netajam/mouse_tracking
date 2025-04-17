INSERT INTO days_summary_by_app (app_name, day_timestamp, total_duration_secs)
SELECT
    app_name,
    day_timestamp,
    SUM(total_duration_secs) as total_for_day
FROM daily_summary -- Aggregate FROM the detailed daily summary
WHERE day_timestamp < ?1 -- aggregate_cutoff_day_ts (e.g., start of yesterday)
GROUP BY app_name, day_timestamp
ON CONFLICT(app_name, day_timestamp) DO UPDATE SET
    total_duration_secs = total_duration_secs + excluded.total_duration_secs;
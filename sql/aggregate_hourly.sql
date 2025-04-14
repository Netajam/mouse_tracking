INSERT INTO hourly_summary (app_name, hour_timestamp, total_duration_secs)
SELECT
    app_name,
    CAST(strftime('%s', DATETIME(start_time, 'unixepoch', 'start of hour')) AS INTEGER) as hour_start,
    SUM(end_time - start_time) as duration
FROM app_intervals
WHERE end_time IS NOT NULL AND end_time <= ?1
  AND hour_start IS NOT NULL
GROUP BY app_name, hour_start
ON CONFLICT(app_name, hour_timestamp) DO UPDATE SET
    total_duration_secs = total_duration_secs + excluded.total_duration_secs
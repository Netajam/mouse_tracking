INSERT INTO daily_summary (app_name, day_timestamp, total_duration_secs)
SELECT
    app_name,
    CAST(strftime('%s', DATETIME(start_time, 'unixepoch', 'start of day')) AS INTEGER) as day_start,
    SUM(end_time - start_time) as duration
FROM app_intervals
WHERE end_time IS NOT NULL AND end_time <= ?1
  AND day_start IS NOT NULL
GROUP BY app_name, day_start
ON CONFLICT(app_name, day_timestamp) DO UPDATE SET
    total_duration_secs = total_duration_secs + excluded.total_duration_secs
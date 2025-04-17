INSERT INTO daily_summary (app_name, detailed_window_title, day_timestamp, total_duration_secs)
SELECT
    app_name,
    COALESCE(detailed_window_title, '[No Detailed Title]') as detailed_title, -- Handle potential NULLs
    CAST(strftime('%s', DATETIME(start_time, 'unixepoch', 'start of day')) AS INTEGER) as day_start,
    SUM(MAX(0, end_time - start_time)) as duration -- Use MAX(0,...)
FROM app_intervals
WHERE end_time IS NOT NULL AND end_time <= ?1 -- aggregate_until timestamp
  AND day_start IS NOT NULL
GROUP BY app_name, detailed_title, day_start -- Updated GROUP BY
ON CONFLICT(app_name, detailed_window_title, day_timestamp) DO UPDATE SET -- Updated ON CONFLICT
    total_duration_secs = total_duration_secs + excluded.total_duration_secs;
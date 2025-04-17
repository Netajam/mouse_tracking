INSERT INTO hourly_summary (app_name, detailed_window_title, hour_timestamp, total_duration_secs)
SELECT
    app_name,
    COALESCE(detailed_window_title, '[No Detailed Title]') as detailed_title, -- Handle potential NULLs
    CAST(strftime('%s', DATETIME(start_time, 'unixepoch', 'start of hour')) AS INTEGER) as hour_start,
    SUM(MAX(0, end_time - start_time)) as duration -- Use MAX(0,...) to avoid negative duration if clocks change
FROM app_intervals
WHERE end_time IS NOT NULL AND end_time <= ?1 -- aggregate_until timestamp
  AND hour_start IS NOT NULL
GROUP BY app_name, detailed_title, hour_start -- Updated GROUP BY
ON CONFLICT(app_name, detailed_window_title, hour_timestamp) DO UPDATE SET -- Updated ON CONFLICT
    total_duration_secs = total_duration_secs + excluded.total_duration_secs;
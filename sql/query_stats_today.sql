SELECT app_name, detailed_window_title, total_duration_secs
FROM daily_summary
WHERE day_timestamp = ?1
ORDER BY app_name, total_duration_secs DESC;
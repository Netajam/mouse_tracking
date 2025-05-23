SELECT app_name, detailed_window_title, total_duration_secs
FROM hourly_summary
WHERE hour_timestamp = ?1
ORDER BY app_name, total_duration_secs DESC;
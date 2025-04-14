SELECT app_name, total_duration_secs
FROM hourly_summary
WHERE hour_timestamp = ?1
ORDER BY total_duration_secs DESC
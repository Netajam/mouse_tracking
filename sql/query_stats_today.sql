SELECT app_name, total_duration_secs
FROM daily_summary
WHERE day_timestamp = ?1
ORDER BY total_duration_secs DESC
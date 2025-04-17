DELETE FROM daily_summary
WHERE day_timestamp < ?1; -- Use aggregate_cutoff_day_ts
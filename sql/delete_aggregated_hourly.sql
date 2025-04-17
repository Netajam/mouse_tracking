DELETE FROM hourly_summary
WHERE hour_timestamp < ?1; -- Use aggregate_cutoff_day_ts
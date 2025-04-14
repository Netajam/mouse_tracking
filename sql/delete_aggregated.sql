DELETE FROM app_intervals
WHERE end_time IS NOT NULL AND end_time <= ?1
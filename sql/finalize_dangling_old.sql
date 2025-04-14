UPDATE app_intervals
SET end_time = start_time
WHERE end_time IS NULL AND start_time < ?1
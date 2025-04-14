UPDATE app_intervals
SET end_time = ?1
WHERE end_time IS NULL AND start_time >= ?2
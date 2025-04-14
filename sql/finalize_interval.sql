UPDATE app_intervals
SET end_time = ?1
WHERE id = ?2 AND end_time IS NULL
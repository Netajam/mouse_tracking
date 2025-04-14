SELECT MAX(end_time)
FROM app_intervals
WHERE end_time < ?1
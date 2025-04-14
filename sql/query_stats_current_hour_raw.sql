SELECT app_name,
       SUM(
           MIN(COALESCE(end_time, ?1), ?2) - MAX(start_time, ?3)
       ) as duration
FROM app_intervals
WHERE start_time < ?2           -- Interval started before end of current hour
  AND COALESCE(end_time, ?1) > ?3 -- Interval ended after start of current hour (or is still running)
GROUP BY app_name
HAVING duration > 0
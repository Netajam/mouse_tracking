SELECT app_name,
       COALESCE(detailed_window_title, '[No Detailed Title]') as detailed_title,
       SUM(
           MAX(0, MIN(COALESCE(end_time, ?1), ?2) - MAX(start_time, ?3))
       ) as duration
FROM app_intervals
WHERE start_time < ?2           -- Interval started before end of current hour
  AND COALESCE(end_time, ?1) > ?3 -- Interval ended after start of current hour (or is still running)
GROUP BY app_name, detailed_title -- Group by detailed title too
HAVING duration > 0;
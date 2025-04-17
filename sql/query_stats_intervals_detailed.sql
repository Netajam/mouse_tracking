-- Fetches detailed raw interval data for a given time period.
-- Used for stats when summary tables don't cover the period (e.g., current hour/day).
-- Params: ?1 = period_start_ts, ?2 = period_end_ts, ?3 = now_ts (for active intervals)
SELECT
    app_name,
    detailed_window_title,
    -- Calculate duration within the period [?1, ?2]
    -- COALESCE(end_time, ?3) uses 'now' as the end time for currently active intervals
    SUM(MAX(0, MIN(COALESCE(end_time, ?3), ?2) - MAX(start_time, ?1))) as duration_in_period
FROM
    app_intervals
WHERE
    -- Interval must start before the period ends
    start_time < ?2
    -- Interval must end (or be currently active) after the period starts
    AND COALESCE(end_time, ?3) > ?1
GROUP BY
    app_name,
    detailed_window_title;
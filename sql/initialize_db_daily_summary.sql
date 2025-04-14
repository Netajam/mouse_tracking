CREATE TABLE IF NOT EXISTS daily_summary (
    app_name TEXT NOT NULL,
    day_timestamp INTEGER NOT NULL, -- Start of the day timestamp
    total_duration_secs INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (app_name, day_timestamp)
)
CREATE TABLE IF NOT EXISTS hourly_summary (
    app_name TEXT NOT NULL,
    hour_timestamp INTEGER NOT NULL, -- Start of the hour timestamp
    total_duration_secs INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (app_name, hour_timestamp)
);
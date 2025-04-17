CREATE TABLE IF NOT EXISTS hourly_summary (
    app_name TEXT NOT NULL,
    detailed_window_title TEXT NOT NULL, -- Added
    hour_timestamp INTEGER NOT NULL,
    total_duration_secs INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (app_name, detailed_window_title, hour_timestamp) -- Updated PK
);
CREATE TABLE IF NOT EXISTS daily_summary (
    app_name TEXT NOT NULL,
    detailed_window_title TEXT NOT NULL, -- Added
    day_timestamp INTEGER NOT NULL,
    total_duration_secs INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (app_name, detailed_window_title, day_timestamp) -- Updated PK
);
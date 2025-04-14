CREATE TABLE IF NOT EXISTS app_intervals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    app_name TEXT NOT NULL,
    window_title TEXT, 
    start_time INTEGER NOT NULL, -- Unix timestamp (seconds)
    end_time INTEGER            -- Unix timestamp (seconds), NULLable
)
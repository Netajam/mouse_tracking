CREATE TABLE IF NOT EXISTS app_intervals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    app_name TEXT NOT NULL,
    main_window_title TEXT,
    detailed_window_title TEXT,
    start_time INTEGER NOT NULL,
    end_time INTEGER
);
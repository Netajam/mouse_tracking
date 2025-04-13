# Mouse Cursor Time Tracker

A Rust command-line tool to track application usage time based on the window currently under the mouse cursor.

**Note:** This application is currently **Windows only**.

## Features

*   **Cursor-Based Tracking:** Detects the application window directly under the mouse cursor.
*   **Time Interval Logging:** Records the start and end times for each period the cursor stays over a specific application's window.
*   **Persistent Storage:** Uses an SQLite database (`app_usage.sqlite`) to store raw time intervals persistently.
*   **Data Aggregation:** Includes logic to aggregate raw time intervals into hourly and daily summary tables within the database.
*   **Data Cleanup:** Automatically deletes raw interval data after it has been aggregated to keep the main log table smaller.
*   **Basic CLI:** Provides two commands:
    *   `run`: Starts the tracking process in the foreground.
    *   `stats`: Displays basic aggregated usage statistics (Today, Last Hour, Current Hour).
*   **Graceful Shutdown:** Handles `Ctrl+C` to stop the tracker cleanly and finalize the last recorded time interval.
*   **Standard Data Directory:** Stores the database in the user's standard data directory (e.g., `%APPDATA%\RustAppTimeTracker` on Windows).

## Platform Support

*   **Windows:** Supported and currently the only target platform due to reliance on specific Win32 APIs (`GetCursorPos`, `WindowFromPoint`, etc.).
*   **macOS / Linux:** Not supported. Would require implementing platform-specific APIs for cursor position and window detection.

## Prerequisites

*   **Rust Toolchain:** Install Rust and Cargo (latest stable recommended) from [https://rustup.rs/](https://rustup.rs/).
*   **(Optional) SQLite Viewer:** A tool like [DB Browser for SQLite](https://sqlitebrowser.org/) is helpful for inspecting the database file (`app_usage.sqlite`).

## Building

Navigate to the project's root directory in your terminal and run:

```bash
# For development (faster builds, less optimized)
cargo build

# For release (optimized, recommended for actual use)
cargo build --release
```

The executable will be located in `target/debug/mouse_tracking.exe` or `target/release/mouse_tracking.exe`. (Replace `mouse_tracking` with your actual package name if different).

## Installation (Optional - for easy access)

To make the command available system-wide from your terminal (recommended):

1.  Navigate to the project's root directory.
2.  Run:
    ```bash
    cargo install --path .
    ```
3.  This builds in release mode and copies the executable to `~/.cargo/bin`. Ensure `~/.cargo/bin` is in your system's `PATH` environment variable (the Rust installer usually does this).
4.  **Restart your terminal** for the `PATH` changes to take effect.

## Usage

After building or installing:

**1. Run the Tracker:**

```bash
# If installed via `cargo install`:
mouse_tracking run

# Or during development:
cargo run -- run

# Or directly via executable path:
target/release/mouse_tracking.exe run
```

This will start the tracking process in the foreground. It will print the database path and then remain mostly silent while running. Press `Ctrl+C` to stop the tracker gracefully. Aggregation of past data happens on startup.

**2. Display Statistics:**

```bash
# If installed via `cargo install`:
mouse_tracking stats

# Or during development:
cargo run -- stats

# Or directly via executable path:
target/release/mouse_tracking.exe stats
```

This command will connect to the database, query the summary tables (and potentially recent raw data), display statistics for Today, Last Completed Hour, and the Current Hour (approximate), and then exit. It does *not* interact with the `run` process if it's active separately.

## Data Storage

The application stores its data in an SQLite database named `app_usage.sqlite`. This file is located in a subdirectory within your user's data directory, typically:

*   **Windows:** `C:\Users\<YourUser>\AppData\Roaming\RustAppTimeTracker\`
*   **(Future Linux):** `/home/<youruser>/.local/share/RustAppTimeTracker/`
*   **(Future macOS):** `/Users/<YourUser>/Library/Application Support/RustAppTimeTracker/`

The database contains three tables:
*   `app_intervals`: Stores the raw start/end timestamps for each time the cursor is over an app. Rows are deleted after aggregation.
*   `hourly_summary`: Stores aggregated total seconds per app for each completed hour.
*   `daily_summary`: Stores aggregated total seconds per app for each completed day.

## Current Limitations

*   **Windows Only:** Requires platform-specific implementation for other operating systems.
*   **Window Detection Accuracy:** Relies on `WindowFromPoint`, which might sometimes return a handle to a child window or control within an application rather than the main application window. This can lead to entries like `TextInputHost.exe` instead of the parent app. Finding the true top-level process executable can be complex.
*   **Aggregation Simplification:** The current aggregation logic assigns the entire duration of an interval to the hour/day in which the interval *started*. Intervals spanning across hour/day boundaries are not split accurately.
*   **Basic Stats:** The `stats` command provides only simple summaries. More detailed querying (e.g., specific date ranges, excluding certain apps) is not implemented.
*   **No Configuration:** Settings like check interval or data location are hardcoded.
*   **Foreground Process:** The `run` command currently runs attached to the terminal. Running as a true background service/daemon requires additional setup (see OS-specific methods like Windows Task Scheduler).

## License

This project is licensed under the [MIT] license. 
# Mouse Cursor Time Tracker

A Rust command-line tool for Windows that tracks application usage time based on the window currently under the mouse cursor.

**Note:** This application is currently **Windows only**.

## Features

*   **Cursor-Based Tracking:** Detects the application window directly under the mouse cursor.
*   **Time Interval Logging:** Records the start and end times for each period the cursor stays over a specific application's window.
*   **Persistent Storage:** Uses an SQLite database (`app_usage.sqlite`) to store raw time intervals persistently.
*   **Data Aggregation:** Includes logic to aggregate raw time intervals into hourly and daily summary tables within the database (run automatically on startup).
*   **Data Cleanup:** Automatically deletes raw interval data after it has been aggregated to keep the main log table smaller.
*   **CLI Commands:** Provides commands for different operations:
    *   `run`: Starts the tracking process in the foreground.
    *   `stats`: Displays basic aggregated usage statistics (Today, Last Hour, Current Hour).
    *   `update`: Checks for and installs application updates from GitHub Releases.
*   **Self-Updating:** Can check for new versions on GitHub Releases and update itself (requires appropriate installation path/permissions).
*   **Automated Releases:** New versions with compiled binaries are automatically created on GitHub when a new version tag (e.g., `v0.1.0`) is pushed.
*   **Graceful Shutdown:** Handles `Ctrl+C` during the `run` command to stop the tracker cleanly and finalize the last recorded time interval.
*   **Standard Data Directory:** Stores the database in the user's standard data directory (e.g., `%APPDATA%\mouse_tracking` on Windows).

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

## Installation (Recommended for Update Feature)

To make the command available system-wide from your terminal and enable reliable updates:

1.  Navigate to the project's root directory.
2.  Run:
    ```bash
    cargo install --path . --force
    ```
    *   `--force` ensures it overwrites any existing version.
3.  This builds in release mode and copies the executable to `~/.cargo/bin`. Ensure `~/.cargo/bin` is in your system's `PATH` environment variable (the Rust installer usually does this).
4.  **Restart your terminal** for the `PATH` changes to take effect.

*(Note: Installing via `cargo install` to the default user directory (`~/.cargo/bin`) is recommended for the `update` command to have the necessary permissions to replace the executable.)*

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

This will start the tracking process in the foreground. It will print the database path, run initial aggregation/cleanup, and then remain mostly silent while running. Press `Ctrl+C` to stop the tracker gracefully.

**2. Display Statistics:**

```bash
# If installed via `cargo install`:
mouse_tracking stats

# Or during development:
cargo run -- stats

# Or directly via executable path:
target/release/mouse_tracking.exe stats
```

This command connects to the database, queries the summary tables (and potentially recent raw data), displays statistics for Today, Last Completed Hour, and the Current Hour (approximate), and then exits.

**3. Update the Application:**

```bash
# If installed via `cargo install`:
mouse_tracking update

# Or during development (will check but might fail replacing the debug build):
cargo run -- update
```

This command connects to GitHub ([github.com/Netajam/mouse_tracking](https://github.com/Netajam/mouse_tracking)), checks for a newer release matching your OS, downloads it, and replaces the current executable if an update is found and permissions allow.

## Data Storage

The application stores its data in an SQLite database named `app_usage.sqlite`. This file is located in a subdirectory within your user's data directory, typically:

*   **Windows:** `C:\Users\<YourUser>\AppData\Roaming\mouse_tracking\`

The database contains three tables:
*   `app_intervals`: Stores the raw start/end timestamps for each time the cursor is over an app. Rows are deleted after aggregation.
*   `hourly_summary`: Stores aggregated total seconds per app for each completed hour.
*   `daily_summary`: Stores aggregated total seconds per app for each completed day.

## Current Limitations

*   **Windows Only:** Requires platform-specific implementation for other operating systems.
*   **Window Detection Accuracy:** Relies on `WindowFromPoint`, which might sometimes return a handle to a child window or control within an application rather than the main application window. This can lead to entries like `TextInputHost.exe` instead of the parent app.
*   **Aggregation Simplification:** The current aggregation logic assigns the entire duration of an interval to the hour/day in which the interval *started*. Intervals spanning across hour/day boundaries are not split accurately.
*   **Basic Stats:** The `stats` command provides only simple summaries. More detailed querying (e.g., specific date ranges, excluding certain apps) is not implemented.
*   **Update Permissions:** The `update` command requires write access to the executable's location. It works best when installed via `cargo install` but may fail due to permissions if installed system-wide or in protected directories.
*   **No Configuration:** Settings like check interval or data location are hardcoded.
*   **Foreground Process:** The `run` command runs attached to the terminal. For background operation, use OS-specific tools like Windows Task Scheduler to launch the `run` command (pointing to the installed executable, e.g., in `~/.cargo/bin`).

## License

This project is licensed under the MIT license.
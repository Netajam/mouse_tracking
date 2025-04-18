use crate::errors::AppError; // Assuming AppError is defined elsewhere
use clap::ValueEnum; // Needed for CLI integration
use std::fmt;

// --- Enums for Control Flow ---

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregationLevel {
    /// Aggregate usage time by application name only
    #[value(name = "app")] // How it appears in CLI help/parsing
    ByApplication,
    /// Show usage time for each application and window title combination
    #[value(name = "detailed")]
    Detailed,
}

// Implement Display for better printing in headers etc.
impl fmt::Display for AggregationLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AggregationLevel::ByApplication => write!(f, "By Application"),
            AggregationLevel::Detailed => write!(f, "Detailed (App + Title)"),
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimePeriod {
    Today,
    LastCompletedHour,
    CurrentHour,
    // Future ideas:
    // Yesterday,
    // ThisWeek,
    // Last7Days,
    // SpecificDate(chrono::NaiveDate),
    // DateRange(i64, i64), // Using timestamps
    // AllTime,
}

impl fmt::Display for TimePeriod {
     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
         match self {
             TimePeriod::Today => write!(f, "Today"),
             TimePeriod::LastCompletedHour => write!(f, "Last Completed Hour"),
             TimePeriod::CurrentHour => write!(f, "Current Hour (Approx)"),
         }
     }
 }


// --- Structs for Data Representation ---

/// Represents detailed usage aggregated by app and title (from summary tables)
#[derive(Debug, Clone)]
pub struct DetailedUsageRecord {
    pub app_name: String,
    pub detailed_title: String,
    pub total_duration_secs: i64,
}

/// Represents the possible results from querying statistics
#[derive(Debug)]
pub enum AggregatedResult {
    /// Results aggregated only by application name
    ByApp(Vec<(String, i64)>), // Vec<(app_name, total_secs)>
    /// Results aggregated by application name and window title
    Detailed(Vec<DetailedUsageRecord>),
}

// Helper to check if the result contains any data
impl AggregatedResult {
    pub fn is_empty(&self) -> bool {
        match self {
            AggregatedResult::ByApp(v) => v.is_empty(),
            AggregatedResult::Detailed(v) => v.is_empty(),
        }
    }
}

// --- Type Aliases ---
// If your AppError isn't directly usable with rusqlite, create mapping or a specific error enum
pub type AppResult<T> = Result<T, AppError>; // Assuming AppError can wrap rusqlite::Error

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ApiKeyType {
    #[value(name = "openai")] // CLI argument name
    OpenAI,
    #[value(name = "google")] // Example for future
    Google,
    // Add other key types here as needed
}

// Implement Display for user messages
impl fmt::Display for ApiKeyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiKeyType::OpenAI => write!(f, "OpenAI"),
            ApiKeyType::Google => write!(f, "Google"),
        }
    }
}

// Helper to get the keyring 'username' (key identifier) for a type
impl ApiKeyType {
    pub fn keyring_username(&self) -> &'static str {
        match self {
            // These MUST be unique within your app's keyring service
            ApiKeyType::OpenAI => "openai_api_key",
            ApiKeyType::Google => "google_api_key",
        }
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum ConfigCommand {
    /// Set an API Key securely (e.g., openai, google)
    SetKey {
        /// The type of API key to set
        #[arg(value_enum)] // Use the enum directly
        key_type: ApiKeyType,
    },
    // No GetKey or DeleteKey based on your requirements
}
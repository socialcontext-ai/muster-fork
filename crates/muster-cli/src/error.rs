use std::fmt;

/// CLI-specific error type.
///
/// Wraps both library errors and user-facing validation errors. The `Display`
/// impl produces the message shown to the user — no additional formatting
/// needed in `main()`.
#[derive(Debug)]
pub(crate) enum CliError {
    /// A user-facing error (e.g. "Profile not found: foo").
    /// Displayed as-is to stderr, then exit 1.
    User(String),

    /// An error propagated from the library or other infrastructure.
    Internal(Box<dyn std::error::Error>),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User(msg) => write!(f, "{msg}"),
            Self::Internal(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<muster::Error> for CliError {
    fn from(e: muster::Error) -> Self {
        Self::Internal(Box::new(e))
    }
}

impl From<serde_json::Error> for CliError {
    fn from(e: serde_json::Error) -> Self {
        Self::Internal(Box::new(e))
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        Self::Internal(Box::new(e))
    }
}

impl From<toml::ser::Error> for CliError {
    fn from(e: toml::ser::Error) -> Self {
        Self::Internal(Box::new(e))
    }
}

impl From<toml::de::Error> for CliError {
    fn from(e: toml::de::Error) -> Self {
        Self::Internal(Box::new(e))
    }
}

impl From<String> for CliError {
    fn from(s: String) -> Self {
        Self::User(s)
    }
}

impl From<&str> for CliError {
    fn from(s: &str) -> Self {
        Self::User(s.to_string())
    }
}

pub(crate) type Result<T = ()> = std::result::Result<T, CliError>;

/// Convenience for creating a user-facing error.
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::error::CliError::User(format!($($arg)*)))
    };
}

pub(crate) use bail;

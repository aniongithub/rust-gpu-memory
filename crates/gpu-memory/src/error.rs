//! Error types.

use std::fmt;

/// Errors returned by [`crate::list_gpus`].
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The current platform is not supported.
    UnsupportedPlatform,
    /// An I/O error while reading platform data (sysfs, /proc, ...).
    Io(std::io::Error),
    /// A platform data source could not be parsed.
    Parse {
        /// What was being parsed, e.g. `"/proc/meminfo"`.
        source: String,
        detail: String,
    },
    /// A platform-specific API call failed (e.g. a DXGI call on Windows).
    Platform { detail: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnsupportedPlatform => write!(f, "this platform is not supported yet"),
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::Parse { source, detail } => {
                write!(f, "failed to parse {source}: {detail}")
            }
            Error::Platform { detail } => write!(f, "platform error: {detail}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// Convenience result type for this crate.
pub type Result<T> = std::result::Result<T, Error>;

use std::fmt;
use std::io;

#[non_exhaustive]
/// Errors returned by tray-icon.
#[derive(Debug)]
pub enum Error {
    OsError(io::Error),
    NotMainThread,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::OsError(err) => write!(f, "OS error: {}", err),
            Error::NotMainThread => write!(f, "Not on the main thread"),
        }
    }
}

impl std::error::Error for Error {}

/// Convenient type alias of Result type for tray-icon.
pub type Result<T> = core::result::Result<T, Error>;

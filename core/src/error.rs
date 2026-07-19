//! Error handling for Slate.
//!
//! One error type per crate does not scale for a desktop shell; one *shared*
//! error type with explicit kinds keeps the entire workspace consistent while
//! avoiding external error-handling crates.

use std::fmt;

/// Convenient result alias used across the workspace.
pub type Result<T> = std::result::Result<T, Error>;

/// The shared Slate error type.
#[derive(Debug)]
pub enum Error {
    /// An I/O operation failed.
    Io(std::io::Error),
    /// Structured text (config, theme, layout spec, manifest) failed to parse.
    Parse { what: &'static str, line: usize, msg: String },
    /// A named entity (note, theme, workspace, plugin…) does not exist.
    NotFound(String),
    /// Input was well-formed but unacceptable.
    Invalid(String),
    /// An optional external tool or service is missing/unreachable.
    Unavailable(String),
    /// Anything else.
    Other(String),
}

impl Error {
    /// Construct a parse error at line `0` (unknown).
    pub fn parse(what: &'static str, msg: impl Into<String>) -> Self {
        Self::Parse { what, line: 0, msg: msg.into() }
    }

    /// Construct a parse error with a line number.
    pub fn parse_at(what: &'static str, line: usize, msg: impl Into<String>) -> Self {
        Self::Parse { what, line, msg: msg.into() }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::Invalid(msg.into())
    }

    pub fn unavailable(msg: impl Into<String>) -> Self {
        Self::Unavailable(msg.into())
    }

    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "io error: {e}"),
            Error::Parse { what, line, msg } => {
                if *line > 0 {
                    write!(f, "invalid {what} (line {line}): {msg}")
                } else {
                    write!(f, "invalid {what}: {msg}")
                }
            }
            Error::NotFound(m) => write!(f, "not found: {m}"),
            Error::Invalid(m) => write!(f, "invalid input: {m}"),
            Error::Unavailable(m) => write!(f, "unavailable: {m}"),
            Error::Other(m) => write!(f, "{m}"),
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

impl From<fmt::Error> for Error {
    fn from(_: fmt::Error) -> Self {
        Error::other("formatting failed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_context() {
        let e = Error::parse_at("config", 3, "expected value");
        assert_eq!(e.to_string(), "invalid config (line 3): expected value");
    }

    #[test]
    fn io_error_converts() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "nope");
        let e: Error = io.into();
        assert!(matches!(e, Error::Io(_)));
        assert!(e.to_string().contains("nope"));
    }
}

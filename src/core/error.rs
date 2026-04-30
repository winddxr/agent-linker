use std::{error, fmt, io};

use crate::core::symlink::SymlinkError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidArguments(String),
    Manifest(String),
    NotImplemented(String),
    Project(String),
    Symlink(SymlinkError),
    Io(String),
}

impl Error {
    pub fn invalid_arguments(message: impl Into<String>) -> Self {
        Self::InvalidArguments(message.into())
    }

    pub fn manifest(message: impl Into<String>) -> Self {
        Self::Manifest(message.into())
    }

    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self::NotImplemented(message.into())
    }

    pub fn project(message: impl Into<String>) -> Self {
        Self::Project(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidArguments(message) => write!(formatter, "{message}"),
            Error::Manifest(message) => write!(formatter, "{message}"),
            Error::NotImplemented(message) => write!(formatter, "{message}"),
            Error::Project(message) => write!(formatter, "{message}"),
            Error::Symlink(error) => write!(formatter, "{error}"),
            Error::Io(message) => write!(formatter, "{message}"),
        }
    }
}

impl error::Error for Error {}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<SymlinkError> for Error {
    fn from(error: SymlinkError) -> Self {
        Self::Symlink(error)
    }
}

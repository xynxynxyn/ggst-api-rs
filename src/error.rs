use std::{error, fmt};
#[derive(Debug)]
pub enum Error {
    ReqwestError(reqwest::Error),
    ChronoParseError(chrono::ParseError),
    ParsingBytesError(String, &'static str),
    UnexpectedResponse(String, &'static str),
    InvalidCharacterCode(&'static str),
    InvalidArgument(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::ReqwestError(e) => write!(f, "Error making request: {}", e),
            Error::ChronoParseError(e) => write!(f, "Error parsing datetime: {}", e),
            Error::ParsingBytesError(bytes, msg) => write!(f, "{}: {}", msg, bytes),
            Error::UnexpectedResponse(bytes, msg) => {
                write!(f, "Unexpected response from API, {}: {}", msg, bytes)
            }
            Error::InvalidCharacterCode(code) => write!(f, "{} is not valid character code", code),
            Error::InvalidArgument(msg) => write!(f, "Invalid argument: {}", msg),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::ReqwestError(e)
    }
}

impl From<chrono::ParseError> for Error {
    fn from(e: chrono::ParseError) -> Self {
        Error::ChronoParseError(e)
    }
}

impl error::Error for Error {}

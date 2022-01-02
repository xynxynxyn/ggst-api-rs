use std::{error, fmt};
#[derive(Debug)]
pub enum Error {
    ReqwestError(reqwest::Error),
    SerdeError(serde_json::Error),
    ChronoParseError(chrono::ParseError),
    ParsingBytesError(String, &'static str),
    UnexpectedResponse(String, &'static str),
    InvalidCharacterCode(&'static str),
    InvalidArgument(String),
    JsonParsingError(serde_json::Value),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::ReqwestError(e) => write!(f, "Error making request: {}", e),
            Error::SerdeError(e) => write!(f, "Error parsing data: {}", e),
            Error::ChronoParseError(e) => write!(f, "Error parsing datetime: {}", e),
            Error::ParsingBytesError(bytes, msg) => write!(f, "{}: {}", msg, bytes),
            Error::UnexpectedResponse(bytes, msg) => {
                write!(f, "Unexpected response from API, {}: {}", msg, bytes)
            }
            Error::InvalidCharacterCode(code) => write!(f, "{} is not valid character code", code),
            Error::InvalidArgument(msg) => write!(f, "Invalid argument: {}", msg),
            Error::JsonParsingError(v) => write!(f, "Could not parse json value: {}", v),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::ReqwestError(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::SerdeError(e)
    }
}

impl From<chrono::ParseError> for Error {
    fn from(e: chrono::ParseError) -> Self {
        Error::ChronoParseError(e)
    }
}

impl error::Error for Error {}

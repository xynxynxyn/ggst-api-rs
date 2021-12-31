use std::{error, fmt};
#[derive(Debug)]
pub enum Error {
    HttpError(reqwest::Error),
    UnexpectedResponse(String),
    ParsingError(serde_json::Error),
    InvalidCharacterCode(String),
    InvalidArguments(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::HttpError(e) => write!(f, "Error making request: {}", e),
            Error::ParsingError(e) => write!(f, "Error parsing data: {}", e),
            Error::UnexpectedResponse(e) => write!(f, "Unexpected response from API: {}", e),
            Error::InvalidCharacterCode(code) => write!(f, "{} is not valid character code", code),
            Error::InvalidArguments(e) => write!(f, "Invalid arguments provided: {}", e),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::HttpError(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::ParsingError(e)
    }
}

impl error::Error for Error {}

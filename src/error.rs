use std::{error, fmt};
#[derive(Debug)]
pub enum Error<'a> {
    HttpError(reqwest::Error),
    UnexpectedResponse,
    ParsingError(serde_json::Error),
    InvalidCharacterCode(&'a str),
}

pub type Result<'a, T> = std::result::Result<T, Error<'a>>;

impl fmt::Display for Error<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::HttpError(e) => write!(f, "Error making request: {}", e),
            Error::ParsingError(e) => write!(f, "Error parsing data: {}", e),
            Error::UnexpectedResponse => write!(f, "Unexpected response from API"),
            Error::InvalidCharacterCode(code) => write!(f, "{} is not valid character code", code),
        }
    }
}

impl From<reqwest::Error> for Error<'_> {
    fn from(e: reqwest::Error) -> Self {
        Error::HttpError(e)
    }
}

impl From<serde_json::Error> for Error<'_> {
    fn from(e: serde_json::Error) -> Self {
        Error::ParsingError(e)
    }
}

impl error::Error for Error<'_> {}

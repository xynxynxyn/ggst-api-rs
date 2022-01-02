use std::{
    error,
    fmt::{self, Display},
};
#[derive(Debug)]
pub enum Error {
    ReqwestError(reqwest::Error),
    ChronoParseError(chrono::ParseError),
    ParsingBytesError(&'static str),
    UnexpectedResponse(&'static str),
    InvalidCharacterCode(&'static str),
    InvalidArgument(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::ReqwestError(e) => write!(f, "Error making request: {}", e),
            Error::ChronoParseError(e) => write!(f, "Error parsing datetime: {}", e),
            Error::ParsingBytesError(msg) => write!(f, "{}", msg),
            Error::UnexpectedResponse(msg) => {
                write!(f, "Unexpected response from API, {}", msg)
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

#[derive(Debug)]
pub struct ParseError {
    reply_content: String,
    inner: Error,
}

impl ParseError {
    pub fn new(reply_content: String, inner: Error) -> Self {
        ParseError {
            reply_content,
            inner,
        }
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Could not parse replay: {}\n  bytes: {}",
            self.inner, self.reply_content
        )
    }
}

impl error::Error for ParseError {}

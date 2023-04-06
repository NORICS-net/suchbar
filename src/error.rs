use crate::error::SuchError::{Denied, ParseError};
use crate::suchbar::Rule;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum SuchError {
    ParseError(String),
    Denied,
}

impl From<pest::error::Error<Rule>> for SuchError {
    fn from(value: pest::error::Error<Rule>) -> Self {
        ParseError(value.to_string())
    }
}

impl Display for SuchError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError(str) => write!(f, "{str}"),
            Denied => write!(f, "DENIED"),
        }
    }
}

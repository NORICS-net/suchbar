use crate::SuchError::ErrorList;
use crate::error::SuchError::{Denied, LikeNotPossible, ParseError, TypeNotComparable};
use crate::suchbar::Rule;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum SuchError {
    Denied,
    LikeNotPossible,
    ParseError(String),
    TypeNotComparable,
    ErrorList(Vec<Self>),
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
            LikeNotPossible => write!(f, "LIKE not possible"),
            Denied => write!(f, "DENIED"),
            TypeNotComparable => write!(f, "Not Comparable"),
            ErrorList(list) => {
                list.iter().for_each(|e| write!(f, "{e}").unwrap());
                Ok(())
            }
        }
    }
}

impl From<timewarp::TimeWarpError> for SuchError {
    fn from(value: timewarp::TimeWarpError) -> Self {
        ParseError(value.to_string())
    }
}

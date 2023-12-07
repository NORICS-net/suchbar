use self::CompOp::{Equal, Gt, Gte, Lt, Lte, NotEqual};
use crate::error::SuchError;
use crate::error::SuchError::ParseError;
use std::fmt::{Display, Formatter};
use std::ops::Not;
use std::str::FromStr;

#[allow(clippy::upper_case_acronyms)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub enum CompOp {
    #[default]
    Equal,
    NotEqual,
    Gt,
    Lt,
    Gte,
    Lte,
}

impl Display for CompOp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Equal => "=",
                Gt => ">",
                Gte => ">=",
                Lt => "<",
                Lte => "<=",
                NotEqual => "!=",
            }
        )
    }
}

impl FromStr for CompOp {
    type Err = SuchError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "=" | "==" => Ok(Equal),
            ">=" | "=>" => Ok(Gte),
            ">" => Ok(Gt),
            "<=" | "=<" => Ok(Lte),
            "<" => Ok(Lt),
            "!=" | "=!" => Ok(NotEqual),
            _ => Err(ParseError(format!("'{s}' is no comparator!"))),
        }
    }
}

impl Not for CompOp {
    type Output = CompOp;

    fn not(self) -> Self::Output {
        match &self {
            Equal => NotEqual,
            NotEqual => Equal,
            Gt => Lte,
            Gte => Lt,
            Lte => Gt,
            Lt => Gte,
        }
    }
}

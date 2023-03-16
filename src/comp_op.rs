use self::CompOp::*;
use crate::error::SuchError;
use crate::error::SuchError::ParseError;
use std::fmt::{Display, Formatter};
use std::ops::Not;
use std::str::FromStr;

#[allow(clippy::upper_case_acronyms)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub(crate) enum CompOp {
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
                NotEqual => panic!("No SQL-representation for CompOp = {self:?}"),
            }
        )
    }
}

impl FromStr for CompOp {
    type Err = SuchError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "=" => Ok(CompOp::Equal),
            "==" => Ok(CompOp::Equal),
            ">=" => Ok(CompOp::Gte),
            "=>" => Ok(CompOp::Gte),
            ">" => Ok(CompOp::Gt),
            "<=" => Ok(CompOp::Lte),
            "=<" => Ok(CompOp::Lte),
            "<" => Ok(CompOp::Lt),
            "!=" => Ok(CompOp::NotEqual),
            "=!" => Ok(CompOp::NotEqual),
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

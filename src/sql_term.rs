use crate::comp_op::CompOp;
use crate::db_field::DbField;
use crate::error::SuchError;
use crate::error::SuchError::ParseError;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use timewarp::Direction;

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug)]
pub(crate) enum SQLTerm {
    AND(Vec<SQLTerm>),
    OR(Vec<SQLTerm>),
    NOT(Box<SQLTerm>),
    VALUE(DbField, CompOp, Direction, String),
    LIKE(DbField, String),
    DENIED,
}

impl SQLTerm {
    pub fn to_sql(&self) -> Result<String, SuchError> {
        use SQLTerm::{AND, DENIED, LIKE, NOT, OR, VALUE};
        match self {
            OR(vec) => explode(vec, " OR "),
            AND(vec) => explode(vec, " AND "),
            NOT(val) => match val.deref() {
                // NOT( NOT(val)) => val
                NOT(inner) => inner.to_sql(),
                _ => Ok(format!("NOT {}", val.to_sql()?)),
            },
            VALUE(f, eq, d, v) => val_sql(f, *eq, v, *d),
            LIKE(f, v) => f.try_sql_like(v),
            DENIED => Err(SuchError::Denied),
        }
    }
}

fn val_sql(f: &DbField, eq: CompOp, v: &str, d: Direction) -> Result<String, SuchError> {
    if v.contains('*') {
        f.try_sql_like(v)
    } else {
        f.try_sql_eq(eq, v, d)
    }
}

fn explode(vec: &[SQLTerm], sep: &str) -> Result<String, SuchError> {
    let v = vec
        .iter()
        .filter_map(|op| op.to_sql().ok())
        .collect::<Vec<String>>();
    match v.len() {
        0 => Err(ParseError("Empty SQLTerm!".to_string())),
        1 => Ok(v[0].clone()),
        _ => Ok(format!("( {} )", v.join(sep))),
    }
}

impl Default for SQLTerm {
    fn default() -> Self {
        SQLTerm::OR(vec![])
    }
}

impl Display for SQLTerm {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_sql().unwrap_or_default())
    }
}

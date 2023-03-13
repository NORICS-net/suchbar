use crate::op_tree::{CompOp, SField};
use crate::sql_term::SQLTerm::*;
use anyhow::bail;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum SQLTerm {
    AND(Vec<SQLTerm>),
    OR(Vec<SQLTerm>),
    NOT(Box<SQLTerm>),
    VALUE(SField, CompOp, String),
    LIKE(SField, String),
}

impl SQLTerm {
    pub fn to_sql(&self) -> anyhow::Result<String> {
        use SQLTerm::*;
        match self {
            OR(vec) => explode(vec, " OR "),
            AND(vec) => explode(vec, " AND "),
            NOT(val) => Ok(format!("NOT {}", val.to_sql()?)),
            VALUE(f, eq, v) => f.try_sql_eq(*eq, v),
            LIKE(f, v) => f.try_sql_like(v),
        }
    }
}

fn explode(vec: &[SQLTerm], sep: &str) -> anyhow::Result<String> {
    let v = vec
        .iter()
        .filter_map(|op| op.to_sql().ok())
        .collect::<Vec<String>>();
    match v.len() {
        0 => bail!("Empty SQLTerm!"),
        1 => Ok(v[0].clone()),
        _ => Ok(format!("( {} )", v.join(sep))),
    }
}

impl Default for SQLTerm {
    fn default() -> Self {
        OR(vec![])
    }
}

impl Display for SQLTerm {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_sql().unwrap_or_default())
    }
}

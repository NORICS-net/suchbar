use crate::comp_op::CompOp;
use crate::db_field::DbField;
use crate::error::SuchError;
use crate::error::SuchError::ParseError;
use std::fmt::{Display, Formatter};
use timewarp::Direction;

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug)]
pub enum SQLTerm {
    AND(Vec<SQLTerm>),
    OR(Vec<SQLTerm>),
    NOT(Box<SQLTerm>),
    VALUE(DbField, CompOp, Direction, String),
    LIKE(DbField, String),
    DENIED,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Style {
    Compact,
    Pretty,
    Html,
}

enum Combinator {
    AND,
    OR,
}

impl Combinator {
    const fn to_sql(&self) -> &'static str {
        match self {
            Self::AND => " AND ",
            Self::OR => " OR ",
        }
    }

    const fn to_text(&self, style: Style) -> &'static str {
        match style {
            Style::Compact => self.to_compact(),
            Style::Pretty => self.to_pretty(),
            Style::Html => self.to_html(),
        }
    }

    const fn to_compact(&self) -> &'static str {
        match self {
            Self::AND => "&&",
            Self::OR => "||",
        }
    }

    const fn to_html(&self) -> &'static str {
        match self {
            Self::AND => "<span class=\"syntax_combinator syntax_b_and\">&amp;&amp;</span>",
            Self::OR => "<span class=\"syntax_combinator syntax_b_or\">||</span>",
        }
    }

    const fn to_pretty(&self) -> &'static str {
        match self {
            Self::AND => " && ",
            Self::OR => " || ",
        }
    }
}

impl SQLTerm {
    /// Emits the SQL-token of this term and it's children.
    pub fn to_sql(&self) -> Result<String, SuchError> {
        use SQLTerm::{AND, DENIED, LIKE, NOT, OR, VALUE};
        match self {
            OR(vec) => explode_sql(vec, Combinator::OR),
            AND(vec) => explode_sql(vec, Combinator::AND),
            NOT(val) => match &**val {
                // NOT( NOT(val)) => val
                NOT(inner) => inner.to_sql(),
                _ => Ok(format!("NOT {}", val.to_sql()?)),
            },
            VALUE(f, eq, d, v) => val_sql(f, *eq, v, *d),
            LIKE(f, v) => f.try_sql_like(v),
            DENIED => Err(SuchError::Denied),
        }
    }

    pub fn to_url(&self, style: Style) -> Result<String, SuchError> {
        use SQLTerm::{AND, DENIED, LIKE, NOT, OR, VALUE};
        match self {
            OR(vec) => explode_text(vec, Combinator::OR, style),
            AND(vec) => explode_text(vec, Combinator::AND, style),
            NOT(val) => match &**val {
                // NOT( NOT(val)) => val
                NOT(inner) => inner.to_url(style),
                VALUE(f, eq, _, v) => Ok(f.to_text(style, !*eq, v)),
                _ => Ok(format!("!{}", val.to_url(style)?)),
            },
            VALUE(f, eq, _, v) => Ok(f.to_text(style, *eq, v)),
            LIKE(f, v) => Ok(f.to_text(style, CompOp::Equal, v)),
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

fn explode_text(
    vec: &[SQLTerm],
    combinator: Combinator,
    style: Style,
) -> Result<String, SuchError> {
    let v = vec
        .iter()
        .filter_map(|op| op.to_url(style).ok())
        .collect::<Vec<String>>();
    match v.len() {
        0 => Err(ParseError("Empty SQLTerm!".to_string())),
        1 => Ok(v[0].clone()),
        _ => Ok(match style {
            Style::Html => format!("<span class=\"syntax_bracket syntax_b_start\">(</span>{}<span class=\"syntax_bracket syntax_b_end\">)</span>",
                v.join(combinator.to_text(style))),
            _ => format!("({})", v.join(combinator.to_text(style))),
        }),
    }
}

fn explode_sql(vec: &[SQLTerm], combinator: Combinator) -> Result<String, SuchError> {
    let v = vec
        .iter()
        .filter_map(|op| op.to_sql().ok())
        .collect::<Vec<String>>();
    match v.len() {
        0 => Err(ParseError("Empty SQLTerm!".to_string())),
        1 => Ok(v[0].clone()),
        _ => Ok(format!("( {} )", v.join(combinator.to_sql()))),
    }
}

impl Default for SQLTerm {
    fn default() -> Self {
        Self::OR(vec![])
    }
}

impl Display for SQLTerm {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_sql().unwrap_or_default())
    }
}

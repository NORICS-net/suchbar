use crate::db_field::DbField;
use crate::error::SuchError;
use crate::error::SuchError::ParseError;
use crate::{SuchError::ErrorList, comp_op::CompOp};
use itertools::Itertools;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use timewarp::Direction::{self, From};

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug)]
pub enum SQLTerm {
    AND(Vec<Self>),
    OR(Vec<Self>),
    NOT(Box<Self>),
    VALUE(DbField, CompOp, Direction, String),
    LIKE(DbField, String),
    BETWEEN(DbField, String, String),
    LIKETERM(DbField, String),
    TERM(DbField, String),
    DENIED,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Style {
    Compact,
    Pretty,
    Html,
    Url,
}

enum Combinator {
    And,
    Or,
}

impl Combinator {
    const fn to_sql(&self) -> &'static str {
        match self {
            Self::And => " AND ",
            Self::Or => " OR ",
        }
    }

    const fn to_text(&self, style: Style) -> &'static str {
        match style {
            Style::Compact => self.to_compact(),
            Style::Pretty => self.to_pretty(),
            Style::Html => self.to_html(),
            Style::Url => self.to_url(),
        }
    }

    const fn to_compact(&self) -> &'static str {
        match self {
            Self::And => "&&",
            Self::Or => "||",
        }
    }

    const fn to_url(&self) -> &'static str {
        match self {
            Self::And => " AND ",
            Self::Or => " OR ",
        }
    }

    const fn to_html(&self) -> &'static str {
        match self {
            Self::And => "<span class=\"syntax_combinator syntax_c_and\">&amp;&amp;</span>",
            Self::Or => "<span class=\"syntax_combinator syntax_c_or\">||</span>",
        }
    }

    const fn to_pretty(&self) -> &'static str {
        match self {
            Self::And => " && ",
            Self::Or => " || ",
        }
    }
}

impl SQLTerm {
    /// Emits the SQL-token of this term and it's children.
    pub fn to_sql(&self) -> Result<String, SuchError> {
        use SQLTerm::{AND, BETWEEN, DENIED, LIKE, LIKETERM, NOT, OR, TERM, VALUE};
        match self {
            OR(vec) => explode_sql(vec, Combinator::Or),
            AND(vec) => explode_sql(vec, Combinator::And),
            NOT(val) => match &**val {
                // NOT( NOT(val)) => val
                NOT(inner) => inner.to_sql(),
                _ => Ok(format!("NOT {}", val.to_sql()?)),
            },
            VALUE(f, eq, d, v) => val_sql(f, *eq, v, *d),
            TERM(f, v) => Ok(val_sql(f, CompOp::Equal, v, From).unwrap_or_default()),
            LIKE(f, v) => f.try_sql_like(v),
            LIKETERM(f, v) => {
                let value = match (v.starts_with("*"), v.ends_with("*")) {
                    (false, false) => format!("*{v}*"),
                    _ => v.to_owned(),
                };
                Ok(f.try_sql_like(&value).unwrap_or_default())
            }
            DENIED => Err(SuchError::Denied),
            BETWEEN(f, from, to) => f.try_sql_between(from, to),
        }
    }

    pub fn as_text(&self, style: Style) -> Result<String, SuchError> {
        use SQLTerm::{AND, BETWEEN, DENIED, LIKE, LIKETERM, NOT, OR, TERM, VALUE};
        match self {
            OR(vec) => explode_text(vec, Combinator::Or, style),
            AND(vec) => explode_text(vec, Combinator::And, style),
            NOT(val) => match &**val {
                // NOT( NOT(val)) => val
                NOT(inner) => inner.as_text(style),
                VALUE(f, eq, _, v) => Ok(f.as_text(style, !*eq, v)),
                _ => Ok(format!("!{}", val.as_text(style)?)),
            },
            VALUE(f, eq, _, v) => Ok(f.as_text(style, *eq, v)),
            TERM(_, _) => Ok(String::new()),
            LIKE(f, v) => Ok(f.as_text(style, CompOp::Equal, v)),
            LIKETERM(_, _) => Ok(String::new()),
            BETWEEN(f, from, to) => Ok(between_text(&f, style, from, to)),
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
    let (v, e): (Vec<_>, Vec<_>) = vec.iter().map(|op| op.as_text(style)).partition_result();
    if !e.is_empty() {
        return Err(ErrorList(e));
    }
    let terms = vec
        .iter()
        .filter_map(|term| match term {
            SQLTerm::TERM(_, v) => Some(v.to_owned()),
            SQLTerm::LIKETERM(_, v) => Some(v.to_owned()),
            _ => None,
        })
        .collect::<HashSet<_>>();

    let v = v
        .into_iter()
        .filter(|s| !s.is_empty())
        .chain(terms.into_iter())
        .collect::<Vec<_>>();

    match v.len() {
        0 => Err(ParseError("Empty SQLTerm (as_text)!".to_string())),
        1 => Ok(v[0].clone()),
        _ => Ok(match style {
            Style::Html => format!(
                "<span class=\"syntax_bracket\"><span class=\"syntax_b_start\">\
                (</span><div class=\"syntax_in_brackets\">{}</div><span class=\"syntax_b_end\">)\
                </span></span>",
                v.join(combinator.to_text(style))
            ),
            _ => format!("({})", v.join(combinator.to_text(style))),
        }),
    }
}

fn explode_sql(vec: &[SQLTerm], combinator: Combinator) -> Result<String, SuchError> {
    let (v, e): (Vec<_>, Vec<_>) = vec.iter().map(SQLTerm::to_sql).partition_result();
    if !e.is_empty() {
        return Err(ErrorList(e));
    }
    let v = v.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>();
    match v.len() {
        0 => Ok(String::new()),
        1 => Ok(v[0].clone()),
        _ => Ok(format!("( {} )", v.join(combinator.to_sql()))),
    }
}

fn between_text(field: &DbField, style: Style, from: &str, to: &str) -> String {
    match style {
        Style::Url | Style::Compact => format!(
            "{}={}~{}",
            field.name(style),
            field.val(style, from),
            field.val(style, to)
        ),
        Style::Pretty => format!(
            "{}={}..{}",
            field.name(style),
            field.val(style, from),
            field.val(style, to)
        ),
        Style::Html => format!(
            "<span class=\"syntax_field\">{}</span><span class=\"syntax_operator\">=</span>\
             <span class=\"syntax_{s_type}\">{}</span><span class=\"syntax_between\">..</span>\
             <span class=\"syntax_{s_type}\">{}</span>",
            field.name(style).to_uppercase(),
            field.val(style, from),
            field.val(style, to),
            s_type = field.db_type().to_lowercase(),
        ),
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

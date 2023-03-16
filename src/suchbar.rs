use crate::comp_op::CompOp;
use crate::db_field::{DbField, SortField};
use crate::error::SuchError;
use crate::sql_term::SQLTerm;
use crate::sql_term::SQLTerm::{AND, LIKE, NOT, OR, VALUE};
use pest::iterators::Pair;
use pest::Parser;
use std::fmt::Display;
use std::ops::Not;
use std::str::FromStr;

type SuchResult = Result<SQLTerm, SuchError>;

#[derive(Parser, Debug, Default)]
#[grammar = "suchbar.pest"]
pub struct Suchbar {
    pub db_fields: Vec<DbField>,
    sql_term: SQLTerm,
    sort_field: Vec<SortField>,
}

impl Suchbar {
    pub fn new(db_fields: &[DbField]) -> Self {
        Self {
            db_fields: db_fields.into(),
            ..Default::default()
        }
    }

    pub fn exec(&mut self, query: impl Into<String>) -> Result<(), SuchError> {
        let query = query.into();
        let qu = Self::parse(Rule::query, &query)?;
        for expr in qu {
            match expr.as_rule() {
                Rule::expr => self.sql_term = self.parse_expr(expr)?,
                Rule::sort => self.sort_field = self.parse_sort(expr),
                _ => {} //ignore EOI and rest
            }
        }
        Ok(())
    }

    fn choose_field(&self, needle: &str) -> Option<DbField> {
        let needle = needle.to_ascii_lowercase();
        self.db_fields
            .iter()
            .find(|sf| sf.alias.iter().any(|s| *s == needle))
            .cloned()
    }

    fn choose_field_vec(&self, needle: &str) -> Vec<DbField> {
        if let Some(f) = self.choose_field(needle) {
            vec![f]
        } else {
            self.db_fields.clone()
        }
    }

    /// expr = { atom ~ (bin_op? ~ atom)* }
    fn parse_expr(&self, expr: Pair<Rule>) -> SuchResult {
        let mut acc = Vec::new();
        let mut or = false;
        let mut comp_op = CompOp::Equal;
        for exp in expr.into_inner() {
            //println!("** Suchbar::parse_expr:: {:?}", exp);
            match exp.as_rule() {
                Rule::field => {
                    if let Ok(field) = self.parse_field(exp) {
                        acc.push(field);
                    }
                }
                Rule::or => or = true,
                Rule::and => or = false,
                Rule::invert => comp_op = !comp_op,
                Rule::term => acc.push(self.parse_term(None, comp_op, exp)?),
                Rule::expr => acc.push(self.parse_expr(exp)?),
                _ => {
                    println!("=> Suchbar::parse_expr:: {exp:?}");
                }
            };
        }
        if or {
            Ok(OR(acc))
        } else {
            Ok(AND(acc))
        }
    }

    fn parse_field(&self, expr: Pair<Rule>) -> SuchResult {
        let mut name = "";
        let mut not = false;
        let mut comp_op = CompOp::default();
        for exp in expr.into_inner() {
            match exp.as_rule() {
                Rule::eq => comp_op = CompOp::from_str(exp.as_str()).unwrap_or_default(),
                Rule::field_name => name = exp.as_str(),
                Rule::invert => not = !not,
                Rule::term => {
                    return self.parse_term(
                        Some(name),
                        if not { comp_op.not() } else { comp_op },
                        exp,
                    );
                }
                _ => {
                    println!("=> Suchbar::parse_field:: {exp:?}");
                }
            }
        }
        Err(SuchError::ParseError(format!(
            "Field '{name}' not parsable."
        )))
    }

    fn parse_term(&self, name: Option<&str>, comp_op: CompOp, expr: Pair<Rule>) -> SuchResult {
        let mut value = String::new();
        let mut like_ending = false;
        let mut like_starting = false;
        let mut to_val = None;
        for exp in expr.into_inner() {
            match exp.as_rule() {
                Rule::starts_with => {
                    if exp.as_str() == "*" {
                        like_starting = true;
                    } else {
                        like_ending = true;
                    }
                }
                Rule::ends_with => {
                    if exp.as_str() == "*" {
                        like_ending = true;
                    } else {
                        like_starting = true;
                    }
                }
                Rule::from_to => to_val = Self::parse_value(exp.into_inner().next().unwrap()),
                Rule::value => value = Self::parse_value(exp).unwrap_or_default(),
                _ => {
                    println!("=> Suchbar::parse_term:: {exp:?}");
                }
            }
        }

        Ok(OR(self
            .choose_field_vec(name.unwrap_or_default())
            .into_iter()
            .map(|sf| {
                let val = if like_ending || like_starting {
                    let value = if like_ending && like_starting {
                        format!("*{}*", value)
                    } else if like_starting {
                        format!("*{}", value)
                    } else {
                        format!("{}*", value)
                    };
                    LIKE(sf, value)
                } else if name.is_none() {
                    // list of terms means LIKE-search.
                    LIKE(sf, format!("*{}*", value))
                } else if to_val.is_some() {
                    AND(vec![
                        VALUE(sf.clone(), CompOp::Gte, value.clone()),
                        VALUE(sf, CompOp::Lte, to_val.clone().unwrap_or_default()),
                    ])
                } else {
                    VALUE(sf, comp_op, value.clone())
                };
                if comp_op == CompOp::NotEqual {
                    NOT(Box::new(val))
                } else {
                    val
                }
            })
            .collect()))
    }

    fn parse_value(expr: Pair<Rule>) -> Option<String> {
        if let Some(exp) = expr.into_inner().next() {
            match exp.as_rule() {
                Rule::raw_string => Some(exp.as_str().to_string()),
                Rule::raw_string_interior => {
                    // cut off surrounding quotes
                    let (_, s) = exp.as_str().split_at(0);
                    let (s, _) = s.split_at(s.len());
                    Some(String::from(s))
                }
                _ => {
                    println!("=> Suchbar::parse_value:: {exp:?}");
                    None
                }
            }
        } else {
            None
        }
    }

    fn parse_sort(&self, sort: Pair<Rule>) -> Vec<SortField> {
        let mut sort_fields = Vec::new();
        let mut desc = false;
        for so in sort.into_inner() {
            match so.as_rule() {
                Rule::down => desc = true,
                Rule::sort_field => {
                    if let Some(field) = self.choose_field(so.as_str()) {
                        sort_fields.push(SortField { field, desc });
                        desc = false
                    }
                }
                _ => {}
            }
        }
        sort_fields
    }

    pub fn to_where(&self) -> Result<String, SuchError> {
        self.sql_term.to_sql()
    }

    pub fn to_sql(&self, prefix: impl Display) -> String {
        let whr = self.to_where().unwrap_or_default();
        let whr = if whr.is_empty() {
            whr
        } else {
            format!(" {prefix} {whr}")
        };
        let sort = if self.sort_field.is_empty() {
            String::new()
        } else {
            format!(
                " SORT BY {}",
                self.sort_field
                    .iter()
                    .map(|sf| sf.to_sql())
                    .collect::<Vec<String>>()
                    .join(", ")
            )
        };
        format!("{whr}{sort}")
    }
}

#[cfg(test)]
mod should {
    use super::Suchbar;
    use crate::db_field::DbField;
    use crate::db_field::DbType::{INTEGER, NUMERIC, TEXT, VARCHAR};

    const FIELDS: [DbField; 5] = [
        DbField::new(
            "artikelnummer",
            VARCHAR(18),
            "READ_OFFER",
            &["art", "artnr", "artikelnummer", "artikelnr", "ano"],
        ),
        DbField::new(
            "positionstext",
            TEXT,
            "READ_OFFER",
            &["beschreibung", "desc", "description", "ptext"],
        ),
        DbField::new("price", NUMERIC(12, 2), "READ_OFFER", &["preis", "price"]),
        DbField::new("age", INTEGER(0, 150), "READ_OFFER", &["alter", "age"]),
        DbField::new(
            "promille",
            INTEGER(1, 1000),
            "READ_OFFER",
            &["number", "nummer", "promille"],
        ),
    ];

    #[test]
    fn parse_query() {
        let query = r#"ano!=23342 AND (desc=^"irgend ein langer Text!" OR price='35,12'); artnr, ^nummer, age"#;
        let mut s = Suchbar::new(&FIELDS);
        s.exec(query).expect("This should not panic!");
        assert_eq!(
            "  ( NOT artikelnummer='23342' AND ( positionstext LIKE 'irgend ein langer Text!%' \
            OR price=35.12 ) ) SORT BY artikelnummer, promille DESC, age",
            s.to_sql("")
        );
    }

    #[test]
    fn parse_from_to_values() {
        let query = r#"age=10-19"#;
        let mut s = Suchbar::new(&FIELDS);
        s.exec(query).expect("This should not panic!");
        assert_eq!("  ( age>=10 AND age<=19 )", s.to_sql(""));
    }

    #[test]
    fn parse_like_somewhere() {
        let mut s = Suchbar::new(&FIELDS);
        let query = r#"*Superman*"#;
        s.exec(query).expect("This should not panic!");
        assert_eq!(
            " WHERE ( artikelnummer LIKE '%Superman%' OR positionstext LIKE '%Superman%' )",
            s.to_sql("WHERE")
        );
        let query = r#"Superman Batman"#;
        s.exec(query).expect("This should not panic!");
        assert_eq!(
            " WHERE ( ( artikelnummer LIKE '%Superman%' OR positionstext LIKE '%Superman%' ) AND \
            ( artikelnummer LIKE '%Batman%' OR positionstext LIKE '%Batman%' ) )",
            s.to_sql("WHERE")
        );
    }
}

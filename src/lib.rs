mod op_tree;
mod sql_term;

use crate::op_tree::{CompOp, SField};
use crate::sql_term::SQLTerm;
use crate::sql_term::SQLTerm::{AND, LIKE, NOT, OR, VALUE};
use pest::iterators::Pair;
use pest::Parser;
use std::str::FromStr;

#[macro_use]
extern crate pest_derive;

#[derive(Debug)]
pub struct SortField {
    desc: bool,
    field: SField,
}

impl SortField {
    pub fn to_sql(&self) -> String {
        let name = self.field.db_name;
        match self.desc {
            true => format!("{name} DESC"),
            false => format!("{name}"),
        }
    }
}

#[derive(Debug, Parser, Default)]
#[grammar = "query.pest"]
pub struct Suchbar {
    pub s_fields: Vec<SField>,
    abstract_filter: SQLTerm,
    sort: Vec<SortField>,
}

impl Suchbar {
    pub fn exec(self, query: impl Into<String>) -> anyhow::Result<Self> {
        let query = query.into();
        let qu = Self::parse(Rule::query, &query)?;
        //     println!("{:#?}", qu);
        let mut this = self;
        for expr in qu {
            match expr.as_rule() {
                Rule::expr => this.abstract_filter = this.parse_expr(expr)?,
                Rule::sort => this.sort = this.parse_sort(expr),
                _ => {} //ignore EOI and rest
            }
        }
        Ok(this)
    }

    fn choose_field(&self, needle: &str) -> Option<SField> {
        let needle = needle.to_ascii_lowercase();
        self.s_fields
            .iter()
            .find(|sf| sf.alias.iter().find(|s| *s == &needle).is_some())
            .map(|sf| sf.clone())
    }

    fn choose_field_vec(&self, needle: &str) -> Vec<SField> {
        if let Some(f) = self.choose_field(needle) {
            vec![f.clone()]
        } else {
            self.s_fields.clone()
        }
    }

    /// expr = { atom ~ (bin_op? ~ atom)* }
    fn parse_expr(&self, expr: Pair<Rule>) -> anyhow::Result<SQLTerm> {
        let mut acc = Vec::new();
        let mut or = true;
        for exp in expr.into_inner() {
            //   println!("parse_expr {exp:#?}");
            match exp.as_rule() {
                Rule::field => {
                    let (field, not, comp_op, val) = Self::parse_field(exp);
                    acc.push(OR(self
                        .choose_field_vec(&field)
                        .into_iter()
                        .map(|sf| {
                            if val.contains(|c| (c == '*' || c == '^' || c == '$' || c == '~')) {
                                LIKE(sf.clone(), val.clone())
                            } else {
                                let value = VALUE(sf.clone(), comp_op, val.clone());
                                if not {
                                    NOT(Box::new(value))
                                } else {
                                    value
                                }
                            }
                        })
                        .collect()));
                }
                Rule::or => or = true,
                Rule::and => or = false,
                Rule::term => acc.push(self.parse_term(exp)?),
                Rule::expr => acc.push(self.parse_expr(exp)?),
                _ => {}
            };
        }
        if or {
            Ok(OR(acc))
        } else {
            Ok(AND(acc))
        }
    }

    fn parse_field(expr: Pair<Rule>) -> (String, bool, CompOp, String) {
        let mut name = String::new();
        let mut value = None;
        let mut not = false;
        let mut comp_op = CompOp::default();
        for exp in expr.into_inner() {
            match exp.as_rule() {
                Rule::invert => not = true,
                Rule::field_name => name = exp.as_str().to_string(),
                Rule::eq => comp_op = CompOp::from_str(exp.as_str()).unwrap_or_default(),
                Rule::term => value = Self::parse_value(exp.into_inner().next().unwrap()),
                _ => {}
            }
        }
        // println!("Field: {name} {value:?}");
        (name, not, comp_op, value.unwrap_or_default())
    }

    fn parse_term(&self, expr: Pair<Rule>) -> anyhow::Result<SQLTerm> {
        let mut value = String::new();
        let mut eq = CompOp::default();
        let mut like_ending = false;
        let mut like_starting = false;
        let mut name = "";
        let mut to_val = None;
        for exp in expr.into_inner() {
            // println!("parse_term {exp:#?}");
            match exp.as_rule() {
                Rule::ends_with => like_ending = true,
                Rule::from_to => to_val = Self::parse_value(exp.into_inner().next().unwrap()),
                Rule::starts_with => like_starting = true,
                Rule::value => value = Self::parse_value(exp).unwrap_or_default(),
                Rule::eq => eq = CompOp::from_str(exp.as_str()).unwrap(),
                Rule::field_name => name = exp.as_str(),
                _ => {}
            }
        }
        let s = self.choose_field(&name);
        Ok(OR(self
            .choose_field_vec(&name)
            .into_iter()
            .map(|sf| {
                if like_ending != like_starting {
                    let value = if like_ending {
                        format!("*{}", value)
                    } else {
                        format!("{}*", value)
                    };
                    LIKE(sf.clone(), value)
                } else if to_val.is_some() {
                    AND(vec![
                        VALUE(sf.clone(), CompOp::GTE, value.clone()),
                        VALUE(sf.clone(), CompOp::LTE, to_val.clone().unwrap_or_default()),
                    ])
                } else {
                    VALUE(sf.clone(), eq, value.clone())
                }
            })
            .collect()))
    }

    fn parse_from_to(expr: Pair<Rule>) -> (SQLTerm, String) {
        let mut name = String::new();
        let mut value = String::new();
        for exp in expr.into_inner() {
            println!("parse_from_to {exp:?}");
            match exp.as_rule() {
                _ => {}
            }
        }
        (name, value);
        todo!()
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
                _ => None,
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

    pub fn to_sql(&self) -> String {
        let sort = if self.sort.is_empty() {
            String::new()
        } else {
            format!(
                " SORT BY {}",
                self.sort
                    .iter()
                    .map(|sf| sf.to_sql())
                    .collect::<Vec<String>>()
                    .join(", ")
            )
        };
        format!(
            "{}{sort}",
            self.abstract_filter.to_sql().unwrap_or_default()
        )
    }
}

#[cfg(test)]
mod should {
    use super::*;
    use crate::op_tree::FType::{INTEGER, NUMERIC, VARCHAR};

    const ARTNR: SField = SField::new(
        "artikelnummer",
        VARCHAR(18),
        "READ_OFFER",
        &["art", "artnr", "artikelnummer", "artikelnr", "ano"],
    );
    const DESC: SField = SField::new(
        "positionstext",
        VARCHAR(512),
        "READ_OFFER",
        &["beschreibung", "decr", "description", "ptext"],
    );
    const PRICE: SField = SField::new("price", NUMERIC(12, 2), "READ_OFFER", &["preis", "price"]);
    const AGE: SField = SField::new("age", INTEGER(0, 150), "READ_OFFER", &["alter", "age"]);
    const NUMMER: SField = SField::new(
        "promille",
        INTEGER(1, 1000),
        "READ_OFFER",
        &["number", "nummer", "promille"],
    );

    #[test]
    fn parse_query() {
        let query =
            r#"ano=!23342 AND ^"irgend ein langer Text!" OR price='35,12'; artnr, ^nummer, age"#;
        let mut s = Suchbar {
            s_fields: vec![ARTNR, DESC, PRICE, NUMMER, AGE],
            ..Default::default()
        };
        println!("Query: {query}");
        let s = s.exec(query).expect("This should not panic!");
        println!("Debug: {s:#?}");
        println!("SQL: {}", s.to_sql());
    }
}

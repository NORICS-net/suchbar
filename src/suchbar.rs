use crate::comp_op::CompOp;
use crate::comp_op::CompOp::{Equal, NotEqual};
use crate::db_field::{DbField, SortField};
use crate::error::SuchError;
use crate::sql_term::SQLTerm;
use crate::sql_term::SQLTerm::{AND, DENIED, LIKE, NOT, OR, VALUE};
use permeable::Permeable;
use pest::iterators::Pair;
use pest::Parser;
use std::fmt::{Display, Write};
use std::ops::Not;
use std::str::FromStr;
use timewarp::Direction;

type SuchResult = Result<SQLTerm, SuchError>;

/// Static object to generate `WhereClause`s from queries.  
#[derive(Parser, Debug)]
#[grammar = "suchbar.pest"]
pub struct Suchbar {
    db_fields: &'static [DbField],
    options: SuchOptions,
}

/// Options for `Suchbar`.
#[derive(Default, Debug)]
pub struct SuchOptions {
    /// Should attempt to find a sequence of digits within a NUMERIC field?
    pub like_in_numerics: bool,
}

impl SuchOptions {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            like_in_numerics: false,
        }
    }
}

impl Suchbar {
    #[must_use]
    pub const fn new(db_fields: &'static [DbField]) -> Self {
        Self {
            db_fields,
            options: SuchOptions::new(),
        }
    }

    /// Returns an explanation which fields are usable for the search.
    /// Shows only fields the user has `permission` to see.
    pub fn explanation(&self, permission: &impl Permeable) -> String {
        let mut buf = String::new();
        for field in self.db_fields {
            if permission.has_perm(field.permission).is_ok() {
                writeln!(&mut buf, "{} {}", field.aliases(), field.db_type()).expect("");
            }
        }
        buf
    }

    /// Creates a `WhereClause` from the given `query` depending on th user's `permission`.
    ///
    /// # Errors
    /// Failures in `query` can cause a `SuchError`.
    pub fn exec(
        &self,
        permission: &impl Permeable,
        query: impl Into<String>,
    ) -> Result<WhereClause, SuchError> {
        let mut sql_term = AND(vec![]);
        let mut sort_field = vec![];
        let query = query.into();
        let qu = Self::parse(Rule::query, &query)?;
        for expr in qu {
            match expr.as_rule() {
                Rule::expr => sql_term = self.parse_expr(permission, expr)?,
                Rule::sort => sort_field = self.parse_sort(expr),
                _ => {} //ignore EOI and rest
            }
        }
        Ok(WhereClause {
            sql_term,
            sort_field,
        })
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
            self.db_fields.to_vec()
        }
    }

    /// expr = { atom ~ (bin_op? ~ atom)* }
    fn parse_expr(&self, perm: &impl Permeable, expr: Pair<Rule>) -> SuchResult {
        let mut acc = Vec::new();
        let mut or = false;
        let mut comp_op = CompOp::Equal;
        for exp in expr.into_inner() {
            //println!("** Suchbar::parse_expr:: {:?}", exp);
            match exp.as_rule() {
                Rule::field => {
                    if let Ok(field) = self.parse_field(perm, exp, comp_op) {
                        acc.push(field);
                    }
                }
                Rule::or => or = true,
                Rule::and => or = false,
                Rule::invert => comp_op = !comp_op,
                Rule::term => acc.push(self.parse_term(perm, None, comp_op, exp)),
                Rule::expr => acc.push(self.parse_expr(perm, exp)?),
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

    fn parse_field(&self, perm: &impl Permeable, expr: Pair<Rule>, not: CompOp) -> SuchResult {
        let mut name = "";
        let mut not = not == NotEqual;
        let mut comp_op = CompOp::default();
        for exp in expr.into_inner() {
            // println!("!!! Suchbar::parse_field:: {exp:?}");
            match exp.as_rule() {
                Rule::eq => comp_op = CompOp::from_str(exp.as_str()).unwrap_or_default(),
                Rule::field_name => name = exp.as_str(),
                Rule::invert => not = !not,
                Rule::term => {
                    return Ok(self.parse_term(
                        perm,
                        Some(name),
                        if not { comp_op.not() } else { comp_op },
                        exp,
                    ));
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

    fn parse_term(
        &self,
        perm: &impl Permeable,
        name: Option<&str>,
        comp_op: CompOp,
        expr: Pair<Rule>,
    ) -> SQLTerm {
        use Direction::{From, To};
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
                Rule::date => value = exp.as_str().to_string(),
                _ => println!("=> Suchbar::parse_term:: {exp:?}"),
            }
        }

        OR(self
            .choose_field_vec(name.unwrap_or_default())
            .into_iter()
            .map(|sf| {
                if perm.has_perm(sf.permission).is_err() {
                    DENIED
                } else if like_ending || like_starting {
                    let value = match (like_starting, like_ending) {
                        (true, false) => format!("*{value}"),
                        (false, true) => format!("{value}*"),
                        _ => format!("*{value}*"),
                    };
                    if comp_op == NotEqual {
                        NOT(Box::new(LIKE(sf, value)))
                    } else {
                        LIKE(sf, value)
                    }
                } else if name.is_none() {
                    // list of terms means LIKE-search for text-fields.
                    if sf.is_text() || self.options.like_in_numerics {
                        LIKE(sf, format!("*{value}*"))
                    } else {
                        VALUE(sf, Equal, From, value.clone())
                    }
                } else if to_val.is_some() {
                    AND(vec![
                        VALUE(sf.clone(), CompOp::Gte, From, value.clone()),
                        VALUE(sf, CompOp::Lt, To, to_val.clone().unwrap_or_default()),
                    ])
                } else if comp_op == NotEqual {
                    NOT(Box::new(VALUE(sf, Equal, From, value.clone())))
                } else {
                    VALUE(sf, comp_op, From, value.clone())
                }
            })
            .collect())
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
                Rule::field_name => {
                    if let Some(field) = self.choose_field(so.as_str()) {
                        sort_fields.push(SortField { desc, field });
                        desc = false;
                    }
                }
                _ => {}
            }
        }
        sort_fields
    }
}

/// The result of a query, ready to be inserted into a SELECT statement.  
#[derive(Debug)]
pub struct WhereClause {
    sql_term: SQLTerm,
    sort_field: Vec<SortField>,
}

impl WhereClause {
    /// Returns the part of a where-clause constructed from the user-query.
    /// Any Error will be ignored, then the returned String might be empty.
    ///
    /// Prefixes the return by `concatenate`, if parameter set, if empty omits.
    ///
    /// # Example
    /// ```rust
    /// use permeable::AllowAllPermission;
    /// use suchbar::*;
    /// use suchbar::DbType::TEXT;    
    ///
    /// const SUCHBAR: Suchbar = Suchbar::new(&[
    ///   DbField::new("surname", TEXT, "STD", &["surname", "sname", "sn"]),
    ///   DbField::new("givenname", TEXT, "STD", &["givenname", "name", "n"])
    /// ]);
    ///
    /// let exec = SUCHBAR.exec(&AllowAllPermission(), "sn=Don* AND n=Duck").unwrap();
    /// assert_eq!("( surname LIKE 'Don%' AND givenname='Duck' )", exec.where_clause().unwrap());
    /// assert_eq!(" WHERE ( surname LIKE 'Don%' AND givenname='Duck' )", exec.to_sql("WHERE"));
    ///
    /// let exec = SUCHBAR.exec(&AllowAllPermission(), "sn=Don*;givenname, age ^sname").unwrap();
    /// assert_eq!("surname LIKE 'Don%'", exec.where_clause().unwrap());
    /// assert_eq!("givenname, surname DESC", exec.order_by());
    /// assert_eq!(" WHERE surname LIKE 'Don%' ORDER BY givenname, surname DESC", exec.to_sql("WHERE"));
    /// ```
    pub fn to_sql(&self, concatenate: impl Display) -> String {
        let whr = self.where_clause().unwrap_or_default();
        let whr = if whr.is_empty() {
            whr
        } else {
            format!(" {concatenate} {whr}")
        };
        let sort = if self.sort_field.is_empty() {
            String::new()
        } else {
            format!(" ORDER BY {}", self.order_by())
        };
        format!("{whr}{sort}")
    }

    /// Returns the WHERE-clause as SQL.
    ///
    /// # Errors
    /// Failures in `query` can cause a `SuchError`.
    pub fn where_clause(&self) -> Result<String, SuchError> {
        self.sql_term.to_sql()
    }

    /// Returns the SQL `ORDER BY` part.
    ///
    pub fn order_by(&self) -> String {
        self.sort_field
            .iter()
            .map(SortField::to_sql)
            .collect::<Vec<String>>()
            .join(", ")
    }
}

#[cfg(test)]
mod should {
    use super::Suchbar;
    use crate::db_field::DbField;
    use crate::db_field::DbType::{INTEGER, NUMERIC, TEXT, VARCHAR};
    use crate::suchbar::SuchOptions;
    use crate::DbType::DATE;
    use permeable::{Permeable, PermissionError};

    const SUCHBAR: Suchbar = Suchbar::new(&[
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
        DbField::new(
            "price",
            NUMERIC(12, 2),
            "READ_OFFER",
            &["preis", "price", "p"],
        ),
        DbField::new("age", INTEGER(0, 150), "ACCESS_PRIVATE", &["alter", "age"]),
        DbField::new(
            "promille",
            INTEGER(1, 1000),
            "ACCESS_PRIVATE",
            &["number", "nummer", "promille"],
        ),
        DbField::new("changed", DATE, "READ_OFFER", &["changed", "ch"]),
    ]);

    struct Perm {
        perms: [&'static str; 2],
    }

    impl Permeable for Perm {
        fn has_perm(&self, permission: &str) -> Result<(), PermissionError> {
            if self.perms.iter().any(|&str| str == permission) {
                Ok(())
            } else {
                Err(PermissionError::denied(permission, "user"))
            }
        }
    }

    const ADMIN: Perm = Perm {
        perms: ["ACCESS_PRIVATE", "READ_OFFER"],
    };

    const USER: Perm = Perm {
        perms: ["ANYTHING_ELSE", "READ_OFFER"],
    };

    #[test]
    fn give_permission() {
        assert_eq!(ADMIN.has_perm("READ_OFFER").is_ok(), true);
        assert_eq!(ADMIN.has_perm("ACCESS_PRIVATE").is_ok(), true);
        assert_eq!(USER.has_perm("READ_OFFER").is_ok(), true);
        assert_eq!(USER.has_perm("ACCESS_PRIVATE").is_ok(), false);
    }

    #[test]
    fn parse_not_equal_as_admin() {
        let s = SUCHBAR
            .exec(&ADMIN, "age!=123")
            .expect("This should not panic!");
        assert_eq!("  NOT age=123", s.to_sql(""));
        let s = SUCHBAR
            .exec(&ADMIN, "NOT age=123")
            .expect("This should not panic!");
        assert_eq!("  NOT age=123", s.to_sql(""));
        let s = SUCHBAR
            .exec(&ADMIN, "ptext!=A")
            .expect("This should not panic!");
        assert_eq!("  NOT positionstext='A'", s.to_sql(""));

        let s = SUCHBAR
            .exec(&ADMIN, " ptext != AAA*")
            .expect("This should not panic!");
        assert_eq!("  NOT positionstext LIKE 'AAA%'", s.to_sql(""));

        let s = SUCHBAR
            .exec(&ADMIN, "NOT ptext == AAA*")
            .expect("This should not panic!");
        assert_eq!("  NOT positionstext LIKE 'AAA%'", s.to_sql(""));

        let s = SUCHBAR
            .exec(&ADMIN, "NOT ptext != AAA*")
            .expect("This should not panic!");
        assert_eq!("  positionstext LIKE 'AAA%'", s.to_sql(""));
    }

    #[test]
    fn parse_and_concat() {
        let s = SUCHBAR
            .exec(&ADMIN, "age=123 AND ptext=AAA")
            .expect("This should not panic!");
        assert_eq!(
            "( age=123 AND positionstext='AAA' )",
            s.where_clause().unwrap_or_default()
        );
        let s = SUCHBAR
            .exec(&ADMIN, "art=123* AND ptext=AAA")
            .expect("This should not panic!");
        assert_eq!(
            "( artikelnummer LIKE '123%' AND positionstext='AAA' )",
            s.where_clause().unwrap_or_default()
        );
    }

    #[test]
    fn parse_not_equal_as_user() {
        let s = SUCHBAR
            .exec(&USER, "age!=123")
            .expect("This should not panic!");
        assert_eq!("", s.to_sql(""));
        let s = SUCHBAR
            .exec(&USER, "NOT age=123")
            .expect("This should not panic!");
        assert_eq!("", s.to_sql(""));
        let s = SUCHBAR
            .exec(&USER, "ptext!=A")
            .expect("This should not panic!");
        assert_eq!("  NOT positionstext='A'", s.to_sql(""));

        let s = SUCHBAR
            .exec(&USER, " ptext != AAA*")
            .expect("This should not panic!");
        assert_eq!("  NOT positionstext LIKE 'AAA%'", s.to_sql(""));

        let s = SUCHBAR
            .exec(&USER, "NOT ptext == AAA*")
            .expect("This should not panic!");
        assert_eq!("  NOT positionstext LIKE 'AAA%'", s.to_sql(""));

        let s = SUCHBAR
            .exec(&USER, "NOT ptext != AAA*")
            .expect("This should not panic!");
        assert_eq!("  positionstext LIKE 'AAA%'", s.to_sql(""));
    }

    #[test]
    fn parse_integer_query_std() {
        let s = SUCHBAR.exec(&ADMIN, "123").expect("This should not panic!");
        assert_eq!(
            "  ( artikelnummer LIKE '%123%' OR positionstext LIKE '%123%' OR \
            price=123 OR age=123 OR promille=123 )",
            s.to_sql("")
        );
        let s = SUCHBAR
            .exec(&ADMIN, "1234")
            .expect("This should not panic!");
        assert_eq!(
            "  ( artikelnummer LIKE '%1234%' OR positionstext LIKE '%1234%' \
            OR price=1234 )",
            s.to_sql("")
        );
    }

    #[test]
    fn parse_integer_query_like() {
        let likebar = Suchbar {
            options: SuchOptions {
                like_in_numerics: true,
            },
            db_fields: SUCHBAR.db_fields,
        };
        let s = likebar.exec(&ADMIN, "123").expect("This should not panic!");
        assert_eq!(
            "  ( artikelnummer LIKE '%123%' OR positionstext LIKE '%123%' OR price::TEXT LIKE '%123%' \
            OR age::TEXT LIKE '%123%' OR promille::TEXT LIKE '%123%' )",
            s.to_sql("")
        );
        let s = likebar
            .exec(&ADMIN, "1234")
            .expect("This should not panic!");
        assert_eq!(
            "  ( artikelnummer LIKE '%1234%' OR positionstext LIKE '%1234%' OR \
            price::TEXT LIKE '%1234%' )",
            s.to_sql("")
        );
    }

    #[test]
    fn parse_like_query() {
        let s = SUCHBAR
            .exec(&ADMIN, "art='2332*'")
            .expect("This should not panic!");
        assert_eq!("  artikelnummer LIKE '2332%'", s.to_sql(""));
        let s = SUCHBAR
            .exec(&ADMIN, "art=2332*")
            .expect("This should not panic!");
        assert_eq!("  artikelnummer LIKE '2332%'", s.to_sql(""));
        let s = SUCHBAR
            .exec(&ADMIN, "art=2332$")
            .expect("This should not panic!");
        assert_eq!("  artikelnummer LIKE '%2332'", s.to_sql(""));
        let s = SUCHBAR
            .exec(&ADMIN, "art='*2332*'")
            .expect("This should not panic!");
        assert_eq!("  artikelnummer LIKE '%2332%'", s.to_sql(""));
        let s = SUCHBAR
            .exec(&ADMIN, "art=^'2332'")
            .expect("This should not panic!");
        assert_eq!("  artikelnummer LIKE '2332%'", s.to_sql(""));
    }

    #[test]
    fn parse_misc_query() {
        let query = r#"ano!=23342 AND (desc=^"irgend ein langer Text!" OR price='35,12'); artnr, ^nummer, age"#;
        let s = SUCHBAR.exec(&ADMIN, query).expect("This should not panic!");
        assert_eq!(
            "  ( NOT artikelnummer='23342' AND ( positionstext LIKE 'irgend ein langer Text!%' \
            OR price=35.12 ) ) ORDER BY artikelnummer, promille DESC, age",
            s.to_sql("")
        );
    }

    #[test]
    fn parse_from_to_values() {
        let s = SUCHBAR
            .exec(&ADMIN, "age=10-19")
            .expect("This should not panic!");
        assert_eq!("  ( age>=10 AND age<19 )", s.to_sql(""));
    }

    #[test]
    fn parse_like_somewhere() {
        let s = SUCHBAR
            .exec(&ADMIN, "*Superman*")
            .expect("This should not panic!");
        assert_eq!(
            " WHERE ( artikelnummer LIKE '%Superman%' OR positionstext LIKE '%Superman%' )",
            s.to_sql("WHERE")
        );

        let s = SUCHBAR
            .exec(&ADMIN, "Superman Batman")
            .expect("This should not panic!");
        assert_eq!(
            " WHERE ( ( artikelnummer LIKE '%Superman%' OR positionstext LIKE '%Superman%' ) AND \
            ( artikelnummer LIKE '%Batman%' OR positionstext LIKE '%Batman%' ) )",
            s.to_sql("WHERE")
        );
        // age = *5*
        let s = SUCHBAR
            .exec(&ADMIN, "artnr = *5*")
            .expect("This should not panic!");
        assert_eq!(" WHERE artikelnummer LIKE '%5%'", s.to_sql("WHERE"));
    }

    #[test]
    fn parse_iso_dates() {
        let s = SUCHBAR
            .exec(&ADMIN, "ch=2022-12-24")
            .expect("This should not panic!");
        assert_eq!(" WHERE changed='2022-12-24'", s.to_sql("WHERE"));

        let s = SUCHBAR
            .exec(&ADMIN, r#"ch="2022-12-24""#)
            .expect("This should not panic!");
        assert_eq!(" WHERE changed='2022-12-24'", s.to_sql("WHERE"));
    }

    #[test]
    fn parse_natural_language_dates() {
        let s = SUCHBAR
            .exec(&ADMIN, "ch=Jan")
            .expect("This should not panic!");
        assert_eq!(" WHERE changed='2023-01-01'", s.to_sql("WHERE"));

        let s = SUCHBAR
            .exec(&ADMIN, r#"ch=24.12.2022"#)
            .expect("This should not panic!");
        assert_eq!(" WHERE changed='2022-12-24'", s.to_sql("WHERE"));
        let s = SUCHBAR
            .exec(&ADMIN, r#"ch='Feb'-'Dez'"#)
            .expect("This should not panic!");
        assert_eq!(
            " WHERE ( changed>='2023-02-01' AND changed<'2024-01-01' )",
            s.to_sql("WHERE")
        );
        let s = SUCHBAR
            .exec(&ADMIN, r#"ch=Feb-Dez"#)
            .expect("This should not panic!");
        assert_eq!(
            " WHERE ( changed>='2023-02-01' AND changed<'2024-01-01' )",
            s.to_sql("WHERE")
        );
    }

    #[test]
    fn list_sort_by_fields() {
        let s = SUCHBAR
            .exec(&ADMIN, ";age, art")
            .expect("This should not panic!");
        assert_eq!(" ORDER BY age, artikelnummer", s.to_sql("WHERE"));
        let s = SUCHBAR
            .exec(&ADMIN, ";art, ^p, ch")
            .expect("This should not panic!");
        assert_eq!(
            " ORDER BY artikelnummer, price DESC, changed",
            s.to_sql("WHERE")
        );
    }
}

use self::DbType::*;
use super::comp_op::CompOp;
use crate::error::SuchError;
use crate::error::SuchError::ParseError;

fn try_bool(str: &str) -> Result<bool, SuchError> {
    let str = str.trim().to_ascii_lowercase();
    match str.as_str() {
        "1" | "true" | "wahr" => Ok(true),
        "0" | "false" | "falsch" | "unwahr" => Ok(false),
        _ => Err(ParseError(format!("No boolean value: '{str}'"))),
    }
}

#[derive(Debug, Clone)]
pub struct DbField {
    pub db_name: &'static str,
    pub db_type: DbType,
    pub permission: &'static str,
    pub alias: &'static [&'static str],
}

impl DbField {
    /// !important: use lowercase
    pub const fn new(
        db_name: &'static str,
        db_type: DbType,
        permission: &'static str,
        alias: &'static [&'static str],
    ) -> Self {
        Self {
            db_name,
            db_type,
            permission,
            alias,
        }
    }

    /// Transforms the given `val` into a EQ-expression. Replaces symbols into a sql-save query.
    pub(crate) fn try_sql_eq(&self, eq: &CompOp, val: &str) -> Result<String, SuchError> {
        use std::str::FromStr;
        let Self {
            db_name, db_type, ..
        } = self;
        let val_sql = val.replace('\'', "''");

        match db_type {
            VARCHAR(a) if *a >= val.len() => Ok(format!("{db_name}{eq}'{val_sql}'")),
            TEXT => Ok(format!("{db_name}{eq}'{val_sql}'")),
            BOOL if eq == &CompOp::Equal => Ok(format!(
                "{db_name}{}",
                if try_bool(val)? { "" } else { "=false" }
            )),
            DATE | TIMESTAMP => Ok(format!("{db_name}{eq}'{val_sql}'")),
            INTEGER(min, max) => {
                let cval = val.replace(',', ".");
                match u64::from_str(&cval) {
                    Ok(d) if d <= *max && d >= *min => Ok(format!("{db_name}{eq}{cval}")),
                    _ => Err(ParseError(format!("No Integer value '{val}'"))),
                }
            }
            NUMERIC(len, _) => {
                let cval = val.replace(',', ".");
                match f64::from_str(&cval) {
                    Ok(_) if cval.len() < (len + 1) as usize => Ok(format!("{db_name}{eq}{cval}")),
                    _ => Err(ParseError(format!("No Numeric value '{val}'"))),
                }
            }
            _ => Err(ParseError(format!(
                "Don't know how to handle: {db_type:?} = '{val}'"
            ))),
        }
    }

    /// Transforms the given `val` into a LIKE-expression. Replaces key-symbols from glob-style to
    /// form a sql-save query.
    pub(crate) fn try_sql_like(&self, val: &str) -> Result<String, SuchError> {
        let Self {
            db_name, db_type, ..
        } = self;
        let escaper = |c: char| match c {
            '?' => String::from("_"),
            '*' => String::from("%"),
            '\'' => String::from("''"),
            '_' | '%' => format!("\\{c}"),
            _ => String::from(c),
        };
        match db_type {
            VARCHAR(a) if val.len() <= *a => Ok(format!(
                "{db_name} LIKE '{}'",
                val.chars().map(escaper).collect::<String>()
            )),
            TEXT => Ok(format!(
                "{db_name} LIKE '{}'",
                val.chars().map(escaper).collect::<String>()
            )),
            DATE if val.len() <= 10 => Ok(format!(
                "{db_name} LIKE '{}'",
                // pre-checking dates is to cumbersome, let the db make its job.
                val.chars().map(escaper).collect::<String>()
            )),
            _ => Err(ParseError(format!(
                "{db_name} ({db_type:?})'{val}' is not compatible"
            ))),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum DbType {
    VARCHAR(usize),
    TEXT,
    INTEGER(u64, u64),
    NUMERIC(u32, u32),
    BOOL,
    DATE,
    TIMESTAMP,
}

#[derive(Debug)]
pub(crate) struct SortField {
    pub(crate) desc: bool,
    pub(crate) field: DbField,
}

impl SortField {
    pub fn to_sql(&self) -> String {
        let name = self.field.db_name;
        match self.desc {
            true => format!("{name} DESC"),
            false => name.to_string(),
        }
    }
}

#[cfg(test)]
mod should {
    use crate::comp_op::CompOp;
    use crate::db_field::DbField;
    use crate::db_field::DbType::{BOOL, DATE, INTEGER, VARCHAR};
    use crate::sql_term::SQLTerm::*;

    const ARTIKEL: DbField = DbField::new(
        "article",
        VARCHAR(200),
        "READ_OFFER",
        &["artnr", "artikelnr"],
    );
    const ACTIVE: DbField = DbField::new("aktiv", BOOL, "READ_OFFER", &["akt"]);
    const END_DATE: DbField =
        DbField::new("end_date", DATE, "READ_OFFER", &["enddate", "end_date"]);
    const NAME: DbField = DbField::new("ma_active", VARCHAR(32), "READ_OFFER", &["akt"]);
    const PRICE: DbField = DbField::new("price", INTEGER(0, 2000), "READ_OFFER_PRICE", &["price"]);

    #[test]
    fn op_to_sql() {
        let df = AND(vec![
            VALUE(ARTIKEL, CompOp::Gt, "1245667".into()),
            OR(vec![
                NOT(Box::new(VALUE(ACTIVE, CompOp::Equal, "false".into()))),
                VALUE(END_DATE, CompOp::Lte, "2022-12-24".into()),
                LIKE(NAME, "Micha's cat*".into()),
            ]),
        ]);
        assert_eq!(
            df.to_sql().unwrap(),
            "( article>'1245667' AND ( NOT aktiv=false OR end_date<=\
            '2022-12-24' OR ma_active LIKE 'Micha''s cat%' ) )"
        );

        let df = VALUE(PRICE, CompOp::Equal, "1000.0".into());
        assert_eq!(df.to_sql().unwrap_or_default(), "");
        let df = VALUE(PRICE, CompOp::Equal, "1000".into());
        assert_eq!(df.to_sql().unwrap_or_default(), "price=1000");
    }
}

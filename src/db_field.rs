use self::DbType::*;
use super::comp_op::CompOp;
use crate::error::SuchError;
use crate::error::SuchError::ParseError;
use timewarp::*;

fn try_bool(str: &str) -> Result<bool, SuchError> {
    let str = str.trim().to_ascii_lowercase();
    match str.as_str() {
        "1" | "true" | "wahr" => Ok(true),
        "0" | "false" | "falsch" | "unwahr" => Ok(false),
        _ => Err(ParseError(format!("No boolean value: '{str}'"))),
    }
}

fn timestamp_checker(str: String) -> Result<String, SuchError> {
    if str.chars().any(|a| match a {
        '-' | ':' | ' ' | '%' => false,
        _ => !a.is_ascii_digit(),
    }) {
        Err(ParseError("No date".to_string()))
    } else if str.len() == 10 {
        Ok(format!("{str} 00:00:00"))
    } else {
        Ok(str)
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
    pub(crate) fn try_sql_eq(
        &self,
        eq: &CompOp,
        val: &str,
        d: Direction,
    ) -> Result<String, SuchError> {
        let Self {
            db_name, db_type, ..
        } = self;

        match db_type {
            BOOL => Ok(format!(
                "{db_name}{}",
                if try_bool(val)? == (*eq == CompOp::Equal) {
                    ""
                } else {
                    "=false"
                }
            )),
            NUMERIC(_, _) | INTEGER(_, _) => Ok(format!("{db_name}{eq}{}", db_type.sql_safe(val)?)),
            DATE => Ok(format!(
                "{db_name}{eq}'{:#}'",
                date_matcher(Doy::today(), d, val).map(|ds| ds.start())?
            )),
            _ => Ok(format!("{db_name}{eq}'{}'", db_type.sql_safe(val)?)),
        }
    }

    /// Transforms the given `val` into a LIKE-expression. Replaces key-symbols from glob-style to
    /// form a sql-save query.
    pub(crate) fn try_sql_like(&self, val: &str) -> Result<String, SuchError> {
        let Self {
            db_name, db_type, ..
        } = self;
        match db_type {
            VARCHAR(_) => Ok(format!("{db_name} LIKE '{}'", db_type.sql_safe(val)?)),
            TEXT => Ok(format!("{db_name} LIKE '{}'", db_type.sql_safe(val)?)),
            DATE | TIMESTAMP => Err(SuchError::LikeNotPossible),
            _ => Ok(format!("{db_name}::TEXT LIKE '{}'", db_type.sql_safe(val)?)),
        }
    }

    pub fn is_text(&self) -> bool {
        matches!(self.db_type, TEXT | VARCHAR(_))
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

impl DbType {
    pub fn sql_safe(&self, val: &str) -> Result<String, SuchError> {
        let escaper = |c: char| match c {
            '?' => String::from("_"),
            '*' => String::from("%"),
            '\'' => String::from("''"),
            '_' | '%' => format!("\\{c}"),
            _ => String::from(c),
        };
        self.checker(val.chars().map(escaper).collect::<String>())
    }

    fn checker(&self, val: String) -> Result<String, SuchError> {
        use std::str::FromStr;
        match self {
            VARCHAR(a) if val.len() > *a => Err(ParseError(format!("Value: '{val}' to long"))),
            VARCHAR(_) | TEXT => Ok(val),
            TIMESTAMP => timestamp_checker(val),
            INTEGER(min, max) => {
                let cval = val.replace(',', ".");
                match u64::from_str(&cval.replace('%', "")) {
                    Ok(d) if d <= *max && d >= *min => Ok(cval),
                    _ => Err(ParseError(format!("No Integer value '{val}'"))),
                }
            }
            NUMERIC(len, _) => {
                let cval = val.replace(',', ".");
                let number = cval.replace('%', "");
                match f64::from_str(&number) {
                    Ok(_) if number.len() < (len + 1) as usize => Ok(cval),
                    _ => Err(ParseError(format!("No Numeric value '{val}'"))),
                }
            }
            _ => Err(ParseError(format!(
                "Don't know how to handle: {self:?} = '{val}'"
            ))),
        }
    }
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
    use crate::DbType::TIMESTAMP;
    use timewarp::Direction::From;

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
    const CHANGED: DbField =
        DbField::new("changed", TIMESTAMP, "READ_OFFER", &["changed", "updated"]);

    #[test]
    fn op_to_sql() {
        let df = AND(vec![
            VALUE(ARTIKEL, CompOp::Gt, From, "1245667".into()),
            OR(vec![
                NOT(Box::new(VALUE(ACTIVE, CompOp::Equal, From, "false".into()))),
                VALUE(END_DATE, CompOp::Lte, From, "2022-12-24".into()),
                LIKE(NAME, "Micha's cat*".into()),
            ]),
        ]);
        assert_eq!(
            df.to_sql().unwrap(),
            "( article>'1245667' AND ( NOT aktiv=false OR end_date<=\
            '2022-12-24' OR ma_active LIKE 'Micha''s cat%' ) )"
        );

        let df = VALUE(PRICE, CompOp::Equal, From, "1000.0".into());
        assert!(df.to_sql().is_err());
        let df = VALUE(PRICE, CompOp::Equal, From, "1000".into());
        assert_eq!(df.to_sql().unwrap_or_default(), "price=1000");

        let df = VALUE(CHANGED, CompOp::Gte, From, "2022-09-01".into());
        assert_eq!(
            df.to_sql().unwrap_or_default(),
            "changed>='2022-09-01 00:00:00'"
        );
        let df = VALUE(CHANGED, CompOp::Lt, From, "2022-09-01 23:30:00".into());
        assert_eq!(
            df.to_sql().unwrap_or_default(),
            "changed<'2022-09-01 23:30:00'"
        );

        let df = VALUE(CHANGED, CompOp::Lt, From, "2022-09-01 23:30:00 MEZ".into());
        assert!(df.to_sql().is_err());
    }
}

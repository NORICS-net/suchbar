use self::DbType::{BOOL, DATE, INTEGER, NUMERIC, TEXT, TIMESTAMP, VARCHAR};
use crate::comp_op::CompOp;
use crate::error::SuchError;
use crate::error::SuchError::ParseError;
use timewarp::{date_matcher, Direction, Doy};

fn try_bool(str: &str) -> Result<bool, SuchError> {
    let str = str.trim().to_ascii_lowercase();
    match str.as_str() {
        "1" | "true" | "wahr" | "yes" | "ja" | "y" | "j" | "t" | "w" => Ok(true),
        "0" | "false" | "falsch" | "unwahr" | "no" | "not" | "nein" | "n" | "f" => Ok(false),
        _ => Err(ParseError(format!("No boolean value: '{str}'"))),
    }
}

fn timestamp_checker(str: String) -> Result<String, SuchError> {
    if str.chars().any(|a| match a {
        '-' | ':' | ' ' | '%' => false,
        _ => !a.is_ascii_digit(),
    }) {
        Err(ParseError(String::from("No date")))
    } else if str.len() == 10 {
        Ok(str + " 00:00:00")
    } else {
        Ok(str)
    }
}

/// Definition of a Database-Field.   
#[derive(Debug, Clone)]
pub struct DbField {
    pub db_name: &'static str,
    pub db_type: DbType,
    pub permission: &'static str,
    pub alias: &'static [&'static str],
}

impl DbField {
    /// !important: use lowercase
    #[must_use]
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
    ///
    /// # Errors
    /// May fail if `val` can't be parsed to the needed type.
    pub(crate) fn try_sql_eq(
        &self,
        eq: CompOp,
        val: &str,
        d: Direction,
    ) -> Result<String, SuchError> {
        let Self {
            db_name, db_type, ..
        } = self;

        match db_type {
            BOOL => {
                let not = try_bool(val)? == (eq == CompOp::Equal);
                Ok(format!("{db_name}{}", if not { "" } else { "=false" }))
            }
            NUMERIC(_, _) | INTEGER(_, _) => Ok(format!("{db_name}{eq}{}", db_type.sql_safe(val)?)),
            DATE => {
                let date = date_matcher(Doy::today(), d, val).map(|d| d.start())?;
                Ok(format!("{db_name}{eq}'{date:#}'"))
            }
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
            VARCHAR(_) | TEXT => Ok(format!("{db_name} LIKE '{}'", db_type.sql_safe(val)?)),
            DATE | TIMESTAMP => Err(SuchError::LikeNotPossible),
            _ => Ok(format!("{db_name}::TEXT LIKE '{}'", db_type.sql_safe(val)?)),
        }
    }

    #[must_use]
    pub fn is_text(&self) -> bool {
        matches!(self.db_type, TEXT | VARCHAR(_))
    }

    /// Returns all aliases by which this `DbField` can be used.
    #[must_use]
    pub fn aliases(&self) -> String {
        format!("[{}]", self.alias.join(", "))
    }

    /// Returns a simplified Type.
    #[must_use]
    pub fn db_type(&self) -> String {
        self.db_type.name()
    }
}

/// Type of the field in the database.
/// It is used to decide in which columns valid hits could be found and how
/// the query must be set up.
#[derive(Debug, Copy, Clone)]
pub enum DbType {
    /// Text in a max. length.
    VARCHAR(usize),
    /// Text in any length.
    TEXT,
    /// The min / max-value this INTEGER is valid.
    INTEGER(u64, u64),
    /// The precision of a numeric is the total count of significant digits
    /// in the whole number, that is, the number of digits to both sides of
    /// the decimal point. The scale of a numeric is the count of decimal
    /// digits in the fractional part, to the right of the decimal point.
    /// So the number 23.5141 has a precision of 6 and a scale of 4.
    NUMERIC(u32, u32),
    /// Interprets:
    /// * True: `1`, `true`, `wahr`, `yes`, `ja`, `y`, `j`, `t`, `w`
    /// * False: `0`, `false`, `falsch`, `unwahr`, `no`, `not`, `nein`, `n`, `f`
    BOOL,
    /// Date without time
    DATE,
    /// Date with time
    TIMESTAMP,
}

impl DbType {
    fn sql_safe(&self, val: &str) -> Result<String, SuchError> {
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
                let c_val = val.replace(',', ".");
                match u64::from_str(&c_val.replace('%', "")) {
                    Ok(d) if d <= *max && d >= *min => Ok(c_val),
                    _ => Err(ParseError(format!("No Integer value '{val}'"))),
                }
            }
            NUMERIC(len, _) => {
                let c_val = val.replace(',', ".");
                let number = c_val.replace('%', "");
                match f64::from_str(&number) {
                    Ok(_) if number.len() < (len + 1) as usize => Ok(c_val),
                    _ => Err(ParseError(format!("No Numeric value '{val}'"))),
                }
            }
            _ => Err(ParseError(format!(
                "Don't know how to handle: {self:?} = '{val}'"
            ))),
        }
    }

    #[must_use]
    pub fn name(&self) -> String {
        match self {
            VARCHAR(_) | TEXT => "TEXT",
            INTEGER(_, _) | NUMERIC(_, _) => "NUMBER",
            BOOL => "BOOL",
            DATE | TIMESTAMP => "TIME",
        }
        .into()
    }
}

#[derive(Debug)]
pub struct SortField {
    pub desc: bool,
    pub field: DbField,
}

impl SortField {
    pub fn to_sql(&self) -> String {
        format!(
            "{}{}",
            self.field.db_name,
            if self.desc { " DESC" } else { "" }
        )
    }
}

#[cfg(test)]
mod should {
    use crate::comp_op::CompOp;
    use crate::db_field::DbField;
    use crate::db_field::DbType::{BOOL, DATE, INTEGER, VARCHAR};
    use crate::sql_term::SQLTerm::{AND, LIKE, NOT, OR, VALUE};
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

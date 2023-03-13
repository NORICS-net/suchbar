use anyhow::bail;
use std::fmt::{Display, Error, Formatter};
use std::ops::Not;
use std::str::FromStr;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum CompOp {
    Equal,
    NotEqual,
    GT,
    LT,
    GTE,
    LTE,
}

impl Default for CompOp {
    fn default() -> Self {
        CompOp::Equal
    }
}

impl Display for CompOp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use CompOp::*;
        write!(
            f,
            "{}",
            match self {
                Equal => "=",
                GT => ">",
                GTE => ">=",
                LT => "<",
                LTE => "<=",
                NotEqual => return Err(Error::default()),
            }
        )
    }
}

impl FromStr for CompOp {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "=" => Ok(CompOp::Equal),
            ">=" => Ok(CompOp::GTE),
            ">" => Ok(CompOp::GT),
            "<=" => Ok(CompOp::LTE),
            "<" => Ok(CompOp::LT),
            "!=" => Ok(CompOp::NotEqual),
            _ => Err(()),
        }
    }
}

impl Not for CompOp {
    type Output = CompOp;

    fn not(self) -> Self::Output {
        use CompOp::*;
        match &self {
            Equal => NotEqual,
            NotEqual => Equal,
            GT => LTE,
            GTE => LT,
            LTE => GT,
            LT => GTE,
        }
    }
}

fn try_bool(str: &str) -> anyhow::Result<bool> {
    let str = str.trim().to_ascii_lowercase();
    match str.as_str() {
        "1" | "true" | "wahr" => Ok(true),
        "0" | "false" | "falsch" | "unwahr" => Ok(false),
        _ => bail!("No boolean value: '{str}'"),
    }
}

#[derive(Debug, Copy, Clone)]
pub enum FType {
    VARCHAR(usize),
    INTEGER(u64, u64),
    NUMERIC(u32, u32),
    BOOL,
    DATE,
    TIMESTAMP,
}

#[derive(Debug, Clone)]
pub struct SField {
    pub db_name: &'static str,
    pub db_type: FType,
    pub permission: &'static str,
    pub alias: &'static [&'static str],
}

impl SField {
    pub(crate) const CATCHALL: Self = SField::new("*", FType::VARCHAR(25000), "", &[]);

    /// !important: use lowercase
    pub const fn new(
        db_name: &'static str,
        db_type: FType,
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
    pub(crate) fn try_sql_eq(&self, eq: CompOp, val: &str) -> anyhow::Result<String> {
        use FType::*;
        let Self {
            db_name, db_type, ..
        } = self;
        match db_type {
            VARCHAR(a) if *a > val.len() => {
                Ok(format!("{db_name}{eq}'{}'", val.replace('\'', "''")))
            }
            BOOL if eq == CompOp::Equal => Ok(format!(
                "{}{db_name}",
                if try_bool(val)? { "" } else { "NOT " }
            )),
            DATE | TIMESTAMP => Ok(format!("{db_name}{eq}'{}'", val.replace('\'', "''"))),
            INTEGER(min, max) => {
                let cval = val.replace(',', ".");
                match u64::from_str(&cval) {
                    Ok(d) if d <= *max && d >= *min => Ok(format!("{db_name}{eq}{cval}")),
                    _ => {
                        //  println!("No Integer value '{val}'");
                        bail!("No Integer value '{val}'");
                    }
                }
            }
            NUMERIC(len, _) => {
                let cval = val.replace(',', ".");
                match f64::from_str(&cval) {
                    Ok(_) if cval.len() < (*len + 1) as usize => Ok(format!("{db_name}{eq}{cval}")),
                    _ => {
                        //  println!("No numeric value '{val}'");
                        bail!("No numeric value '{val}'");
                    }
                }
            }
            _ => bail!("Don't know how to handle: {db_type:?} = '{val}'"),
        }
    }

    /// Transforms the given `val` into a LIKE-expression. Replaces key-symbols from glob-style to
    /// form a sql-save query.
    pub(crate) fn try_sql_like(&self, val: &str) -> anyhow::Result<String> {
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
            FType::VARCHAR(a) if val.len() <= *a => Ok(format!(
                "{db_name} IS LIKE '%{}%'",
                val.chars().map(escaper).collect::<String>()
            )),
            FType::DATE if val.len() <= 10 => Ok(format!(
                "{db_name} IS LIKE '%{}%'",
                // pre-checking dates is to cumbersome, let the db make its job.
                val.chars().map(escaper).collect::<String>()
            )),

            _ => bail!("{db_name} ({db_type:?})'{val}' is not compatible "),
        }
    }
}

#[cfg(test)]
mod should {

    use crate::op_tree::FType::{BOOL, DATE, VARCHAR};
    use crate::op_tree::{CompOp, FType, SField};
    use crate::sql_term::SQLTerm::*;

    const ARTIKEL: SField = SField::new(
        "article",
        VARCHAR(200),
        "READ_OFFER",
        &["artnr", "artikelnr"],
    );
    const ACTIVE: SField = SField::new("aktiv", BOOL, "READ_OFFER", &["akt"]);
    const END_DATE: SField = SField::new("end_date", DATE, "READ_OFFER", &["enddate", "end_date"]);
    const NAME: SField = SField::new("ma_active", VARCHAR(32), "READ_OFFER", &["akt"]);
    const PRICE: SField = SField::new(
        "price",
        FType::INTEGER(0, 2000),
        "READ_OFFER_PRICE",
        &["price"],
    );

    #[test]
    fn op_to_sql() {
        let df = AND(vec![
            VALUE(ARTIKEL, CompOp::GT, "1245667".into()),
            OR(vec![
                NOT(Box::new(VALUE(ACTIVE, CompOp::Equal, "false".into()))),
                VALUE(END_DATE, CompOp::LTE, "2022-12-24".into()),
                LIKE(NAME, "Micha's cat".into()),
            ]),
        ]);
        assert_eq!(
            df.to_sql().unwrap(),
            "( article>'1245667' AND ( NOT ( NOT aktiv ) OR end_date<=\
            '2022-12-24' OR ma_active IS LIKE '%Micha''s cat%' ) )"
        );

        let df = VALUE(PRICE, CompOp::Equal, "1000.0".into());
        assert_eq!(df.to_sql().unwrap_or_default(), "");
        let df = VALUE(PRICE, CompOp::Equal, "1000".into());
        assert_eq!(df.to_sql().unwrap_or_default(), "price=1000");
    }
}

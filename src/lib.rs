//! # Enduser-learnable language for database queries.
//!
//! A query language that can be learned by normal users, similar to an internet
//! search engine. To empower users to filter and sort data in a kind of natural
//! language while preserving permissions.     
//!
//! The search query entered by the user is parsed and converted into an SQL WHERE
//! clause. For the best possible user experience, field-free queries are sent to
//! all syntactically eligible fields as similarity (LIKE) searches.   
//!
//! ## Example
//!
//! A query like `"plz=26440-26452 OR (Eisen AND sn!=Hammer*); plz"` could generate a
//! WHERE-Clause for such kind of SELECT.
//!
//! ```sql
//! SELECT
//!     pa.shortname, pa.description, pa.taxnumber, pb.longname, pb.postcode, pb.city, pb.street
//! FROM
//!   partner_partner pa, partner_branchstore pb
//! WHERE
//!   pa.id = pb.cmrpartner AND
//!   (
//!     ( pb.postcode>='26440' AND
//!       pb.postcode<='26452'
//!     ) OR
//!     ( pa.shortname LIKE '%Eisen%' OR
//!       pa.description LIKE '%Eisen%' OR
//!       pa.taxnumber LIKE '%Eisen%' OR
//!       pb.city LIKE '%Eisen%' OR
//!       pb.street LIKE '%Eisen%'
//!     ) AND
//!     NOT pa.shortname LIKE 'Hammer%'
//!   )
//! ORDER BY pb.postcode
//! ;
//! ```
//!
//! ## Usage
//! ```rust
//! use permeable::AllowAllPermission;
//! use suchbar::{DbField, Suchbar};
//! use suchbar::DbType::*;
//!
//! const SUCHBAR: Suchbar = Suchbar::new(&[
//!     //            fieldname in SQL, fieldtype    Permission  Aliases for query
//!     DbField::new("pa.shortname",    TEXT,        "STD",      &["sname", "sn"]),
//!     DbField::new("pa.description",  TEXT,        "STD",      &["desc", "d"]),
//!     DbField::new("pa.taxnumber",    VARCHAR(15), "STD",      &["ust_id", "tax", "ustid"]),
//!     DbField::new("pb.city",         VARCHAR(35), "STD",      &["city", "ort"]),
//!     DbField::new("pb.street",       VARCHAR(55), "STD",      &["street", "st"]),
//!     DbField::new("pb.postcode",     VARCHAR(5),  "STD",      &["plz", "zip"]),
//! ]);
//!
//! fn main() {
//!     let suche = "plz=26440-26452 OR (Eisen AND sn!=Hammer*); plz";
//!     match SUCHBAR.exec(&AllowAllPermission(), suche) {
//!         Err(error) => println!("\n{error}"),
//!         Ok(sr) => {
//!             let query = format ! (
//!             "SELECT pa.shortname, pa.description, pa.taxnumber, \
//!                     pb.longname, pb.postcode AS INTEGER, pb.city, pb.street \
//!                     FROM partner_partner pa, partner_branchstore pb \
//!                     WHERE pa.id = pb.cmrpartner{} LIMIT 20",
//!             sr.to_sql("AND")
//!             );
//!         }
//!     }
//! }
//! ```

mod comp_op;
mod db_field;
mod error;
mod sql_term;
mod suchbar;

#[macro_use]
extern crate pest_derive;

pub use crate::db_field::{DbField, DbType};
pub use crate::sql_term::SQLTerm;
pub use crate::suchbar::{SuchOptions, Suchbar, WhereClause};

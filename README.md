# SuchBar - Enduser-learnable language for database queries.

[![Crates.io][crates-badge]][crates-url]
[![MIT licensed][mit-badge]][mit-url]
[![Documentation][docs-badge]][docs-url]

[crates-badge]: https://img.shields.io/crates/v/suchbar.svg
[crates-url]: https://crates.io/crates/suchbar
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://github.com/NORICS—NET/suchbar/blob/master/LICENSE
[docs-badge]: https://docs.rs/suchbar/badge.svg
[docs-url]: https://docs.rs/suchbar

A query language that can be learned by normal users, similar to an internet
search engine. To empower users to filter and sort data in a kind of natural
language while preserving permissions.

The search query entered by the user is parsed and converted into an SQL WHERE
clause. For the best possible user experience, field-free queries are sent to
all syntactically eligible fields as similarity (LIKE) searches.

# What it does

A query like `"plz=26440-26452 OR (Eisen AND sn!=Hammer*); plz"` could generate a
WHERE-Clause for such kind of SELECT:

```sql
SELECT
    pa.shortname, pa.description, pa.taxnumber, pb.longname, pb.postcode, pb.city, pb.street
FROM
    partner_partner pa, partner_branchstore pb
WHERE
  pa.id = pb.cmrpartner AND
  (
    ( pb.postcode>='26440' AND
      pb.postcode<='26452'
    ) OR
    ( pa.shortname LIKE '%Eisen%' OR
      pa.description LIKE '%Eisen%' OR
      pa.taxnumber LIKE '%Eisen%' OR
      pb.city LIKE '%Eisen%' OR
      pb.street LIKE '%Eisen%'
    ) AND
    NOT pa.shortname LIKE 'Hammer%'
 )
LIMIT 20;
```

| Boolean Operator |Alternative Symbol	 | Description                                                                         |
|:----------------:|:------------------:|:---------------------------------------------------------------------------------------|
|       AND        |         &&         | 	Requires both terms on either side of the Boolean operator to be present for a match. |
|       NOT        |         !	        |   Requires that the following term not be present.                                      |
|       OR         |    &vert;&vert;    | 	Requires that either term (or both terms) be present for a match.                     |


## Usage:


### Register the search-fields.

The fields available for searching are registered:
`DbField::new("db-fieldname", db-type, "permission", &["name", "alternative_name", "abbr"]),`


## Example

```rust
use permeable::AllowAllPermission;
use suchbar::{DbField, Suchbar};
use suchbar::DbType::*;

const SUCHBAR: Suchbar = Suchbar::new(&[
    //            fieldname in SQL, fieldtype    Permission  Aliases for query
    DbField::new("pa.shortname",    TEXT,        "STD",      &["sname", "sn"]),
    DbField::new("pa.description",  TEXT,        "STD",      &["desc", "d"]),
    DbField::new("pa.taxnumber",    VARCHAR(15), "STD",      &["ust_id", "taxid", "ustid", "vatid"]),
    DbField::new("pb.city",         VARCHAR(35), "STD",      &["city", "ort"]),
    DbField::new("pb.street",       VARCHAR(55), "STD",      &["street", "st"]),
    DbField::new("pb.postcode",     VARCHAR(5),  "STD",      &["plz", "zip"]),
]);

fn main() {
    let query = "plz=26440-26452 OR (Eisen AND sn!=Hammer*)";
    match SUCHBAR.exec(&AllowAllPermission(), query) {
        Err(error) => println!("\n{error}"),
        Ok(sr) => {
            let sql = format!(
            "SELECT pa.shortname, pa.description, pa.taxnumber, \
                    pb.longname, pb.postcode AS INTEGER, pb.city, pb.street \
                    FROM partner_partner pa, partner_branchstore pb \
                    WHERE pa.id = pb.cmrpartner{} LIMIT 20",
            sr.to_sql("AND")
            );
        }
    }
}
```

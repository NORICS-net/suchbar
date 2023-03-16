# SuchBar - Such-Abfragesprache

Eine von Otto-Normal-Anwendern erlernbare Abfragesprache ähnlich einer Internet-Suchmaschine.   

Aus einer Anfrage "`plz=26440-26452 OR (Eisen AND sn!=Hammecke*)`" wird eine SQL Abfrage erstellt:

```sql
SELECT 
    pa.shortname, pa.description, pa.taxnumber, pb.longname, pb.postcode, pb.city, pb.street 
FROM partner_partner pa, partner_branchstore pb 
WHERE pa.id = pb.cmrpartner AND 
  (
    ( pb.postcode>='26440' 
          AND pb.postcode<='26452' 
    ) OR
    ( pa.shortname LIKE '%Eisen%' 
            OR pa.description LIKE '%Eisen%' 
            OR pa.taxnumber LIKE '%Eisen%' 
            OR pb.city LIKE '%Eisen%' 
            OR pb.street LIKE '%Eisen%' 
    ) AND 
    NOT pa.shortname LIKE 'Hammecke%' 
 )
LIMIT 20;
```

| Boolean Operator |Alternative Symbol	 | Description                                                                         |
|:----------------:|:------------------:|:---------------------------------------------------------------------------------------|
|       AND        |         &&         | 	Requires both terms on either side of the Boolean operator to be present for a match. |
|       NOT        |         !	         | Requires that the following term not be present.                                       |
|       OR         |    &vert;&vert;    | 	Requires that either term (or both terms) be present for a match.                     |


## Programmierung: 


### Anmeldung der Felder

Die zur Suche zur Verfügung stehenden Felder werden angemeldet: 
`DbField::new("datenbankfeld", Datentyp, "Berechtigung", &["name", "alternative_name", "abk"]),`   


## Beispiel

```rust 
const DB_FIELDS: [DbField; 6] = [
    DbField::new("pa.shortname", TEXT, "STD", &["sname", "sn"]),
    DbField::new("pa.description", TEXT, "STD", &["desc", "d"]),
    DbField::new(
        "pa.taxnumber",
        VARCHAR(15),
        "STD",
        &["ust_id", "tax", "ustid"],
    ),
    DbField::new("pb.city", VARCHAR(35), "STD", &["city"]),
    DbField::new("pb.street", VARCHAR(55), "STD", &["street", "st"]),
    DbField::new("pb.postcode", VARCHAR(5), "STD", &["plz", "zip"]),
];

fn main() {
    let mut suchbar = Suchbar::new(&DB_FIELDS);
    let suche = "plz=26440-26452 OR (Eisen AND sn!=Hammecke*)";
    match suchbar.exec(suche) {
        Ok(()) => {}
        Err(c) => println!("\n{c}"),
    }
    let query = format!(
        "SELECT pa.shortname, pa.description, pa.taxnumber, \
        pb.longname, pb.postcode AS INTEGER, pb.city, pb.street \
        FROM partner_partner pa, partner_branchstore pb \
        WHERE pa.id = pb.cmrpartner{} LIMIT 20",
        suchbar.to_sql("AND")
    );
}
```

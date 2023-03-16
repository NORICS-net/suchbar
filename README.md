# SuchBar - Such-Abfragesprache

Eine von Otto-Normal-Anwendern erlernbare Abfragesprache ähnlich einer Internet-Suchmaschine.   

Aus einer Anfrage mit
```
plz=26440-26452 OR (Eisen AND sn!=Hammecke*)
```
wird eine SQL Abfrage erstellt:

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
 );
```

| Boolean Operator |Alternative Symbol	 | Description                                                                         |
|:----------------:|:------------------:|:---------------------------------------------------------------------------------------|
|       AND        |         &&         | 	Requires both terms on either side of the Boolean operator to be present for a match. |
|       NOT        |         !	         | Requires that the following term not be present.                                       |
|       OR         |    &vert;&vert;    | 	Requires that either term (or both terms) be present for a match.                     |



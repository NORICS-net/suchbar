# SuchBar - Such-Abfragesprache

Eine von Otto-Normal-Anwendern erlernbare Abfragesprache ähnlich einer Internet-Suchmaschiene.   

Aus einer Anfrage mit
```
artnr=2334232 AND irgend ein Text
```
wird eine SQL Abfrage erstellt:

```sql
SELECT 
    artnr, field1, field2, field3 
FROM table_or_view 
WHERE
    artnr = "2334232" AND
    (field1 LIKE '%irgend%ein%Text%' OR 
        field2 LIKE '%irgend%ein%Text%' OR 
        field3 LIKE '%irgend%ein%Text%');
```

| Boolean Operator |Alternative Symbol	 | Description                                                                                                                                                                                                                                                                           |
|:----------------:|:--------:|:--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
|       AND        |    &&    | 	Requires both terms on either side of the Boolean operator to be present for a match.                                                                                                                                                                                                |
|       NOT        |    !	    | Requires that the following term not be present.                                                                                                                                                                                                                                      |
|        OR        | PIPE,PIPE | 	Requires that either term (or both terms) be present for a match.                                                                                                                                                                                                                    |
|        +	        |          | Requires that the following term be present.                                                                                                                                                                                                                                          |
|        -	        |          | Prohibits the following term (that is, matches on fields or documents that do not include that term). The - operator is functionally similar to the Boolean operator !. Because it’s used by popular search engines such as Google, it may be more familiar to some user communities. |



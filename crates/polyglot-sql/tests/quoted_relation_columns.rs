use polyglot_sql::{generate, parse_one, DialectType};

#[test]
fn quoted_relation_columns_survive_parse_generate_and_reparse() {
    for dialect in [DialectType::DuckDB, DialectType::PostgreSQL] {
        for sql in [
            "SELECT * FROM (SELECT 1 AS x) AS d(\"a b\")",
            "SELECT * FROM (SELECT 1 AS x) d(\"a b\")",
            "SELECT * FROM records AS d(\"a b\")",
            "SELECT * FROM records d(\"a b\")",
            "WITH d(\"a b\") AS (SELECT 1 AS x) SELECT * FROM d",
            "SELECT * FROM (VALUES (1)) AS d(\"a b\")",
            "SELECT * FROM UNNEST([1]) AS d(\"a b\")",
            "SELECT * FROM LATERAL UNNEST([1]) AS d(\"a b\")",
        ] {
            let parsed = parse_one(sql, dialect).unwrap();
            let output = generate(&parsed, dialect).unwrap();
            assert!(output.contains("\"a b\""), "{sql}: {output}");
            let reparsed = parse_one(&output, dialect).unwrap();
            assert_eq!(generate(&reparsed, dialect).unwrap(), output);
        }
    }
}

#[test]
fn lateral_relation_and_column_alias_quotes_survive_json() {
    for dialect in [DialectType::DuckDB, DialectType::PostgreSQL] {
        for alias in [
            "AS \"item rows\"(\"unit value\")",
            "\"item rows\"(\"unit value\")",
            "AS \"Items\"(\"Value\")",
            "AS d(\"a\"\"b\")",
            "AS d(unquoted, \"unit value\")",
        ] {
            let sql = format!("SELECT * FROM LATERAL UNNEST(ARRAY[1]) {alias}");
            let parsed = parse_one(&sql, dialect).unwrap();
            let output = generate(&parsed, dialect).unwrap();
            assert!(
                output.contains(alias.trim_start_matches("AS ")),
                "{sql}: {output}"
            );
            let json = serde_json::to_string(&parsed).unwrap();
            let restored = serde_json::from_str(&json).unwrap();
            assert_eq!(generate(&restored, dialect).unwrap(), output);
        }
    }
}

#[test]
fn unquoted_lateral_aliases_keep_the_existing_json_shape() {
    let parsed = parse_one(
        "SELECT * FROM LATERAL UNNEST(ARRAY[1]) AS items(value)",
        DialectType::DuckDB,
    )
    .unwrap();
    let json = serde_json::to_string(&parsed).unwrap();
    assert!(!json.contains("column_aliases_quoted"));
    let restored = serde_json::from_str(&json).unwrap();
    assert_eq!(
        generate(&restored, DialectType::DuckDB).unwrap(),
        generate(&parsed, DialectType::DuckDB).unwrap()
    );
}

#[test]
fn quoted_aliases_do_not_interpret_index_or_order_suffixes() {
    for dialect in [DialectType::DuckDB, DialectType::PostgreSQL] {
        for alias in [
            r#"AS items("value(1)")"#,
            r#"AS items("value ASC")"#,
            r#"AS items("value DESC")"#,
            r#"AS "items(1)"(value)"#,
            r#"AS "items ASC"(value)"#,
            r#"AS "items DESC"("value(1)")"#,
        ] {
            for relation in ["LATERAL UNNEST(ARRAY[1])", "(VALUES (1))", "(SELECT 1)"] {
                let sql = format!("SELECT * FROM {relation} {alias}");
                let parsed = parse_one(&sql, dialect).unwrap();
                let json = serde_json::to_string(&parsed).unwrap();
                let restored = serde_json::from_str(&json).unwrap();
                let output = generate(&restored, dialect).unwrap();
                assert!(output.contains(alias), "{sql}: {output}");
                let reparsed = parse_one(&output, dialect).unwrap();
                assert_eq!(generate(&reparsed, dialect).unwrap(), output);
            }
        }
    }
}

#[test]
fn quoted_alias_references_keep_complete_identifier_names() {
    for dialect in [DialectType::DuckDB, DialectType::PostgreSQL] {
        for (relation, column) in [("items(1)", "value(1)"), ("items DESC", "value ASC")] {
            let sql = format!(
                r#"SELECT "{relation}"."{column}" FROM LATERAL UNNEST(ARRAY[1]) AS "{relation}"("{column}")"#
            );
            let parsed = parse_one(&sql, dialect).unwrap();
            let output = generate(&parsed, dialect).unwrap();
            assert!(
                output.starts_with(&format!(r#"SELECT "{relation}"."{column}""#)),
                "{output}"
            );
            assert!(parse_one(&output, dialect).is_ok());
        }
    }
}

#[test]
fn index_column_prefixes_and_ordering_keep_their_syntax() {
    for sql in [
        "CREATE TABLE t (name VARCHAR(64), PRIMARY KEY (`name`(16) DESC))",
        "CREATE TABLE t (name VARCHAR(64), UNIQUE (`name`(16) ASC))",
    ] {
        let parsed = parse_one(sql, DialectType::MySQL).unwrap();
        let output = generate(&parsed, DialectType::MySQL).unwrap();
        assert!(output.contains("`name`(16)"), "{output}");
        assert!(parse_one(&output, DialectType::MySQL).is_ok());
    }
}

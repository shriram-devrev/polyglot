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

use polyglot_sql::{generate, parse_one, DialectType, Expression, ExpressionWalk};

fn assert_shapes(sql: &str, dialect: DialectType, json_count: usize, lambda_count: usize) {
    let expression = parse_one(sql, dialect).unwrap_or_else(|error| panic!("{sql}: {error}"));
    assert_eq!(
        expression.count(|node| matches!(node, Expression::JsonExtract(_))),
        json_count,
        "JSON extraction count: {sql}"
    );
    assert_eq!(
        expression.count(|node| matches!(node, Expression::Lambda(_))),
        lambda_count,
        "lambda count: {sql}"
    );
    let generated = generate(&expression, dialect).unwrap();
    let reparsed = parse_one(&generated, dialect).unwrap();
    assert_eq!(
        reparsed.count(|node| matches!(node, Expression::JsonExtract(_))),
        json_count,
        "generated SQL: {generated}"
    );
    assert_eq!(
        reparsed.count(|node| matches!(node, Expression::Lambda(_))),
        lambda_count,
        "generated SQL: {generated}"
    );
}

#[test]
fn json_arrows_accept_path_lists_and_dynamic_paths() {
    for input in ["payload", "s.payload", "(payload)", "CAST(payload AS JSON)"] {
        for path in ["['$.a', '$.b']", "path", "0", "'$.a'"] {
            for expression in [
                format!("{input} -> {path}"),
                format!("COALESCE({input} -> {path}, NULL)"),
            ] {
                assert_shapes(
                    &format!("SELECT {expression} FROM samples AS s"),
                    DialectType::DuckDB,
                    1,
                    0,
                );
            }
        }
    }
}

#[test]
fn lambda_arguments_keep_array_returning_and_nested_bodies() {
    for function in [
        "apply",
        "array_apply",
        "array_transform",
        "list_apply",
        "list_transform",
    ] {
        for body in ["[x, x]", "['$.a', '$.b']", "(x -> ['$.a', '$.b'])"] {
            assert_shapes(
                &format!("SELECT {function}(items, x -> {body}) FROM samples"),
                DialectType::DuckDB,
                usize::from(body.starts_with("(x ->")),
                1,
            );
        }
    }
    for function in ["filter", "array_filter", "list_filter"] {
        assert_shapes(
            &format!("SELECT {function}(items, x -> (x -> '$.a') IS NOT NULL) FROM samples"),
            DialectType::DuckDB,
            1,
            1,
        );
    }
    for function in ["reduce", "array_reduce", "list_reduce"] {
        assert_shapes(
            &format!("SELECT {function}(items, (x, y) -> x + y, 0) FROM samples"),
            DialectType::DuckDB,
            0,
            1,
        );
    }
}

#[test]
fn json_inputs_and_nested_lambdas_use_their_own_argument_context() {
    for sql in [
        "SELECT list_transform(JSON_EXTRACT(payload, ['$.a', '$.b']), x -> CAST(x AS INTEGER)) FROM samples",
        "SELECT list_transform(items, x -> list_transform([x], y -> (y -> ['$.a']))) FROM samples",
    ] {
        let lambdas = if sql.contains("y ->") { 2 } else { 1 };
        assert_shapes(sql, DialectType::DuckDB, 1, lambdas);
    }
    for sql in [
        "SELECT COLUMNS(c -> starts_with(c, 'a')) FROM samples",
        "SELECT COALESCE(*COLUMNS(c -> starts_with(c, 'a'))) FROM samples",
        "SELECT list_transform(items, LAMBDA x : x -> ['$.a']) FROM samples",
        "SELECT main.list_transform(items, (x) -> [x, x]) FROM samples",
        "SELECT list_transform(items, (x -> [x, x])) FROM samples",
        "SELECT list_transform(items, ((x) -> [x, x])) FROM samples",
        "SELECT list_transform(items, \"x\" -> [\"x\", \"x\"]) FROM samples",
    ] {
        assert_shapes(
            sql,
            DialectType::DuckDB,
            usize::from(sql.contains("['$.a']")),
            1,
        );
    }
}

#[test]
fn other_dialects_keep_arrow_lambda_support() {
    for dialect in [
        DialectType::Generic,
        DialectType::ClickHouse,
        DialectType::Spark,
    ] {
        assert_shapes(
            "SELECT transform(items, x -> [x, x]) FROM samples",
            dialect,
            0,
            1,
        );
    }
}

fn projected(sql: &str) -> Expression {
    let Expression::Select(select) = parse_one(sql, DialectType::DuckDB).unwrap() else {
        panic!("expected SELECT")
    };
    select.expressions[0].clone()
}

#[test]
fn generated_json_arrows_keep_their_operand_boundary() {
    use polyglot_sql::expressions::{BinaryOp, IsNull};
    for json in ["payload -> '$.a'", "payload -> ['$.a', '$.b']"] {
        let extraction = projected(&format!("SELECT {json}"));
        let comparison = Expression::Eq(Box::new(BinaryOp::new(
            extraction.clone(),
            projected("SELECT other"),
        )));
        let generated = generate(&comparison, DialectType::DuckDB).unwrap();
        assert_eq!(generated, format!("({json}) = other"));
        assert!(matches!(
            projected(&format!("SELECT {generated}")),
            Expression::Eq(_)
        ));
        let null_test = Expression::IsNull(Box::new(IsNull {
            this: extraction,
            not: false,
            postfix_form: false,
        }));
        assert_eq!(
            generate(&null_test, DialectType::DuckDB).unwrap(),
            format!("({json}) IS NULL")
        );
    }
}

#[test]
fn standalone_json_arrows_keep_native_rendering() {
    for sql in [
        "SELECT payload -> '$.a'",
        "SELECT payload -> ['$.a', '$.b']",
        "SELECT payload -> '$.a' -> '$[0]'",
        "SELECT (payload -> '$.a') = other",
        "SELECT (payload -> '$.a')[1]",
    ] {
        let expression = parse_one(sql, DialectType::DuckDB).unwrap();
        assert_eq!(generate(&expression, DialectType::DuckDB).unwrap(), sql);
    }
}

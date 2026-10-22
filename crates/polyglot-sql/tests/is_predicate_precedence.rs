use polyglot_sql::expressions::{IsNull, IsTrueFalse};
use polyglot_sql::{generate, parse_one, DialectType, Expression};

fn projected(sql: &str) -> Expression {
    let Expression::Select(select) = parse_one(sql, DialectType::DuckDB).unwrap() else {
        panic!("expected SELECT")
    };
    select.expressions[0].clone()
}

#[test]
fn null_predicates_group_unparenthesized_boolean_operands() {
    for operand in ["a OR b", "a AND b", "NOT a"] {
        for not in [false, true] {
            for postfix_form in [false, true] {
                for dialect in [
                    DialectType::DuckDB,
                    DialectType::PostgreSQL,
                    DialectType::Generic,
                ] {
                    let expression = Expression::IsNull(Box::new(IsNull {
                        this: projected(&format!("SELECT {operand}")),
                        not,
                        postfix_form,
                    }));
                    let output = generate(&expression, dialect).unwrap();
                    let prefix = not
                        && (dialect == DialectType::Generic
                            || (postfix_form && dialect != DialectType::PostgreSQL));
                    let expected = if prefix {
                        format!("NOT ({operand}) IS NULL")
                    } else {
                        format!("({operand}) IS {}NULL", if not { "NOT " } else { "" })
                    };
                    assert_eq!(output, expected, "{dialect:?}");
                    assert_eq!(
                        projected(&format!("SELECT {output}")),
                        projected(&format!("SELECT {expected}"))
                    );
                }
            }
        }
    }
}

#[test]
fn truth_predicates_group_unparenthesized_boolean_operands() {
    for operand in ["a OR b", "a AND b", "NOT a"] {
        for not in [false, true] {
            for truth in [false, true] {
                let test = Box::new(IsTrueFalse {
                    this: projected(&format!("SELECT {operand}")),
                    not,
                });
                let expression = if truth {
                    Expression::IsTrue(test)
                } else {
                    Expression::IsFalse(test)
                };
                let output = generate(&expression, DialectType::DuckDB).unwrap();
                let expected = format!(
                    "({operand}) IS {}{}",
                    if not { "NOT " } else { "" },
                    if truth { "TRUE" } else { "FALSE" }
                );
                assert_eq!(output, expected);
                assert_eq!(
                    projected(&format!("SELECT {output}")),
                    projected(&format!("SELECT {expected}"))
                );
            }
        }
    }
}

#[test]
fn atomic_and_already_parenthesized_operands_keep_their_shape() {
    for operand in ["a", "(a OR b)", "(a AND b)", "(NOT a)", "COALESCE(a, b)"] {
        let expression = Expression::IsNull(Box::new(IsNull {
            this: projected(&format!("SELECT {operand}")),
            not: true,
            postfix_form: false,
        }));
        assert_eq!(
            generate(&expression, DialectType::DuckDB).unwrap(),
            format!("{operand} IS NOT NULL")
        );
    }
}

#[test]
fn builder_is_predicates_preserve_boolean_operands() {
    use polyglot_sql::builder::col;
    for operand in [col("a").or(col("b")), col("a").and(col("b"))] {
        let expression = operand.clone().is_null().0;
        let output = generate(&expression, DialectType::DuckDB).unwrap();
        assert_eq!(
            projected(&format!("SELECT {output}")),
            projected(&format!(
                "SELECT ({}) IS NULL",
                generate(&operand.0, DialectType::DuckDB).unwrap()
            ))
        );
    }
}

#[test]
fn null_safe_predicates_preserve_both_boolean_operands() {
    use polyglot_sql::expressions::BinaryOp;
    for distinct in [false, true] {
        let operands = Box::new(BinaryOp::new(
            projected("SELECT a OR b"),
            projected("SELECT c AND d"),
        ));
        let expression = if distinct {
            Expression::NullSafeNeq(operands)
        } else {
            Expression::NullSafeEq(operands)
        };
        let output = generate(&expression, DialectType::DuckDB).unwrap();
        let expected = format!(
            "SELECT (a OR b) IS {}DISTINCT FROM (c AND d)",
            if distinct { "" } else { "NOT " }
        );
        assert_eq!(projected(&format!("SELECT {output}")), projected(&expected));
    }
}

#[test]
fn clickhouse_not_wraps_the_null_predicate() {
    let Expression::Select(select) =
        parse_one("SELECT NOT a IS NULL", DialectType::ClickHouse).unwrap()
    else {
        panic!("expected SELECT")
    };
    assert!(matches!(&select.expressions[0], Expression::Not(_)));
}

#[test]
fn json_predicates_preserve_boolean_operands() {
    use polyglot_sql::expressions::IsJson;
    for operand in ["a OR b", "a AND b", "NOT a"] {
        for negated in [false, true] {
            let expression = Expression::IsJson(Box::new(IsJson {
                this: projected(&format!("SELECT {operand}")),
                json_type: None,
                unique_keys: None,
                negated,
            }));
            assert_eq!(
                generate(&expression, DialectType::PostgreSQL).unwrap(),
                format!("({operand}) IS {}JSON", if negated { "NOT " } else { "" })
            );
        }
    }
}

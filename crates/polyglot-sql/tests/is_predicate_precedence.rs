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

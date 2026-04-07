use crate::coroutine_transform::*;
use ni_error::Span;
use ni_parser::*;

fn make_span() -> Span {
    Span::default()
}

fn make_yield_call() -> Statement {
    Statement {
        kind: StmtKind::Expr(Expr {
            kind: ExprKind::Call {
                callee: Box::new(Expr {
                    kind: ExprKind::Identifier("yield".to_string()),
                    span: make_span(),
                }),
                args: vec![],
                named_args: vec![],
            },
            span: make_span(),
        }),
        span: make_span(),
    }
}

fn make_var_decl(name: &str, value: i64) -> Statement {
    Statement {
        kind: StmtKind::VarDecl(VarDecl {
            name: name.to_string(),
            type_ann: None,
            initializer: Expr {
                kind: ExprKind::IntLiteral(value),
                span: make_span(),
            },
        }),
        span: make_span(),
    }
}

fn make_pass() -> Statement {
    Statement {
        kind: StmtKind::Pass,
        span: make_span(),
    }
}

#[test]
fn test_no_yield_points() {
    let stmts = vec![make_pass(), make_var_decl("x", 1)];
    let points = find_yield_points(&stmts);
    assert!(points.is_empty());
}

#[test]
fn test_single_yield_point() {
    let stmts = vec![
        make_var_decl("x", 1),
        make_yield_call(),
        make_var_decl("y", 2),
    ];
    let points = find_yield_points(&stmts);
    assert_eq!(points.len(), 1);
    assert_eq!(points[0], 1);
}

#[test]
fn test_multiple_yield_points() {
    let stmts = vec![
        make_var_decl("x", 1),
        make_yield_call(),
        make_var_decl("y", 2),
        make_yield_call(),
        make_var_decl("z", 3),
    ];
    let points = find_yield_points(&stmts);
    assert_eq!(points.len(), 2);
    assert_eq!(points[0], 1);
    assert_eq!(points[1], 3);
}

#[test]
fn test_hoisted_vars() {
    let stmts = vec![
        make_var_decl("x", 1),
        make_yield_call(),
        make_var_decl("y", 2),
    ];
    let vars = find_hoisted_vars(&stmts);
    assert!(vars.contains(&"x".to_string()));
    assert!(vars.contains(&"y".to_string()));
}

#[test]
fn test_no_hoisted_vars_without_yield() {
    let stmts = vec![make_var_decl("x", 1), make_pass()];
    let vars = find_hoisted_vars(&stmts);
    assert!(vars.is_empty());
}

#[test]
fn test_build_ir_no_yield() {
    let stmts = vec![make_pass()];
    let ir = build_coroutine_ir("test", &[], &stmts);
    assert!(ir.is_none());
}

#[test]
fn test_build_ir_with_yield() {
    let stmts = vec![
        make_var_decl("x", 1),
        make_yield_call(),
        make_var_decl("y", 2),
    ];
    let ir = build_coroutine_ir("test_fn", &[], &stmts);
    assert!(ir.is_some());
    let ir = ir.unwrap();
    assert_eq!(ir.name, "test_fn");
    assert_eq!(ir.states.len(), 2); // before yield + after yield
}

#[test]
fn test_build_ir_multiple_yields() {
    let stmts = vec![
        make_var_decl("a", 1),
        make_yield_call(),
        make_var_decl("b", 2),
        make_yield_call(),
        make_var_decl("c", 3),
    ];
    let ir = build_coroutine_ir("multi", &[], &stmts);
    assert!(ir.is_some());
    let ir = ir.unwrap();
    assert_eq!(ir.states.len(), 3);
}

#[test]
fn test_wait_detected_as_yield() {
    let wait_call = Statement {
        kind: StmtKind::Expr(Expr {
            kind: ExprKind::Call {
                callee: Box::new(Expr {
                    kind: ExprKind::Identifier("wait".to_string()),
                    span: make_span(),
                }),
                args: vec![Expr {
                    kind: ExprKind::FloatLiteral(1.0),
                    span: make_span(),
                }],
                named_args: vec![],
            },
            span: make_span(),
        }),
        span: make_span(),
    };
    let stmts = vec![make_var_decl("x", 1), wait_call];
    let points = find_yield_points(&stmts);
    assert_eq!(points.len(), 1);
}

#[test]
fn test_wait_until_detected_as_yield() {
    let wait_until_call = Statement {
        kind: StmtKind::Expr(Expr {
            kind: ExprKind::Call {
                callee: Box::new(Expr {
                    kind: ExprKind::Identifier("wait_until".to_string()),
                    span: make_span(),
                }),
                args: vec![Expr {
                    kind: ExprKind::BoolLiteral(true),
                    span: make_span(),
                }],
                named_args: vec![],
            },
            span: make_span(),
        }),
        span: make_span(),
    };
    let stmts = vec![make_var_decl("x", 1), wait_until_call];
    let points = find_yield_points(&stmts);
    assert_eq!(points.len(), 1);
}

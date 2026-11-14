//! Phase 3 §5 step 2 — HIR node types + `lower_expr` with resolved types.

use rca::compiler::hir::{HExprKind, lower_expr};
use rca::compiler::lexer::Lexer;
use rca::compiler::parser::Parser;
use rca::compiler::semantic::Checker;
use rca::compiler::types::Type;

fn parse(src: &str) -> rca::compiler::ast::Program {
    let tokens = Lexer::new(src).tokenize().expect("lex");
    Parser::new(tokens).parse_program().expect("parse")
}

fn first_let_expr(
    src: &str,
) -> (
    rca::compiler::ast::Expr,
    std::collections::HashMap<(usize, usize), Type>,
) {
    let program = parse(src);
    let (result, types) = Checker::new().check_program_with_types(&program);
    assert!(result.is_ok(), "checker failed");
    let stmt = &program.statements[0];
    let expr = match &stmt.kind {
        rca::compiler::ast::StmtKind::Let { value, .. } => value.clone(),
        _ => panic!("expected let"),
    };
    (expr, types)
}

#[test]
fn lower_int_literal() {
    let (expr, types) = first_let_expr("let x = 5\n");
    let h = lower_expr(&expr, &types);
    assert!(matches!(h.kind, HExprKind::Int(5)));
    assert_eq!(h.ty, Type::Int);
}

#[test]
fn lower_string_literal_has_type() {
    let (expr, types) = first_let_expr("let s = \"hi\"\n");
    let h = lower_expr(&expr, &types);
    assert!(matches!(h.kind, HExprKind::Str(_)));
    assert_eq!(h.ty, Type::String);
}

#[test]
fn lower_binary_recurses_and_has_type() {
    let (expr, types) = first_let_expr("let r = 1 + 2\n");
    let h = lower_expr(&expr, &types);
    assert!(matches!(h.kind, HExprKind::Binary { .. }));
    assert_eq!(h.ty, Type::Int);
    if let HExprKind::Binary { left, right, .. } = h.kind {
        assert!(matches!(left.kind, HExprKind::Int(1)));
        assert!(matches!(right.kind, HExprKind::Int(2)));
        assert_eq!(left.ty, Type::Int);
    } else {
        unreachable!();
    }
}

#[test]
fn lower_identifier() {
    let (expr, types) = first_let_expr("let a = 1\nlet b = a\n");
    // Take the second let's value.
    let program = parse("let a = 1\nlet b = a\n");
    let (r, types2) = Checker::new().check_program_with_types(&program);
    assert!(r.is_ok());
    let expr2 = match &program.statements[1].kind {
        rca::compiler::ast::StmtKind::Let { value, .. } => value.clone(),
        _ => panic!("expected let"),
    };
    let _ = (expr, types); // silence unused
    let h = lower_expr(&expr2, &types2);
    assert!(matches!(h.kind, HExprKind::Ident(ref n) if n == "a"));
    assert_eq!(h.ty, Type::Int);
}

#[test]
fn lower_call_has_type() {
    let src = "fn add(a: Int, b: Int) -> Int:\n    return a + b\nlet r = add(1, 2)\n";
    let program = parse(src);
    let (result, types) = Checker::new().check_program_with_types(&program);
    assert!(result.is_ok());
    let expr = match &program.statements[1].kind {
        rca::compiler::ast::StmtKind::Let { value, .. } => value.clone(),
        _ => panic!("expected let"),
    };
    let h = lower_expr(&expr, &types);
    assert!(matches!(h.kind, HExprKind::Call { .. }));
    assert_eq!(h.ty, Type::Int);
}

#[test]
fn missing_span_defaults_to_unknown() {
    // Lower a literal with an empty type map; ty should be Unknown.
    let program = parse("let x = 42\n");
    let expr = match &program.statements[0].kind {
        rca::compiler::ast::StmtKind::Let { value, .. } => value.clone(),
        _ => panic!("expected let"),
    };
    let h = lower_expr(&expr, &std::collections::HashMap::new());
    assert_eq!(h.ty, Type::Unknown);
}

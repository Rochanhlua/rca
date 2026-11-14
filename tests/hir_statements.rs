//! Phase 3 §5 step 3 — statement-level HIR (`lower_ast_to_hir`).

use rca::compiler::hir::{HStmtKind, lower_ast_to_hir};
use rca::compiler::lexer::Lexer;
use rca::compiler::parser::Parser;
use rca::compiler::semantic::Checker;

fn lower(src: &str) -> Vec<rca::compiler::hir::HStmt> {
    let tokens = Lexer::new(src).tokenize().expect("lex");
    let program = Parser::new(tokens).parse_program().expect("parse");
    let (result, types) = Checker::new().check_program_with_types(&program);
    assert!(result.is_ok(), "check failed: {result:?}");
    lower_ast_to_hir(&program, &types)
}

#[test]
fn lower_let_stmt() {
    let h = lower("let x = 5\n");
    assert_eq!(h.len(), 1);
    assert!(matches!(h[0].kind, HStmtKind::Let { .. }));
}

#[test]
fn lower_assign_and_expr() {
    let h = lower("mut x = 1\nx = 2\nprint(x)\n");
    assert_eq!(h.len(), 3);
    assert!(matches!(h[0].kind, HStmtKind::Let { .. }));
    assert!(matches!(h[1].kind, HStmtKind::Assign { .. }));
    assert!(matches!(h[2].kind, HStmtKind::Expr(_)));
}

#[test]
fn lower_if_while_for() {
    let h =
        lower("if true:\n    print(1)\nwhile false:\n    print(2)\nfor i in 0..3:\n    print(i)\n");
    assert_eq!(h.len(), 3);
    assert!(matches!(h[0].kind, HStmtKind::If { .. }));
    assert!(matches!(h[1].kind, HStmtKind::While { .. }));
    assert!(matches!(h[2].kind, HStmtKind::For { .. }));
}

#[test]
fn lower_function_decl() {
    let h = lower("fn add(a: Int, b: Int) -> Int:\n    return a + b\n");
    assert_eq!(h.len(), 1);
    match &h[0].kind {
        HStmtKind::Function(f) => {
            assert_eq!(f.name, "add");
            assert_eq!(f.params.len(), 2);
            assert!(!f.public);
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn lower_struct_and_enum() {
    let h = lower("struct P:\n    x: Int\n    y: Int\nenum C:\n    R\n    G\n");
    assert_eq!(h.len(), 2);
    match &h[0].kind {
        HStmtKind::Struct(s) => {
            assert_eq!(s.name, "P");
            assert_eq!(s.fields.len(), 2);
        }
        _ => panic!("expected Struct"),
    }
    match &h[1].kind {
        HStmtKind::Enum(e) => {
            assert_eq!(e.name, "C");
            assert_eq!(e.variants.len(), 2);
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn lower_return_break_continue() {
    let h = lower(
        "fn f() -> Int:\n    for i in 0..3:\n        if i == 1:\n            break\n        continue\n    return 1\n",
    );
    match &h[0].kind {
        HStmtKind::Function(f) => {
            assert!(matches!(f.body[1].kind, HStmtKind::Return(Some(_))));
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn lower_try_throw() {
    let h = lower("try:\n    throw \"e\"\ncatch e:\n    print(e)\n");
    assert_eq!(h.len(), 1);
    match &h[0].kind {
        HStmtKind::Try {
            body,
            catch_var,
            catch_block,
        } => {
            assert_eq!(body.len(), 1);
            assert!(matches!(body[0].kind, HStmtKind::Throw(_)));
            assert_eq!(catch_var, "e");
            assert_eq!(catch_block.len(), 1);
        }
        _ => panic!("expected Try"),
    }
}

#[test]
fn lower_let_tuple() {
    let h = lower("let (a, b) = (1, 2)\n");
    match &h[0].kind {
        HStmtKind::LetTuple { names, .. } => assert_eq!(names, &vec!["a", "b"]),
        _ => panic!("expected LetTuple"),
    }
}

//! Phase 3 §5 — HIR foundation: checker records Span -> Type for every expr.

use rca::compiler::lexer::Lexer;
use rca::compiler::parser::Parser;
use rca::compiler::semantic::Checker;
use rca::compiler::types::Type;

fn parse(src: &str) -> rca::compiler::ast::Program {
    let tokens = Lexer::new(src).tokenize().expect("lex");
    Parser::new(tokens).parse_program().expect("parse")
}

#[test]
fn expr_types_captures_int_literal() {
    let src = "let x = 5\n";
    let program = parse(src);
    let (result, types) = Checker::new().check_program_with_types(&program);
    assert!(result.is_ok());
    // The literal `5` is at line 1, column 9.
    let ty = types.get(&(1, 9)).expect("type for Int literal");
    assert_eq!(*ty, Type::Int);
}

#[test]
fn expr_types_captures_string_literal() {
    let src = "let x = \"hi\"\n";
    let program = parse(src);
    let (result, types) = Checker::new().check_program_with_types(&program);
    assert!(result.is_ok());
    let ty = types.get(&(1, 9)).expect("type for String literal");
    assert_eq!(*ty, Type::String);
}

#[test]
fn expr_types_captures_binop_result() {
    // `1 + 2` is at 1:9. Both operands Int, result Int.
    let src = "let x = 1 + 2\n";
    let program = parse(src);
    let (result, types) = Checker::new().check_program_with_types(&program);
    assert!(result.is_ok());
    // The whole binary expression's span is the left operand's span.
    let ty = types.get(&(1, 9)).expect("type for binary expression");
    assert_eq!(*ty, Type::Int);
}

#[test]
fn check_program_still_works_unchanged() {
    // The existing API must keep working; the type map is just an extra.
    let src = "let x = 5\nprint(x)\n";
    let program = parse(src);
    assert!(Checker::new().check_program(&program).is_ok());
}

#[test]
fn check_program_with_types_returns_errors() {
    let src = "let x: Int = \"hi\"\n";
    let program = parse(src);
    let (result, _types) = Checker::new().check_program_with_types(&program);
    assert!(result.is_err(), "type mismatch should still error");
}

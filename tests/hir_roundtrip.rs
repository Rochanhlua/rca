//! Phase 3 §5 step 4a — differential test: AST -> IR vs AST -> HIR -> AST -> IR.

use rca::compiler::ir::lower;
use rca::compiler::lexer::Lexer;
use rca::compiler::parser::Parser;
use rca::compiler::semantic::Checker;

fn parse(src: &str) -> rca::compiler::ast::Program {
    let tokens = Lexer::new(src).tokenize().expect("lex");
    Parser::new(tokens).parse_program().expect("parse")
}

fn check_eq(src: &str, label: &str) {
    let program = parse(src);
    let (result, types) = Checker::new().check_program_with_types(&program);
    assert!(result.is_ok(), "{}: check failed", label);
    let direct = lower::compile(&program).expect("direct compile");
    let via_hir = lower::compile_hir(&program, &types).expect("hir compile");
    assert_eq!(
        format!("{:?}", direct),
        format!("{:?}", via_hir),
        "{}: IR differs",
        label
    );
}

#[test]
fn roundtrip_let_arith() {
    check_eq("let x = 1 + 2\nprint(x)\n", "let_arith");
}

#[test]
fn roundtrip_if_else() {
    check_eq(
        "let x = 5\nif x > 3:\n    print(1)\nelse:\n    print(2)\n",
        "if_else",
    );
}

#[test]
fn roundtrip_while() {
    check_eq(
        "mut i = 0\nwhile i < 3:\n    print(i)\n    i = i + 1\n",
        "while",
    );
}

#[test]
fn roundtrip_for() {
    check_eq("for i in 0..3:\n    print(i)\n", "for");
}

#[test]
fn roundtrip_function() {
    check_eq(
        "fn add(a: Int, b: Int) -> Int:\n    return a + b\nprint(add(1, 2))\n",
        "function",
    );
}

#[test]
fn roundtrip_struct() {
    check_eq(
        "struct P:\n    x: Int\n    y: Int\nlet p = P(x = 1, y = 2)\nprint(p.x)\n",
        "struct",
    );
}

#[test]
fn roundtrip_enum() {
    check_eq("enum C:\n    R\n    G\nprint(1)\n", "enum");
}

#[test]
fn roundtrip_try_throw() {
    check_eq(
        "try:\n    throw \"e\"\ncatch e:\n    print(e)\n",
        "try_throw",
    );
}

#[test]
fn roundtrip_let_tuple() {
    check_eq("let (a, b) = (1, 2)\nprint(a)\nprint(b)\n", "let_tuple");
}

#[test]
fn roundtrip_match() {
    check_eq(
        "let x = 1\nmatch x:\n    1:\n        print(1)\n    _:\n        print(0)\n",
        "match",
    );
}

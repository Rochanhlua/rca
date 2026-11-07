use super::scope::{ScopeStack, Symbol};
use crate::compiler::ast::*;
use crate::compiler::diagnostics::Diagnostic;
use crate::compiler::lexer::token::Span;
use crate::compiler::types::Type;

pub struct Checker {
    scopes: ScopeStack,
    errors: Vec<Diagnostic>,
    loop_depth: usize,
}

impl Checker {
    pub fn new() -> Self {
        Checker {
            scopes: ScopeStack::new(),
            errors: Vec::new(),
            loop_depth: 0,
        }
    }

    pub fn check_program(mut self, program: &Program) -> Result<(), Vec<Diagnostic>> {
        for stmt in &program.statements {
            self.check_stmt(stmt);
        }
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors)
        }
    }

    fn error(&mut self, message: impl Into<String>, span: &Span) {
        self.errors.push(Diagnostic::new(message, span.clone()));
    }

    fn check_block(&mut self, stmts: &[Stmt]) {
        self.scopes.push();
        for s in stmts {
            self.check_stmt(s);
        }
        self.scopes.pop();
    }

    fn check_condition(&mut self, expr: &Expr) {
        let ty = self.check_expr(expr);
        if ty != Type::Bool && ty != Type::Unknown {
            self.error(format!("condition must be Bool, found {}", ty), &expr.span);
        }
    }

    // ---------- statements ----------

    fn check_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Let { name, mutable, ty, value } => {
                let value_ty = self.check_expr(value);
                let var_ty = match ty {
                    Some(annotation) => match Type::from_name(&annotation.name) {
                        Some(declared) => {
                            if !Type::compatible(&declared, &value_ty) {
                                self.error(
                                    format!("type mismatch: expected {}, found {}", declared, value_ty),
                                    &value.span,
                                );
                            }
                            declared
                        }
                        None => {
                            self.error(
                                format!("unknown type '{}'", annotation.name),
                                &annotation.span,
                            );
                            Type::Unknown
                        }
                    },
                    None => value_ty,
                };
                let symbol = Symbol { ty: var_ty, mutable: *mutable };
                if !self.scopes.declare(name, symbol) {
                    self.error(
                        format!("'{}' is already declared in this scope", name),
                        &stmt.span,
                    );
                }
            }
            StmtKind::Assign { target, value } => {
                let value_ty = self.check_expr(value);
                match &target.kind {
                    ExprKind::Ident(name) => {
                        let found = self.scopes.lookup(name).map(|s| (s.ty.clone(), s.mutable));
                        match found {
                            None => {
                                self.error(format!("undefined variable '{}'", name), &target.span);
                            }
                            Some((var_ty, mutable)) => {
                                if !mutable {
                                    self.error(
                                        format!(
                                            "cannot assign to immutable variable '{}' (declare it with 'mut')",
                                            name
                                        ),
                                        &target.span,
                                    );
                                } else if !Type::compatible(&var_ty, &value_ty) {
                                    self.error(
                                        format!("type mismatch: expected {}, found {}", var_ty, value_ty),
                                        &value.span,
                                    );
                                }
                            }
                        }
                    }
                    _ => {
                        self.error("only assignment to a variable is supported for now", &target.span);
                    }
                }
            }
            StmtKind::Expr(e) => {
                self.check_expr(e);
            }
            StmtKind::Return(_) => {
                self.error("'return' outside of a function", &stmt.span);
            }
            StmtKind::Break => {
                if self.loop_depth == 0 {
                    self.error("'break' outside of a loop", &stmt.span);
                }
            }
            StmtKind::Continue => {
                if self.loop_depth == 0 {
                    self.error("'continue' outside of a loop", &stmt.span);
                }
            }
            StmtKind::If { condition, then_block, else_block } => {
                self.check_condition(condition);
                self.check_block(then_block);
                if let Some(block) = else_block {
                    self.check_block(block);
                }
            }
            StmtKind::While { condition, body } => {
                self.check_condition(condition);
                self.loop_depth += 1;
                self.check_block(body);
                self.loop_depth -= 1;
            }
            StmtKind::For { var, iter, body } => {
                let iter_ty = self.check_expr(iter);
                let var_ty = match iter_ty {
                    Type::Range => Type::Int,
                    Type::List(inner) => *inner,
                    Type::Unknown => Type::Unknown,
                    other => {
                        self.error(format!("cannot iterate over {}", other), &iter.span);
                        Type::Unknown
                    }
                };
                self.scopes.push();
                self.scopes.declare(var, Symbol { ty: var_ty, mutable: false });
                self.loop_depth += 1;
                for s in body {
                    self.check_stmt(s);
                }
                self.loop_depth -= 1;
                self.scopes.pop();
            }
            StmtKind::Function(_) => {
                self.error("function declarations are not supported by the type checker yet", &stmt.span);
            }
            StmtKind::Struct(_) => {
                self.error("struct declarations are not supported by the type checker yet", &stmt.span);
            }
        }
    }

    // ---------- expressions ----------

    fn check_expr(&mut self, expr: &Expr) -> Type {
        match &expr.kind {
            ExprKind::Int(_) => Type::Int,
            ExprKind::Float(_) => Type::Float,
            ExprKind::Str(_) => Type::String,
            ExprKind::Bool(_) => Type::Bool,
            ExprKind::NoneLit => Type::None,
            ExprKind::Ident(name) => {
                let found = self.scopes.lookup(name).map(|s| s.ty.clone());
                match found {
                    Some(ty) => ty,
                    None => {
                        self.error(format!("undefined variable '{}'", name), &expr.span);
                        Type::Unknown
                    }
                }
            }
            ExprKind::Unary { op, operand } => {
                let ty = self.check_expr(operand);
                match op {
                    UnaryOp::Neg => match ty {
                        Type::Int | Type::Float | Type::Unknown => ty,
                        other => {
                            self.error(format!("cannot negate {}", other), &expr.span);
                            Type::Unknown
                        }
                    },
                    UnaryOp::Not => match ty {
                        Type::Bool | Type::Unknown => Type::Bool,
                        other => {
                            self.error(format!("'not' requires Bool, found {}", other), &expr.span);
                            Type::Bool
                        }
                    },
                }
            }
            ExprKind::Binary { op, left, right } => {
                let lt = self.check_expr(left);
                let rt = self.check_expr(right);
                self.check_binary(op, &lt, &rt, &expr.span)
            }
            ExprKind::List(items) => {
                if items.is_empty() {
                    self.error("cannot infer the type of an empty list", &expr.span);
                    return Type::Unknown;
                }
                let first = self.check_expr(&items[0]);
                for item in &items[1..] {
                    let t = self.check_expr(item);
                    if !Type::compatible(&first, &t) {
                        self.error(
                            format!("list elements must have the same type: expected {}, found {}", first, t),
                            &item.span,
                        );
                    }
                }
                Type::List(Box::new(first))
            }
            ExprKind::Index { object, index } => {
                let obj = self.check_expr(object);
                let idx = self.check_expr(index);
                if !Type::compatible(&Type::Int, &idx) {
                    self.error(format!("list index must be Int, found {}", idx), &index.span);
                }
                match obj {
                    Type::List(inner) => *inner,
                    Type::Unknown => Type::Unknown,
                    other => {
                        self.error(format!("cannot index into {}", other), &object.span);
                        Type::Unknown
                    }
                }
            }
            ExprKind::Call { callee, args } => {
                if let ExprKind::Ident(name) = &callee.kind {
                    if name.as_str() == "print" {
                        for a in args {
                            self.check_expr(&a.value);
                        }
                        return Type::None;
                    }
                }
                self.error("only the built-in 'print' can be called for now", &expr.span);
                for a in args {
                    self.check_expr(&a.value);
                }
                Type::Unknown
            }
            ExprKind::Field { object, .. } => {
                self.check_expr(object);
                self.error("field access is not supported by the type checker yet", &expr.span);
                Type::Unknown
            }
        }
    }

    fn check_binary(&mut self, op: &BinaryOp, left: &Type, right: &Type, span: &Span) -> Type {
        if *left == Type::Unknown || *right == Type::Unknown {
            return Type::Unknown;
        }
        let numeric = |t: &Type| matches!(t, Type::Int | Type::Float);
        match op {
            BinaryOp::Add => {
                if left == right && (numeric(left) || *left == Type::String) {
                    left.clone()
                } else {
                    self.error(format!("cannot add {} and {}", left, right), span);
                    Type::Unknown
                }
            }
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                if left == right && numeric(left) {
                    left.clone()
                } else {
                    self.error(
                        format!("arithmetic needs two numbers of the same type, found {} and {}", left, right),
                        span,
                    );
                    Type::Unknown
                }
            }
            BinaryOp::Eq | BinaryOp::NotEq => {
                if left != right {
                    self.error(format!("cannot compare {} with {}", left, right), span);
                }
                Type::Bool
            }
            BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => {
                if !(left == right && numeric(left)) {
                    self.error(
                        format!("comparison needs two numbers of the same type, found {} and {}", left, right),
                        span,
                    );
                }
                Type::Bool
            }
            BinaryOp::And | BinaryOp::Or => {
                if *left != Type::Bool || *right != Type::Bool {
                    self.error(
                        format!("'and'/'or' need Bool operands, found {} and {}", left, right),
                        span,
                    );
                }
                Type::Bool
            }
            BinaryOp::Range => {
                if *left == Type::Int && *right == Type::Int {
                    Type::Range
                } else {
                    self.error(
                        format!("range needs Int bounds, found {} and {}", left, right),
                        span,
                    );
                    Type::Unknown
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::lexer::Lexer;
    use crate::compiler::parser::Parser;

    fn check(src: &str) -> Result<(), Vec<Diagnostic>> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse_program().unwrap();
        Checker::new().check_program(&program)
    }

    fn first_error(src: &str) -> String {
        check(src).unwrap_err()[0].message.clone()
    }

    #[test]
    fn valid_program() {
        assert!(check("let x = 1\nlet y: Float = 2.5\nmut c: Int = 0\nc = c + 1\nprint(x)\n").is_ok());
    }

    #[test]
    fn type_mismatch() {
        assert_eq!(first_error("let x: Int = \"hello\"\n"), "type mismatch: expected Int, found String");
    }

    #[test]
    fn undefined_variable() {
        assert_eq!(first_error("print(y)\n"), "undefined variable 'y'");
    }

    #[test]
    fn immutable_assignment() {
        assert!(first_error("let x = 1\nx = 2\n").contains("immutable"));
    }

    #[test]
    fn redeclaration_in_same_scope() {
        assert!(first_error("let x = 1\nlet x = 2\n").contains("already declared"));
    }

    #[test]
    fn shadowing_in_inner_scope_is_ok() {
        assert!(check("let x = 1\nif true:\n    let x = \"a\"\n    print(x)\n").is_ok());
    }

    #[test]
    fn variable_not_visible_after_block() {
        assert_eq!(
            first_error("if true:\n    let inner = 1\nprint(inner)\n"),
            "undefined variable 'inner'"
        );
    }

    #[test]
    fn condition_must_be_bool() {
        assert!(first_error("if 1:\n    print(1)\n").contains("condition must be Bool"));
    }

    #[test]
    fn for_over_range_and_list() {
        assert!(check("for i in 0..10:\n    print(i + 1)\n").is_ok());
        assert!(check("for n in [1, 2, 3]:\n    print(n)\n").is_ok());
    }

    #[test]
    fn mixed_arithmetic_is_error() {
        assert_eq!(first_error("let a = 1 + 2.5\n"), "cannot add Int and Float");
    }

    #[test]
    fn break_outside_loop() {
        assert!(first_error("break\n").contains("outside of a loop"));
    }

    #[test]
    fn unknown_type_name() {
        assert_eq!(first_error("let x: Foo = 1\n"), "unknown type 'Foo'");
    }

    #[test]
    fn mixed_list_is_error() {
        assert!(first_error("let xs = [1, \"a\"]\n").contains("same type"));
    }

    #[test]
    fn while_loop_ok() {
        assert!(check("mut i: Int = 0\nwhile i < 3:\n    i = i + 1\n").is_ok());
    }
}

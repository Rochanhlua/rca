use super::scope::{ScopeStack, Symbol};
use crate::compiler::ast::*;
use crate::compiler::diagnostics::Diagnostic;
use crate::compiler::lexer::token::Span;
use crate::compiler::types::Type;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
struct ParamSig {
    name: String,
    ty: Type,
    has_default: bool,
}

#[derive(Clone)]
struct FuncSig {
    params: Vec<ParamSig>,
    ret: Type,
}

pub struct Checker {
    scopes: ScopeStack,
    errors: Vec<Diagnostic>,
    loop_depth: usize,
    structs: HashMap<String, Vec<(String, Type)>>,
    functions: HashMap<String, FuncSig>,
    current_return: Option<Type>,
}

impl Checker {
    pub fn new() -> Self {
        Checker {
            scopes: ScopeStack::new(),
            errors: Vec::new(),
            loop_depth: 0,
            structs: HashMap::new(),
            functions: HashMap::new(),
            current_return: None,
        }
    }

    pub fn check_program(mut self, program: &Program) -> Result<(), Vec<Diagnostic>> {
        self.collect_declarations(program);
        for stmt in &program.statements {
            match &stmt.kind {
                StmtKind::Function(f) => self.check_function(f, &stmt.span),
                StmtKind::Struct(_) => {}
                _ => self.check_stmt(stmt),
            }
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

    // ---------- declarations (structs and functions) ----------

    fn resolve_type(&mut self, t: &TypeExpr, report: bool) -> Type {
        if let Some(ty) = Type::from_name(&t.name) {
            return ty;
        }
        if self.structs.contains_key(&t.name) {
            return Type::Struct(t.name.clone());
        }
        if report {
            self.error(format!("unknown type '{}'", t.name), &t.span);
        }
        Type::Unknown
    }

    fn collect_declarations(&mut self, program: &Program) {
        // pass 1: struct names
        for stmt in &program.statements {
            if let StmtKind::Struct(s) = &stmt.kind {
                if self.structs.contains_key(&s.name) {
                    self.error(format!("'{}' is already defined", s.name), &stmt.span);
                } else {
                    self.structs.insert(s.name.clone(), Vec::new());
                }
            }
        }
        // pass 2: struct fields
        let mut seen = HashSet::new();
        for stmt in &program.statements {
            if let StmtKind::Struct(s) = &stmt.kind {
                if !seen.insert(s.name.clone()) {
                    continue;
                }
                let mut fields: Vec<(String, Type)> = Vec::new();
                for f in &s.fields {
                    let ty = self.resolve_type(&f.ty, true);
                    if fields.iter().any(|(n, _)| *n == f.name) {
                        self.error(
                            format!("duplicate field '{}' in struct '{}'", f.name, s.name),
                            &f.span,
                        );
                    } else {
                        fields.push((f.name.clone(), ty));
                    }
                }
                self.structs.insert(s.name.clone(), fields);
            }
        }
        // pass 3: function signatures
        for stmt in &program.statements {
            if let StmtKind::Function(f) = &stmt.kind {
                if matches!(f.name.as_str(), "print" | "len" | "str" | "int" | "float")
                    || self.functions.contains_key(&f.name)
                    || self.structs.contains_key(&f.name)
                {
                    self.error(format!("'{}' is already defined", f.name), &stmt.span);
                    continue;
                }
                let mut params = Vec::new();
                for p in &f.params {
                    let ty = self.resolve_type(&p.ty, true);
                    params.push(ParamSig {
                        name: p.name.clone(),
                        ty,
                        has_default: p.default.is_some(),
                    });
                }
                let ret = match &f.return_type {
                    Some(t) => self.resolve_type(t, true),
                    None => Type::None,
                };
                self.functions.insert(f.name.clone(), FuncSig { params, ret });
            }
        }
    }

    fn always_returns(stmts: &[Stmt]) -> bool {
        stmts.iter().any(|s| match &s.kind {
            StmtKind::Return(_) => true,
            StmtKind::If { then_block, else_block: Some(else_b), .. } => {
                Self::always_returns(then_block) && Self::always_returns(else_b)
            }
            _ => false,
        })
    }

    fn check_function(&mut self, f: &FunctionDecl, span: &Span) {
        let ret = match &f.return_type {
            Some(t) => self.resolve_type(t, false),
            None => Type::None,
        };
        self.scopes.push();
        for p in &f.params {
            let ty = self.resolve_type(&p.ty, false);
            if let Some(def) = &p.default {
                let dt = self.check_expr(def);
                if !Type::compatible(&ty, &dt) {
                    self.error(
                        format!("default value mismatch: expected {}, found {}", ty, dt),
                        &def.span,
                    );
                }
            }
            if !self.scopes.declare(&p.name, Symbol { ty, mutable: false }) {
                self.error(format!("duplicate parameter '{}'", p.name), &p.ty.span);
            }
        }
        let saved_return = self.current_return.take();
        self.current_return = Some(ret.clone());
        let saved_loop = self.loop_depth;
        self.loop_depth = 0;
        for s in &f.body {
            self.check_stmt(s);
        }
        self.loop_depth = saved_loop;
        self.current_return = saved_return;
        if ret != Type::None && ret != Type::Unknown && !Self::always_returns(&f.body) {
            self.error(
                format!(
                    "function '{}' must return a value of type {} on all paths",
                    f.name, ret
                ),
                span,
            );
        }
        self.scopes.pop();
    }

    // ---------- statements ----------

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

    fn root_ident(expr: &Expr) -> Option<&str> {
        match &expr.kind {
            ExprKind::Ident(n) => Some(n.as_str()),
            ExprKind::Field { object, .. } | ExprKind::Index { object, .. } => {
                Self::root_ident(object)
            }
            _ => None,
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Let { name, mutable, ty, value } => {
                let value_ty = self.check_expr(value);
                let var_ty = match ty {
                    Some(annotation) => {
                        let declared = self.resolve_type(annotation, true);
                        if !Type::compatible(&declared, &value_ty) {
                            self.error(
                                format!("type mismatch: expected {}, found {}", declared, value_ty),
                                &value.span,
                            );
                        }
                        declared
                    }
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
                    ExprKind::Field { .. } | ExprKind::Index { .. } => {
                        let target_ty = self.check_expr(target);
                        if let Some(root) = Self::root_ident(target).map(|n| n.to_string()) {
                            let mutable = self.scopes.lookup(&root).map(|s| s.mutable);
                            if mutable == Some(false) {
                                self.error(
                                    format!(
                                        "cannot assign through immutable variable '{}' (declare it with 'mut')",
                                        root
                                    ),
                                    &target.span,
                                );
                            }
                        }
                        if !Type::compatible(&target_ty, &value_ty) {
                            self.error(
                                format!("type mismatch: expected {}, found {}", target_ty, value_ty),
                                &value.span,
                            );
                        }
                    }
                    _ => {
                        self.error("invalid assignment target", &target.span);
                    }
                }
            }
            StmtKind::Expr(Expr { kind: ExprKind::Call { callee, args }, .. })
                if matches!(&callee.kind, ExprKind::Field { name, .. } if name == "push") =>
            {
                if let ExprKind::Field { object, .. } = &callee.kind {
                    if args.len() != 1 {
                        self.error("push() takes exactly 1 argument", &stmt.span);
                        self.check_arg_exprs(args);
                    } else {
                        let list_ty = self.check_expr(object);
                        let val_ty = self.check_expr(&args[0].value);
                        match list_ty {
                            Type::List(inner) => {
                                if !Type::compatible(&inner, &val_ty) {
                                    self.error(
                                        format!("type mismatch: expected {}, found {}", inner, val_ty),
                                        &args[0].value.span,
                                    );
                                }
                            }
                            Type::Unknown => {}
                            other => self.error(
                                format!("push() needs a List, found {}", other),
                                &object.span,
                            ),
                        }
                        if let Some(root) = Self::root_ident(object).map(|n| n.to_string()) {
                            if self.scopes.lookup(&root).map(|s| s.mutable) == Some(false) {
                                self.error(
                                    format!("cannot push to immutable variable '{}' (declare it with 'mut')", root),
                                    &object.span,
                                );
                            }
                        }
                    }
                }
            }
            StmtKind::Expr(e) => {
                self.check_expr(e);
            }
            StmtKind::Return(value) => match self.current_return.clone() {
                None => {
                    self.error("'return' outside of a function", &stmt.span);
                }
                Some(expected) => match value {
                    Some(e) => {
                        let t = self.check_expr(e);
                        if !Type::compatible(&expected, &t) {
                            self.error(
                                format!("return type mismatch: expected {}, found {}", expected, t),
                                &e.span,
                            );
                        }
                    }
                    None => {
                        if expected != Type::None && expected != Type::Unknown {
                            self.error(
                                format!("'return' needs a value of type {}", expected),
                                &stmt.span,
                            );
                        }
                    }
                },
            },
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
            StmtKind::Try { body, catch_var, catch_block } => {
                self.check_block(body);
                self.scopes.push();
                self.scopes.declare(catch_var, Symbol { ty: Type::String, mutable: false });
                for s in catch_block {
                    self.check_stmt(s);
                }
                self.scopes.pop();
            }
            StmtKind::Throw(e) => {
                let t = self.check_expr(e);
                if t != Type::String && t != Type::Unknown {
                    self.error(format!("can only throw a String, found {}", t), &e.span);
                }
            }
            StmtKind::Function(_) => {
                self.error("functions can only be declared at the top level for now", &stmt.span);
            }
            StmtKind::Struct(_) => {
                self.error("structs can only be declared at the top level for now", &stmt.span);
            }
        }
    }

    // ---------- expressions ----------

    fn check_arg_exprs(&mut self, args: &[Argument]) {
        for a in args {
            self.check_expr(&a.value);
        }
    }

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
            ExprKind::Call { callee, args } => self.check_call(callee, args, &expr.span),
            ExprKind::Field { object, name } => {
                let obj = self.check_expr(object);
                match obj {
                    Type::Struct(sname) => {
                        let field_ty = self
                            .structs
                            .get(&sname)
                            .and_then(|fs| fs.iter().find(|f| &f.0 == name))
                            .map(|f| f.1.clone());
                        match field_ty {
                            Some(t) => t,
                            None => {
                                self.error(
                                    format!("struct '{}' has no field '{}'", sname, name),
                                    &expr.span,
                                );
                                Type::Unknown
                            }
                        }
                    }
                    Type::Unknown => Type::Unknown,
                    other => {
                        self.error(format!("{} has no fields", other), &expr.span);
                        Type::Unknown
                    }
                }
            }
        }
    }

    fn check_call(&mut self, callee: &Expr, args: &[Argument], span: &Span) -> Type {
        let name = match &callee.kind {
            ExprKind::Ident(n) => n.clone(),
            _ => {
                self.error(
                    "only functions and struct constructors can be called for now",
                    &callee.span,
                );
                self.check_arg_exprs(args);
                return Type::Unknown;
            }
        };

        if name == "print" {
            self.check_arg_exprs(args);
            return Type::None;
        }

        if name == "str" || name == "int" || name == "float" {
            if args.len() != 1 {
                self.error(format!("{}() takes exactly 1 argument", name), span);
                self.check_arg_exprs(args);
            } else {
                let t = self.check_expr(&args[0].value);
                let ok = match name.as_str() {
                    "str" => true,
                    "int" => matches!(t, Type::Int | Type::Float | Type::Bool | Type::String | Type::Unknown),
                    _ => matches!(t, Type::Int | Type::Float | Type::String | Type::Unknown),
                };
                if !ok {
                    self.error(format!("{}() cannot convert {}", name, t), &args[0].value.span);
                }
            }
            return match name.as_str() {
                "str" => Type::String,
                "int" => Type::Int,
                _ => Type::Float,
            };
        }

        if name == "len" {
            if args.len() != 1 {
                self.error("len() takes exactly 1 argument", span);
                self.check_arg_exprs(args);
                return Type::Int;
            }
            let t = self.check_expr(&args[0].value);
            match t {
                Type::String | Type::List(_) | Type::Unknown => {}
                other => self.error(
                    format!("len() needs a String or List, found {}", other),
                    &args[0].value.span,
                ),
            }
            return Type::Int;
        }

        if let Some(sig) = self.functions.get(&name).cloned() {
            return self.check_function_call(&name, &sig, args, span);
        }

        if let Some(fields) = self.structs.get(&name).cloned() {
            let sig = FuncSig {
                params: fields
                    .into_iter()
                    .map(|(n, t)| ParamSig { name: n, ty: t, has_default: false })
                    .collect(),
                ret: Type::Struct(name.clone()),
            };
            return self.check_function_call(&name, &sig, args, span);
        }

        self.error(format!("undefined function '{}'", name), &callee.span);
        self.check_arg_exprs(args);
        Type::Unknown
    }

    fn check_function_call(
        &mut self,
        name: &str,
        sig: &FuncSig,
        args: &[Argument],
        span: &Span,
    ) -> Type {
        let mut provided = vec![false; sig.params.len()];
        let mut seen_named = false;

        for (i, arg) in args.iter().enumerate() {
            let arg_ty = self.check_expr(&arg.value);
            let index = match &arg.name {
                None => {
                    if seen_named {
                        self.error(
                            "positional argument cannot follow a named argument",
                            &arg.value.span,
                        );
                        continue;
                    }
                    if i >= sig.params.len() {
                        self.error(
                            format!(
                                "'{}' takes {} argument(s) but more were given",
                                name,
                                sig.params.len()
                            ),
                            &arg.value.span,
                        );
                        continue;
                    }
                    i
                }
                Some(n) => {
                    seen_named = true;
                    match sig.params.iter().position(|p| &p.name == n) {
                        Some(idx) => idx,
                        None => {
                            self.error(
                                format!("'{}' has no parameter named '{}'", name, n),
                                &arg.value.span,
                            );
                            continue;
                        }
                    }
                }
            };

            if provided[index] {
                self.error(
                    format!("parameter '{}' given more than once", sig.params[index].name),
                    &arg.value.span,
                );
                continue;
            }
            provided[index] = true;

            let expected = &sig.params[index].ty;
            if !Type::compatible(expected, &arg_ty) {
                self.error(
                    format!(
                        "argument '{}' of '{}': expected {}, found {}",
                        sig.params[index].name, name, expected, arg_ty
                    ),
                    &arg.value.span,
                );
            }
        }

        for (i, p) in sig.params.iter().enumerate() {
            if !provided[i] && !p.has_default {
                self.error(
                    format!("missing argument '{}' in call to '{}'", p.name, name),
                    span,
                );
            }
        }

        sig.ret.clone()
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

    // ----- variables, expressions, control flow -----

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

    // ----- functions -----

    #[test]
    fn function_call_ok() {
        assert!(check("fn add(a: Int, b: Int) -> Int:\n    return a + b\nlet r = add(1, 2)\nprint(r)\n").is_ok());
    }

    #[test]
    fn wrong_argument_type() {
        assert!(first_error("fn f(a: Int) -> Int:\n    return a\nlet r = f(\"x\")\n")
            .contains("expected Int, found String"));
    }

    #[test]
    fn too_many_arguments() {
        assert!(first_error("fn f(a: Int) -> Int:\n    return a\nlet r = f(1, 2)\n")
            .contains("takes 1 argument"));
    }

    #[test]
    fn missing_argument() {
        assert!(first_error("fn f(a: Int) -> Int:\n    return a\nlet r = f()\n")
            .contains("missing argument 'a'"));
    }

    #[test]
    fn default_and_named_arguments() {
        let src = "fn greet(name: String, times: Int = 1) -> None:\n    print(name)\ngreet(\"a\")\ngreet(\"a\", times = 3)\n";
        assert!(check(src).is_ok());
    }

    #[test]
    fn return_type_mismatch() {
        assert!(first_error("fn f() -> Int:\n    return \"x\"\n").contains("return type mismatch"));
    }

    #[test]
    fn missing_return() {
        assert!(first_error("fn f(a: Int) -> Int:\n    print(a)\n").contains("must return"));
    }

    #[test]
    fn return_in_if_else() {
        let src = "fn f(a: Int) -> Int:\n    if a > 0:\n        return 1\n    else:\n        return 2\n";
        assert!(check(src).is_ok());
    }

    #[test]
    fn return_outside_function() {
        assert!(first_error("return 1\n").contains("outside of a function"));
    }

    #[test]
    fn forward_call() {
        let src = "let r = add(1, 2)\nfn add(a: Int, b: Int) -> Int:\n    return a + b\n";
        assert!(check(src).is_ok());
    }

    #[test]
    fn recursion() {
        let src = "fn fact(n: Int) -> Int:\n    if n <= 1:\n        return 1\n    return n * fact(n - 1)\n";
        assert!(check(src).is_ok());
    }

    #[test]
    fn undefined_function() {
        assert!(first_error("foo(1)\n").contains("undefined function 'foo'"));
    }

    #[test]
    fn duplicate_function() {
        let src = "fn f() -> None:\n    print(1)\nfn f() -> None:\n    print(2)\n";
        assert!(first_error(src).contains("already defined"));
    }

    #[test]
    fn unknown_named_argument() {
        assert!(first_error("fn f(a: Int) -> Int:\n    return a\nlet r = f(b = 1)\n")
            .contains("no parameter named 'b'"));
    }

    // ----- structs -----

    const USER: &str = "struct User:\n    name: String\n    age: Int\n";

    #[test]
    fn struct_ok() {
        let src = format!("{}let u = User(name = \"A\", age = 20)\nprint(u.name)\nlet n: Int = u.age\n", USER);
        assert!(check(&src).is_ok());
    }

    #[test]
    fn unknown_field() {
        let src = format!("{}let u = User(name = \"A\", age = 20)\nprint(u.email)\n", USER);
        assert!(first_error(&src).contains("has no field 'email'"));
    }

    #[test]
    fn constructor_wrong_type() {
        let src = format!("{}let u = User(name = 5, age = 1)\n", USER);
        assert!(first_error(&src).contains("expected String, found Int"));
    }

    #[test]
    fn field_assignment() {
        let bad = "struct P:\n    x: Int\nlet p = P(x = 1)\np.x = 2\n";
        assert!(first_error(bad).contains("immutable"));
        let good = "struct P:\n    x: Int\nmut p = P(x = 1)\np.x = 2\n";
        assert!(check(good).is_ok());
    }

    #[test]
    fn struct_as_parameter() {
        let src = "struct P:\n    x: Int\nfn get(p: P) -> Int:\n    return p.x\nlet q = P(x = 3)\nlet v = get(q)\n";
        assert!(check(src).is_ok());
    }

    #[test]
    fn index_assignment() {
        assert!(check("mut xs = [1, 2]\nxs[0] = 5\n").is_ok());
    }

    #[test]
    fn try_catch_valid() {
        let src = "try:\n    print(1)\ncatch e:\n    print(e)\n";
        assert!(check(src).is_ok());
    }

    #[test]
    fn throw_requires_string() {
        assert!(first_error("throw 5\n").contains("can only throw a String"));
        assert!(check("throw \"boom\"\n").is_ok());
    }

    #[test]
    fn catch_variable_is_scoped() {
        let src = "try:\n    print(1)\ncatch e:\n    print(e)\nprint(e)\n";
        assert!(first_error(src).contains("undefined variable"));
    }

    #[test]
    fn len_checks_argument() {
        assert!(check("print(len([1, 2]))\nprint(len(\"abc\"))\n").is_ok());
        assert!(first_error("print(len(5))\n").contains("len() needs a String or List"));
        assert!(first_error("print(len())\n").contains("exactly 1 argument"));
    }

    #[test]
    fn conversions_check_types() {
        assert!(check("print(str(1))\nprint(int(\"5\"))\nprint(float(2))\n").is_ok());
        assert!(first_error("print(int([1]))\n").contains("cannot convert"));
        assert!(first_error("print(str())\n").contains("exactly 1 argument"));
        let r = check("let x: Int = str(5)\n");
        assert!(r.is_err());
    }

    #[test]
    fn push_checks() {
        assert!(check("mut xs = [1]\nxs.push(2)\n").is_ok());
        assert!(first_error("let xs = [1]\nxs.push(2)\n").contains("immutable"));
        assert!(first_error("mut xs = [1]\nxs.push(\"a\")\n").contains("type mismatch"));
        assert!(first_error("mut n = 5\nn.push(1)\n").contains("push() needs a List"));
    }
}

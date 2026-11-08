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
    enums: HashMap<String, Vec<String>>,
    consts: HashSet<String>,
    block_depth: usize,
    functions: HashMap<String, FuncSig>,
    current_return: Option<Type>,
    imports: HashMap<String, ModuleExports>,
    module_aliases: HashMap<String, String>,
}

impl Checker {
    pub fn new() -> Self {
        Checker {
            scopes: ScopeStack::new(),
            errors: Vec::new(),
            loop_depth: 0,
            structs: HashMap::new(),
            enums: HashMap::new(),
            consts: HashSet::new(),
            block_depth: 0,
            functions: HashMap::new(),
            current_return: None,
            imports: HashMap::new(),
            module_aliases: HashMap::new(),
        }
    }

    pub fn check_program(mut self, program: &Program) -> Result<(), Vec<Diagnostic>> {
        self.apply_imports(program);
        self.collect_declarations(program);
        for stmt in &program.statements {
            match &stmt.kind {
                StmtKind::Function(f) => {
                    self.block_depth += 1;
                    self.check_function(f, &stmt.span);
                    self.block_depth -= 1;
                }
                StmtKind::Struct(_) => {}
                StmtKind::Enum(_) => {}
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
        if self.enums.contains_key(&t.name) {
            return Type::Enum(t.name.clone());
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
        // pass 0: enums
        for stmt in &program.statements {
            if let StmtKind::Enum(e) = &stmt.kind {
                if self.enums.contains_key(&e.name) {
                    self.error(format!("'{}' is already defined", e.name), &stmt.span);
                    continue;
                }
                let mut names: Vec<String> = Vec::new();
                for v in &e.variants {
                    if names.contains(&v.name) {
                        self.error(
                            format!("duplicate variant '{}' in enum '{}'", v.name, e.name),
                            &v.span,
                        );
                    } else {
                        names.push(v.name.clone());
                    }
                }
                self.enums.insert(e.name.clone(), names);
            }
        }
        // pass 1: struct names
        for stmt in &program.statements {
            if let StmtKind::Struct(s) = &stmt.kind {
                if self.structs.contains_key(&s.name) || self.enums.contains_key(&s.name) {
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
                self.functions
                    .insert(f.name.clone(), FuncSig { params, ret });
            }
        }
    }

    fn always_returns(stmts: &[Stmt]) -> bool {
        stmts.iter().any(|s| match &s.kind {
            StmtKind::Return(_) => true,
            StmtKind::If {
                then_block,
                else_block: Some(else_b),
                ..
            } => Self::always_returns(then_block) && Self::always_returns(else_b),
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

    fn check_match(
        &mut self,
        subject: &Expr,
        arms: &[crate::compiler::ast::MatchArm],
        span: &Span,
    ) {
        use crate::compiler::ast::Pattern;
        let st = self.check_expr(subject);
        let valid = matches!(
            st,
            Type::Int | Type::String | Type::Bool | Type::Enum(_) | Type::Unknown
        );
        if !valid {
            self.error(format!("cannot match on {}", st), &subject.span);
        }
        let mut seen: Vec<String> = Vec::new();
        let mut has_wildcard = false;
        for arm in arms {
            if has_wildcard {
                self.error("unreachable match arm after '_'", &arm.span);
            }
            match &arm.pattern {
                Pattern::Wildcard => has_wildcard = true,
                Pattern::Value(e) => {
                    let key = match &e.kind {
                        ExprKind::Int(n) => Some(format!("{}", n)),
                        ExprKind::Str(s) => Some(format!("\"{}\"", s)),
                        ExprKind::Bool(b) => Some(format!("{}", b)),
                        ExprKind::Field { object, name } => match &object.kind {
                            ExprKind::Ident(en)
                                if self.enums.contains_key(en)
                                    && self.scopes.lookup(en).is_none() =>
                            {
                                Some(format!("{}.{}", en, name))
                            }
                            _ => None,
                        },
                        _ => None,
                    };
                    match key {
                        None => self.error(
                            "a match pattern must be a literal, an enum variant or '_'",
                            &e.span,
                        ),
                        Some(k) => {
                            let pt = self.check_expr(e);
                            if st != Type::Unknown
                                && pt != Type::Unknown
                                && !Type::compatible(&st, &pt)
                            {
                                self.error(
                                    format!(
                                        "match pattern type mismatch: expected {}, found {}",
                                        st, pt
                                    ),
                                    &e.span,
                                );
                            }
                            if seen.contains(&k) {
                                self.error(format!("duplicate match pattern {}", k), &e.span);
                            } else {
                                seen.push(k);
                            }
                        }
                    }
                }
            }
            self.check_block(&arm.body);
        }
        if has_wildcard || !valid || st == Type::Unknown {
            return;
        }
        match &st {
            Type::Enum(name) => {
                let missing: Vec<String> = self.enums[name]
                    .iter()
                    .filter(|v| !seen.contains(&format!("{}.{}", name, v)))
                    .cloned()
                    .collect();
                if !missing.is_empty() {
                    self.error(
                        format!(
                            "match on enum '{}' is not exhaustive, missing: {}",
                            name,
                            missing.join(", ")
                        ),
                        span,
                    );
                }
            }
            Type::Bool => {
                if !(seen.contains(&"true".to_string()) && seen.contains(&"false".to_string())) {
                    self.error("match on Bool must cover true and false, or use '_'", span);
                }
            }
            _ => self.error(format!("match on {} needs a '_' arm", st), span),
        }
    }

    fn is_const_expr(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Int(_) | ExprKind::Float(_) | ExprKind::Str(_) | ExprKind::Bool(_) => true,
            ExprKind::Unary { operand, .. } => self.is_const_expr(operand),
            ExprKind::Binary { left, right, .. } => {
                self.is_const_expr(left) && self.is_const_expr(right)
            }
            ExprKind::Ident(n) => self.consts.contains(n),
            _ => false,
        }
    }

    fn check_block(&mut self, stmts: &[Stmt]) {
        self.block_depth += 1;
        self.check_block_inner(stmts);
        self.block_depth -= 1;
    }

    fn check_block_inner(&mut self, stmts: &[Stmt]) {
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
            StmtKind::Let {
                name,
                mutable,
                constant,
                ty,
                value,
            } => {
                let value_ty = self.check_expr(value);
                if *constant {
                    if self.block_depth > 0 {
                        self.error(
                            "constants can only be declared at the top level",
                            &stmt.span,
                        );
                    }
                    if !self.is_const_expr(value) {
                        self.error(
                            "a constant value must be a literal or other constants",
                            &value.span,
                        );
                    }
                    self.consts.insert(name.clone());
                }
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
                let symbol = Symbol {
                    ty: var_ty,
                    mutable: *mutable,
                };
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
                                if !mutable && self.consts.contains(name) {
                                    self.error(
                                        format!("cannot assign to constant '{}'", name),
                                        &target.span,
                                    );
                                } else if !mutable {
                                    self.error(
                                        format!(
                                            "cannot assign to immutable variable '{}' (declare it with 'mut')",
                                            name
                                        ),
                                        &target.span,
                                    );
                                } else if !Type::compatible(&var_ty, &value_ty) {
                                    self.error(
                                        format!(
                                            "type mismatch: expected {}, found {}",
                                            var_ty, value_ty
                                        ),
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
                                format!(
                                    "type mismatch: expected {}, found {}",
                                    target_ty, value_ty
                                ),
                                &value.span,
                            );
                        }
                    }
                    _ => {
                        self.error("invalid assignment target", &target.span);
                    }
                }
            }
            StmtKind::Expr(Expr {
                kind: ExprKind::Call { callee, args },
                ..
            }) if matches!(&callee.kind, ExprKind::Field { name, .. } if name == "push") => {
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
                                        format!(
                                            "type mismatch: expected {}, found {}",
                                            inner, val_ty
                                        ),
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
            StmtKind::If {
                condition,
                then_block,
                else_block,
            } => {
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
                self.scopes.declare(
                    var,
                    Symbol {
                        ty: var_ty,
                        mutable: false,
                    },
                );
                self.loop_depth += 1;
                for s in body {
                    self.check_stmt(s);
                }
                self.loop_depth -= 1;
                self.scopes.pop();
            }
            StmtKind::Try {
                body,
                catch_var,
                catch_block,
            } => {
                self.check_block(body);
                self.scopes.push();
                self.scopes.declare(
                    catch_var,
                    Symbol {
                        ty: Type::String,
                        mutable: false,
                    },
                );
                for s in catch_block {
                    self.check_stmt(s);
                }
                self.scopes.pop();
            }
            StmtKind::Import { .. } | StmtKind::FromImport { .. } => {
                // imports are resolved by apply_imports before checking
            }
            StmtKind::Throw(e) => {
                let t = self.check_expr(e);
                if t != Type::String && t != Type::Unknown {
                    self.error(format!("can only throw a String, found {}", t), &e.span);
                }
            }
            StmtKind::Function(_) => {
                self.error(
                    "functions can only be declared at the top level for now",
                    &stmt.span,
                );
            }
            StmtKind::Struct(_) => {
                self.error(
                    "structs can only be declared at the top level for now",
                    &stmt.span,
                );
            }
            StmtKind::Match { subject, arms } => {
                self.check_match(subject, arms, &stmt.span);
            }
            StmtKind::Enum(_) => {
                self.error(
                    "enums can only be declared at the top level for now",
                    &stmt.span,
                );
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
                            format!(
                                "list elements must have the same type: expected {}, found {}",
                                first, t
                            ),
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
                    self.error(
                        format!("list index must be Int, found {}", idx),
                        &index.span,
                    );
                }
                match obj {
                    Type::List(inner) => *inner,
                    Type::Tensor => Type::Unknown,
                    Type::Unknown => Type::Unknown,
                    other => {
                        self.error(format!("cannot index into {}", other), &object.span);
                        Type::Unknown
                    }
                }
            }
            ExprKind::Call { callee, args } => self.check_call(callee, args, &expr.span),
            ExprKind::Field { object, name } => {
                if let ExprKind::Ident(en) = &object.kind {
                    if self.scopes.lookup(en).is_none() && self.enums.contains_key(en) {
                        let found = self.enums[en].iter().any(|v| v == name);
                        if found {
                            return Type::Enum(en.clone());
                        }
                        self.error(
                            format!("enum '{}' has no variant '{}'", en, name),
                            &expr.span,
                        );
                        return Type::Unknown;
                    }
                }
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
        if let ExprKind::Field { object, name: item } = &callee.kind {
            if let ExprKind::Ident(m) = &object.kind {
                if self.scopes.lookup(m).is_none() {
                    if let Some(module) = self.module_aliases.get(m).cloned() {
                        return self.check_module_call(&module, item, args, span, &callee.span);
                    }
                }
            }
        }
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

        if name == "matmul" {
            if args.len() != 2 {
                self.error("matmul() takes exactly 2 arguments".to_string(), span);
                self.check_arg_exprs(args);
                return Type::Tensor;
            }
            for a in args.iter() {
                let t = self.check_expr(&a.value);
                if !matches!(t, Type::Tensor | Type::Unknown) {
                    self.error(
                        format!("matmul() needs Tensor arguments, found {}", t),
                        &a.value.span,
                    );
                }
            }
            return Type::Tensor;
        }

        if matches!(name.as_str(), "sum" | "mean" | "max" | "min" | "transpose") {
            let ret = if name == "transpose" {
                Type::Tensor
            } else {
                Type::Float
            };
            if args.len() != 1 {
                self.error(format!("{}() takes exactly 1 argument", name), span);
                self.check_arg_exprs(args);
                return ret;
            }
            let t = self.check_expr(&args[0].value);
            if !matches!(t, Type::Tensor | Type::Unknown) {
                self.error(
                    format!("{}() needs a Tensor, found {}", name, t),
                    &args[0].value.span,
                );
            }
            return ret;
        }

        if name == "reshape" {
            if args.len() != 2 {
                self.error("reshape() takes exactly 2 arguments".to_string(), span);
                self.check_arg_exprs(args);
                return Type::Tensor;
            }
            let t = self.check_expr(&args[0].value);
            if !matches!(t, Type::Tensor | Type::Unknown) {
                self.error(
                    format!("reshape() needs a Tensor, found {}", t),
                    &args[0].value.span,
                );
            }
            let s = self.check_expr(&args[1].value);
            let ok = match &s {
                Type::List(inner) => matches!(**inner, Type::Int | Type::Unknown),
                Type::Unknown => true,
                _ => false,
            };
            if !ok {
                self.error(
                    format!("reshape() needs a List of Int (the shape), found {}", s),
                    &args[1].value.span,
                );
            }
            return Type::Tensor;
        }

        if name == "Tensor" || name == "zeros" || name == "ones" || name == "shape" {
            if args.len() != 1 {
                self.error(format!("{}() takes exactly 1 argument", name), span);
                self.check_arg_exprs(args);
                return if name == "shape" {
                    Type::List(Box::new(Type::Int))
                } else {
                    Type::Tensor
                };
            }
            let t = self.check_expr(&args[0].value);
            let arg_span = args[0].value.span.clone();
            match name.as_str() {
                "Tensor" => {
                    let ok = match &t {
                        Type::List(inner) => tensor_elem_ok(inner),
                        Type::Unknown => true,
                        _ => false,
                    };
                    if !ok {
                        self.error(
                            format!("Tensor() needs a List of Float values, found {}", t),
                            &arg_span,
                        );
                    }
                    return Type::Tensor;
                }
                "zeros" | "ones" => {
                    let ok = match &t {
                        Type::List(inner) => matches!(**inner, Type::Int | Type::Unknown),
                        Type::Unknown => true,
                        _ => false,
                    };
                    if !ok {
                        self.error(
                            format!("{}() needs a List of Int (the shape), found {}", name, t),
                            &arg_span,
                        );
                    }
                    return Type::Tensor;
                }
                _ => {
                    if !matches!(t, Type::Tensor | Type::Unknown) {
                        self.error(format!("shape() needs a Tensor, found {}", t), &arg_span);
                    }
                    return Type::List(Box::new(Type::Int));
                }
            }
        }

        if name == "str" || name == "int" || name == "float" {
            if args.len() != 1 {
                self.error(format!("{}() takes exactly 1 argument", name), span);
                self.check_arg_exprs(args);
            } else {
                let t = self.check_expr(&args[0].value);
                let ok = match name.as_str() {
                    "str" => true,
                    "int" => matches!(
                        t,
                        Type::Int | Type::Float | Type::Bool | Type::String | Type::Unknown
                    ),
                    _ => matches!(t, Type::Int | Type::Float | Type::String | Type::Unknown),
                };
                if !ok {
                    self.error(
                        format!("{}() cannot convert {}", name, t),
                        &args[0].value.span,
                    );
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
                Type::String | Type::List(_) | Type::Tensor | Type::Unknown => {}
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
                    .map(|(n, t)| ParamSig {
                        name: n,
                        ty: t,
                        has_default: false,
                    })
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
                    format!(
                        "parameter '{}' given more than once",
                        sig.params[index].name
                    ),
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
        if matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
        ) && (*left == Type::Tensor || *right == Type::Tensor)
        {
            let ok = matches!(
                (left, right),
                (Type::Tensor, Type::Tensor)
                    | (Type::Tensor, Type::Float)
                    | (Type::Float, Type::Tensor)
            );
            if ok {
                return Type::Tensor;
            }
            self.error(
                format!(
                    "tensor arithmetic needs a Tensor with a Tensor or a Float, found {} and {}",
                    left, right
                ),
                span,
            );
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
                        format!(
                            "arithmetic needs two numbers of the same type, found {} and {}",
                            left, right
                        ),
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
                        format!(
                            "comparison needs two numbers of the same type, found {} and {}",
                            left, right
                        ),
                        span,
                    );
                }
                Type::Bool
            }
            BinaryOp::And | BinaryOp::Or => {
                if *left != Type::Bool || *right != Type::Bool {
                    self.error(
                        format!(
                            "'and'/'or' need Bool operands, found {} and {}",
                            left, right
                        ),
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
        assert!(
            check("let x = 1\nlet y: Float = 2.5\nmut c: Int = 0\nc = c + 1\nprint(x)\n").is_ok()
        );
    }

    #[test]
    fn type_mismatch() {
        assert_eq!(
            first_error("let x: Int = \"hello\"\n"),
            "type mismatch: expected Int, found String"
        );
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
        assert!(
            check(
                "fn add(a: Int, b: Int) -> Int:\n    return a + b\nlet r = add(1, 2)\nprint(r)\n"
            )
            .is_ok()
        );
    }

    #[test]
    fn wrong_argument_type() {
        assert!(
            first_error("fn f(a: Int) -> Int:\n    return a\nlet r = f(\"x\")\n")
                .contains("expected Int, found String")
        );
    }

    #[test]
    fn too_many_arguments() {
        assert!(
            first_error("fn f(a: Int) -> Int:\n    return a\nlet r = f(1, 2)\n")
                .contains("takes 1 argument")
        );
    }

    #[test]
    fn missing_argument() {
        assert!(
            first_error("fn f(a: Int) -> Int:\n    return a\nlet r = f()\n")
                .contains("missing argument 'a'")
        );
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
        let src =
            "fn f(a: Int) -> Int:\n    if a > 0:\n        return 1\n    else:\n        return 2\n";
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
        assert!(
            first_error("fn f(a: Int) -> Int:\n    return a\nlet r = f(b = 1)\n")
                .contains("no parameter named 'b'")
        );
    }

    // ----- structs -----

    const USER: &str = "struct User:\n    name: String\n    age: Int\n";

    #[test]
    fn struct_ok() {
        let src = format!(
            "{}let u = User(name = \"A\", age = 20)\nprint(u.name)\nlet n: Int = u.age\n",
            USER
        );
        assert!(check(&src).is_ok());
    }

    #[test]
    fn unknown_field() {
        let src = format!(
            "{}let u = User(name = \"A\", age = 20)\nprint(u.email)\n",
            USER
        );
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

/// What a module makes available to other modules.
/// Only `pub` items are importable; private names are remembered
/// so the error message can say "private" instead of "not found".
#[derive(Clone, Default)]
pub struct ModuleExports {
    functions: HashMap<String, FuncSig>,
    structs: HashMap<String, Vec<(String, Type)>>,
    private: HashSet<String>,
}

impl Checker {
    pub fn with_imports(mut self, imports: HashMap<String, ModuleExports>) -> Self {
        self.imports = imports;
        self
    }

    /// Computes the public interface of a module (declarations only, bodies are not checked).
    pub fn collect_exports(
        program: &Program,
        imports: &HashMap<String, ModuleExports>,
    ) -> ModuleExports {
        let mut c = Checker::new().with_imports(imports.clone());
        c.apply_imports(program);
        c.collect_declarations(program);
        let mut out = ModuleExports::default();
        for stmt in &program.statements {
            match &stmt.kind {
                StmtKind::Function(f) => {
                    if f.public {
                        if let Some(sig) = c.functions.get(&f.name) {
                            out.functions.insert(f.name.clone(), sig.clone());
                        }
                    } else {
                        out.private.insert(f.name.clone());
                    }
                }
                StmtKind::Struct(s) => {
                    if s.public {
                        if let Some(fields) = c.structs.get(&s.name) {
                            out.structs.insert(s.name.clone(), fields.clone());
                        }
                    } else {
                        out.private.insert(s.name.clone());
                    }
                }
                _ => {}
            }
        }
        out
    }

    fn apply_imports(&mut self, program: &Program) {
        for stmt in &program.statements {
            match &stmt.kind {
                StmtKind::Import { module, alias } => {
                    self.module_aliases.insert(
                        alias.clone().unwrap_or_else(|| module.clone()),
                        module.clone(),
                    );
                    if !self.imports.contains_key(module) {
                        self.error(format!("cannot find module '{}'", module), &stmt.span);
                    }
                }
                StmtKind::FromImport { module, names } => {
                    let exports = match self.imports.get(module).cloned() {
                        Some(e) => e,
                        None => {
                            self.error(format!("cannot find module '{}'", module), &stmt.span);
                            continue;
                        }
                    };
                    for item in names {
                        let local = item.alias.clone().unwrap_or_else(|| item.name.clone());
                        if let Some(sig) = exports.functions.get(&item.name) {
                            self.functions.insert(local, sig.clone());
                        } else if let Some(fields) = exports.structs.get(&item.name) {
                            self.structs.insert(local, fields.clone());
                        } else if exports.private.contains(&item.name) {
                            self.error(
                                format!("'{}' is private in module '{}'", item.name, module),
                                &item.span,
                            );
                        } else {
                            self.error(
                                format!("module '{}' has no public item '{}'", module, item.name),
                                &item.span,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod import_tests {
    use super::*;
    use crate::compiler::lexer::Lexer;
    use crate::compiler::parser::Parser;

    fn parse(src: &str) -> Program {
        let tokens = Lexer::new(src).tokenize().unwrap();
        Parser::new(tokens).parse_program().unwrap()
    }

    fn errors_of(lib: &str, main: &str) -> Vec<String> {
        let exports = Checker::collect_exports(&parse(lib), &HashMap::new());
        let mut imports = HashMap::new();
        imports.insert("utils".to_string(), exports);
        match Checker::new()
            .with_imports(imports)
            .check_program(&parse(main))
        {
            Ok(()) => Vec::new(),
            Err(v) => v.iter().map(|e| e.to_string()).collect(),
        }
    }

    const ADD: &str = "pub fn add(a: Int, b: Int) -> Int:\n    return a + b\n";

    #[test]
    fn from_import_function_call_works() {
        assert!(errors_of(ADD, "from utils import add\nlet r: Int = add(1, 2)\n").is_empty());
    }

    #[test]
    fn from_import_alias_works() {
        assert!(
            errors_of(
                ADD,
                "from utils import add as plus\nlet r: Int = plus(1, 2)\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn importing_private_function_is_error() {
        let lib = "fn secret() -> Int:\n    return 1\n";
        let errs = errors_of(lib, "from utils import secret\n");
        assert!(
            errs.iter().any(|e| e.contains("private")),
            "got: {:?}",
            errs
        );
    }

    #[test]
    fn importing_missing_name_is_error() {
        let errs = errors_of(ADD, "from utils import nothing\n");
        assert!(
            errs.iter().any(|e| e.contains("no public item 'nothing'")),
            "got: {:?}",
            errs
        );
    }

    #[test]
    fn unknown_module_is_error() {
        let result = Checker::new().check_program(&parse("import other\n"));
        let errs: Vec<String> = result.unwrap_err().iter().map(|e| e.to_string()).collect();
        assert!(
            errs.iter()
                .any(|e| e.contains("cannot find module 'other'")),
            "got: {:?}",
            errs
        );
    }

    #[test]
    fn imported_function_arguments_are_type_checked() {
        let errs = errors_of(ADD, "from utils import add\nlet r = add(1, \"x\")\n");
        assert!(!errs.is_empty());
    }

    #[test]
    fn imported_struct_can_be_constructed() {
        let lib = "pub struct Point:\n    x: Int\n    y: Int\n";
        let errs = errors_of(
            lib,
            "from utils import Point\nlet p = Point(x = 1, y = 2)\n",
        );
        assert!(errs.is_empty(), "got: {:?}", errs);
    }
}

impl Checker {
    /// Checks modules in dependency order (as returned by the module loader).
    /// Each module sees the public items of the modules before it.
    /// Errors are returned together with the name of the module they belong to.
    pub fn check_modules(
        modules: &[crate::compiler::modules::Module],
    ) -> Result<(), Vec<(String, Diagnostic)>> {
        let mut exports: HashMap<String, ModuleExports> = HashMap::new();
        let mut errors: Vec<(String, Diagnostic)> = Vec::new();
        for m in modules {
            if let Err(errs) = Checker::new()
                .with_imports(exports.clone())
                .check_program(&m.program)
            {
                for e in errs {
                    errors.push((m.name.clone(), e));
                }
            }
            let ex = Checker::collect_exports(&m.program, &exports);
            exports.insert(m.name.clone(), ex);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Checker {
    /// Checks a call written as `module.item(...)`.
    fn check_module_call(
        &mut self,
        module: &str,
        item: &str,
        args: &[Argument],
        span: &Span,
        callee_span: &Span,
    ) -> Type {
        let exports = match self.imports.get(module).cloned() {
            Some(e) => e,
            None => {
                self.check_arg_exprs(args);
                return Type::Unknown;
            }
        };
        if let Some(sig) = exports.functions.get(item).cloned() {
            return self.check_function_call(&item.to_string(), &sig, args, span);
        }
        if let Some(fields) = exports.structs.get(item).cloned() {
            let sig = FuncSig {
                params: fields
                    .into_iter()
                    .map(|(n, t)| ParamSig {
                        name: n,
                        ty: t,
                        has_default: false,
                    })
                    .collect(),
                ret: Type::Struct(item.to_string()),
            };
            return self.check_function_call(&item.to_string(), &sig, args, span);
        }
        if exports.private.contains(item) {
            self.error(
                format!("'{}' is private in module '{}'", item, module),
                callee_span,
            );
        } else {
            self.error(
                format!("module '{}' has no public item '{}'", module, item),
                callee_span,
            );
        }
        self.check_arg_exprs(args);
        Type::Unknown
    }
}

#[cfg(test)]
mod qualified_tests {
    use super::*;
    use crate::compiler::lexer::Lexer;
    use crate::compiler::parser::Parser;

    fn parse(src: &str) -> Program {
        let tokens = Lexer::new(src).tokenize().unwrap();
        Parser::new(tokens).parse_program().unwrap()
    }

    fn errors_of(lib: &str, main: &str) -> Vec<String> {
        let exports = Checker::collect_exports(&parse(lib), &HashMap::new());
        let mut imports = HashMap::new();
        imports.insert("utils".to_string(), exports);
        match Checker::new()
            .with_imports(imports)
            .check_program(&parse(main))
        {
            Ok(()) => Vec::new(),
            Err(v) => v.iter().map(|e| e.to_string()).collect(),
        }
    }

    const LIB: &str = "pub fn add(a: Int, b: Int) -> Int:\n    return a + b\nfn secret() -> Int:\n    return 1\npub struct Point:\n    x: Int\n    y: Int\n";

    #[test]
    fn qualified_call_works() {
        assert!(errors_of(LIB, "import utils\nlet r: Int = utils.add(1, 2)\n").is_empty());
    }

    #[test]
    fn qualified_call_with_alias_works() {
        assert!(errors_of(LIB, "import utils as u\nlet r: Int = u.add(1, 2)\n").is_empty());
    }

    #[test]
    fn qualified_private_is_error() {
        let errs = errors_of(LIB, "import utils\nlet r = utils.secret()\n");
        assert!(
            errs.iter().any(|e| e.contains("private")),
            "got: {:?}",
            errs
        );
    }

    #[test]
    fn qualified_missing_is_error() {
        let errs = errors_of(LIB, "import utils\nlet r = utils.nothing()\n");
        assert!(
            errs.iter().any(|e| e.contains("no public item 'nothing'")),
            "got: {:?}",
            errs
        );
    }

    #[test]
    fn qualified_call_is_type_checked() {
        let errs = errors_of(LIB, "import utils\nlet r = utils.add(1, \"x\")\n");
        assert!(!errs.is_empty());
    }

    #[test]
    fn qualified_struct_constructor_works() {
        let errs = errors_of(LIB, "import utils\nlet p = utils.Point(x = 1, y = 2)\n");
        assert!(errs.is_empty(), "got: {:?}", errs);
    }
}

/// A Tensor can be built from nested lists whose innermost elements are Float.
fn tensor_elem_ok(t: &Type) -> bool {
    match t {
        Type::Float | Type::Unknown => true,
        Type::List(inner) => tensor_elem_ok(inner),
        _ => false,
    }
}

#[cfg(test)]
mod enum_match_tests {
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

    const COLOR: &str = "enum Color:\n    Red\n    Green\n";

    #[test]
    fn enum_values_and_equality_are_ok() {
        let src = format!(
            "{}let a = Color.Red\nlet b: Color = Color.Green\nlet s = a == b\n",
            COLOR
        );
        assert!(check(&src).is_ok());
    }

    #[test]
    fn unknown_variant() {
        let src = format!("{}let a = Color.Blue\n", COLOR);
        assert_eq!(first_error(&src), "enum 'Color' has no variant 'Blue'");
    }

    #[test]
    fn duplicate_variant() {
        assert_eq!(
            first_error("enum C:\n    A\n    A\n"),
            "duplicate variant 'A' in enum 'C'"
        );
    }

    #[test]
    fn exhaustive_enum_match_is_ok() {
        let src = format!(
            "{}let c = Color.Red\nmatch c:\n    Color.Red:\n        print(1)\n    Color.Green:\n        print(2)\n",
            COLOR
        );
        assert!(check(&src).is_ok());
    }

    #[test]
    fn missing_variant_is_error() {
        let src = format!(
            "{}let c = Color.Red\nmatch c:\n    Color.Red:\n        print(1)\n",
            COLOR
        );
        assert_eq!(
            first_error(&src),
            "match on enum 'Color' is not exhaustive, missing: Green"
        );
    }

    #[test]
    fn int_match_needs_wildcard() {
        assert_eq!(
            first_error("let n = 1\nmatch n:\n    1:\n        print(1)\n"),
            "match on Int needs a '_' arm"
        );
    }

    #[test]
    fn duplicate_pattern() {
        let src = "let n = 1\nmatch n:\n    1:\n        print(1)\n    1:\n        print(2)\n    _:\n        print(3)\n";
        assert_eq!(first_error(src), "duplicate match pattern 1");
    }

    #[test]
    fn pattern_type_mismatch() {
        let src = "let n = 1\nmatch n:\n    \"a\":\n        print(1)\n    _:\n        print(2)\n";
        assert_eq!(
            first_error(src),
            "match pattern type mismatch: expected Int, found String"
        );
    }

    #[test]
    fn unreachable_arm_and_float_subject() {
        let src = "let n = 1\nmatch n:\n    _:\n        print(1)\n    2:\n        print(2)\n";
        assert_eq!(first_error(src), "unreachable match arm after '_'");
        let src2 = "let f = 1.5\nmatch f:\n    _:\n        print(1)\n";
        assert_eq!(first_error(src2), "cannot match on Float");
    }
}

#[cfg(test)]
mod const_check_tests {
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
    fn valid_constants() {
        assert!(check("const A: Int = 2\nconst B = A * 3\nconst S = \"x\"\nprint(B)\n").is_ok());
    }

    #[test]
    fn assign_to_constant() {
        assert_eq!(
            first_error("const A = 1\nA = 2\n"),
            "cannot assign to constant 'A'"
        );
    }

    #[test]
    fn constant_must_be_top_level() {
        assert_eq!(
            first_error("fn f() -> Int:\n    const X = 1\n    return X\n"),
            "constants can only be declared at the top level"
        );
    }

    #[test]
    fn constant_value_must_be_constant() {
        assert_eq!(
            first_error("let a = 1\nconst Z = a\n"),
            "a constant value must be a literal or other constants"
        );
    }

    #[test]
    fn constant_type_mismatch() {
        assert_eq!(
            first_error("const A: Int = \"x\"\n"),
            "type mismatch: expected Int, found String"
        );
    }
}

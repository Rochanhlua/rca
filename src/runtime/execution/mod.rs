use crate::compiler::ast::*;
use crate::compiler::diagnostics::Diagnostic;
use crate::compiler::lexer::token::Span;
use crate::runtime::values::Value;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

const MAX_CALL_DEPTH: usize = 1000;

enum Flow {
    Normal,
    Break,
    Continue,
    Return(Value),
}

fn err(message: impl Into<String>, span: &Span) -> Diagnostic {
    Diagnostic::new(message, span.clone())
}

pub struct Interpreter {
    /// frames[0][0] holds the global variables.
    /// Each function call pushes a new frame (a stack of block scopes).
    frames: Vec<Vec<HashMap<String, Value>>>,
    functions: HashMap<String, Rc<FunctionDecl>>,
    structs: HashMap<String, Vec<String>>,
    capture: bool,
    output: String,
}

impl Interpreter {
    /// Interpreter that prints to the terminal.
    pub fn new() -> Self {
        Self::build(false)
    }

    /// Interpreter that stores printed output (used by tests).
    pub fn capturing() -> Self {
        Self::build(true)
    }

    fn build(capture: bool) -> Self {
        Interpreter {
            frames: vec![vec![HashMap::new()]],
            functions: HashMap::new(),
            structs: HashMap::new(),
            capture,
            output: String::new(),
        }
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    pub fn run(&mut self, program: &Program) -> Result<(), Diagnostic> {
        for stmt in &program.statements {
            match &stmt.kind {
                StmtKind::Function(f) => {
                    self.functions.insert(f.name.clone(), Rc::new(f.clone()));
                }
                StmtKind::Struct(s) => {
                    let names = s.fields.iter().map(|f| f.name.clone()).collect();
                    self.structs.insert(s.name.clone(), names);
                }
                _ => {}
            }
        }
        for stmt in &program.statements {
            self.exec_stmt(stmt)?;
        }
        Ok(())
    }

    fn emit(&mut self, line: String) {
        if self.capture {
            self.output.push_str(&line);
            self.output.push('\n');
        } else {
            println!("{}", line);
        }
    }

    // ---------- environment ----------

    fn push_scope(&mut self) {
        self.frames.last_mut().unwrap().push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.frames.last_mut().unwrap().pop();
    }

    fn declare(&mut self, name: &str, value: Value) {
        self.frames
            .last_mut()
            .unwrap()
            .last_mut()
            .unwrap()
            .insert(name.to_string(), value);
    }

    fn lookup(&self, name: &str) -> Option<&Value> {
        let cur = self.frames.len() - 1;
        for scope in self.frames[cur].iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v);
            }
        }
        if cur != 0 {
            return self.frames[0][0].get(name);
        }
        None
    }

    fn lookup_mut(&mut self, name: &str) -> Option<&mut Value> {
        let cur = self.frames.len() - 1;
        let pos = self.frames[cur].iter().rposition(|s| s.contains_key(name));
        if let Some(i) = pos {
            return self.frames[cur][i].get_mut(name);
        }
        if cur != 0 {
            return self.frames[0][0].get_mut(name);
        }
        None
    }

    /// Finds the memory location an assignment target refers to.
    fn place_mut(&mut self, expr: &Expr) -> Result<&mut Value, Diagnostic> {
        match &expr.kind {
            ExprKind::Ident(name) => match self.lookup_mut(name) {
                Some(v) => Ok(v),
                None => Err(err(format!("undefined variable '{}'", name), &expr.span)),
            },
            ExprKind::Field { object, name } => {
                let base = self.place_mut(object)?;
                match base {
                    Value::Struct { fields, .. } => match fields.iter_mut().find(|f| f.0 == *name) {
                        Some(f) => Ok(&mut f.1),
                        None => Err(err(format!("no field '{}'", name), &expr.span)),
                    },
                    _ => Err(err("value has no fields", &expr.span)),
                }
            }
            ExprKind::Index { object, index } => {
                let idx = match self.eval_expr(index)? {
                    Value::Int(i) => i,
                    _ => return Err(err("list index must be Int", &index.span)),
                };
                let base = self.place_mut(object)?;
                match base {
                    Value::List(items) => {
                        let len = items.len();
                        if idx < 0 || (idx as usize) >= len {
                            Err(err(
                                format!("index {} out of bounds for list of length {}", idx, len),
                                &expr.span,
                            ))
                        } else {
                            Ok(&mut items[idx as usize])
                        }
                    }
                    _ => Err(err("cannot index into this value", &expr.span)),
                }
            }
            _ => Err(err("invalid assignment target", &expr.span)),
        }
    }

    // ---------- statements ----------

    fn exec_block(&mut self, stmts: &[Stmt]) -> Result<Flow, Diagnostic> {
        self.push_scope();
        let result = self.exec_stmts(stmts);
        self.pop_scope();
        result
    }

    fn exec_stmts(&mut self, stmts: &[Stmt]) -> Result<Flow, Diagnostic> {
        for s in stmts {
            let flow = self.exec_stmt(s)?;
            if !matches!(flow, Flow::Normal) {
                return Ok(flow);
            }
        }
        Ok(Flow::Normal)
    }

    fn run_iteration(&mut self, var: &str, value: Value, body: &[Stmt]) -> Result<Flow, Diagnostic> {
        self.push_scope();
        self.declare(var, value);
        let result = self.exec_stmts(body);
        self.pop_scope();
        result
    }

    fn eval_cond(&mut self, expr: &Expr) -> Result<bool, Diagnostic> {
        match self.eval_expr(expr)? {
            Value::Bool(b) => Ok(b),
            _ => Err(err("condition must be Bool", &expr.span)),
        }
    }

    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<Flow, Diagnostic> {
        match &stmt.kind {
            StmtKind::Let { name, value, .. } => {
                let v = self.eval_expr(value)?;
                self.declare(name, v);
                Ok(Flow::Normal)
            }
            StmtKind::Assign { target, value } => {
                let v = self.eval_expr(value)?;
                let slot = self.place_mut(target)?;
                *slot = v;
                Ok(Flow::Normal)
            }
            StmtKind::Expr(e) => {
                self.eval_expr(e)?;
                Ok(Flow::Normal)
            }
            StmtKind::Return(value) => {
                let v = match value {
                    Some(e) => self.eval_expr(e)?,
                    None => Value::None,
                };
                Ok(Flow::Return(v))
            }
            StmtKind::Break => Ok(Flow::Break),
            StmtKind::Continue => Ok(Flow::Continue),
            StmtKind::If { condition, then_block, else_block } => {
                if self.eval_cond(condition)? {
                    self.exec_block(then_block)
                } else if let Some(block) = else_block {
                    self.exec_block(block)
                } else {
                    Ok(Flow::Normal)
                }
            }
            StmtKind::Try { body, catch_var, catch_block } => match self.exec_block(body) {
                Ok(flow) => Ok(flow),
                Err(e) => {
                    self.push_scope();
                    self.declare(catch_var, Value::Str(e.message));
                    let result = self.exec_stmts(catch_block);
                    self.pop_scope();
                    result
                }
            },
            StmtKind::Throw(e) => {
                let v = self.eval_expr(e)?;
                Err(err(v.to_string(), &stmt.span))
            }
            StmtKind::While { condition, body } => {
                while self.eval_cond(condition)? {
                    match self.exec_block(body)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Normal | Flow::Continue => {}
                    }
                }
                Ok(Flow::Normal)
            }
            StmtKind::For { var, iter, body } => {
                match self.eval_expr(iter)? {
                    Value::Range(a, b) => {
                        let mut i = a;
                        while i < b {
                            match self.run_iteration(var, Value::Int(i), body)? {
                                Flow::Break => break,
                                Flow::Return(v) => return Ok(Flow::Return(v)),
                                _ => {}
                            }
                            i += 1;
                        }
                    }
                    Value::List(items) => {
                        for item in items {
                            match self.run_iteration(var, item, body)? {
                                Flow::Break => break,
                                Flow::Return(v) => return Ok(Flow::Return(v)),
                                _ => {}
                            }
                        }
                    }
                    _ => return Err(err("cannot iterate over this value", &iter.span)),
                }
                Ok(Flow::Normal)
            }
            StmtKind::Function(_) | StmtKind::Struct(_) => Ok(Flow::Normal),
        }
    }

    // ---------- expressions ----------

    fn eval_expr(&mut self, expr: &Expr) -> Result<Value, Diagnostic> {
        match &expr.kind {
            ExprKind::Int(n) => Ok(Value::Int(*n)),
            ExprKind::Float(x) => Ok(Value::Float(*x)),
            ExprKind::Str(s) => Ok(Value::Str(s.clone())),
            ExprKind::Bool(b) => Ok(Value::Bool(*b)),
            ExprKind::NoneLit => Ok(Value::None),
            ExprKind::Ident(name) => match self.lookup(name) {
                Some(v) => Ok(v.clone()),
                None => Err(err(format!("undefined variable '{}'", name), &expr.span)),
            },
            ExprKind::Unary { op, operand } => {
                let v = self.eval_expr(operand)?;
                match (op, v) {
                    (UnaryOp::Neg, Value::Int(n)) => n
                        .checked_neg()
                        .map(Value::Int)
                        .ok_or_else(|| err("integer overflow", &expr.span)),
                    (UnaryOp::Neg, Value::Float(x)) => Ok(Value::Float(-x)),
                    (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                    _ => Err(err("invalid operand", &expr.span)),
                }
            }
            ExprKind::Binary { op, left, right } => match op {
                BinaryOp::And => {
                    if !self.eval_cond(left)? {
                        return Ok(Value::Bool(false));
                    }
                    Ok(Value::Bool(self.eval_cond(right)?))
                }
                BinaryOp::Or => {
                    if self.eval_cond(left)? {
                        return Ok(Value::Bool(true));
                    }
                    Ok(Value::Bool(self.eval_cond(right)?))
                }
                _ => {
                    let l = self.eval_expr(left)?;
                    let r = self.eval_expr(right)?;
                    Self::apply_binary(op, l, r, &expr.span)
                }
            },
            ExprKind::List(items) => {
                let mut out = Vec::new();
                for item in items {
                    out.push(self.eval_expr(item)?);
                }
                Ok(Value::List(out))
            }
            ExprKind::Index { object, index } => {
                let obj = self.eval_expr(object)?;
                let idx = self.eval_expr(index)?;
                match (obj, idx) {
                    (Value::List(items), Value::Int(i)) => {
                        if i < 0 || (i as usize) >= items.len() {
                            Err(err(
                                format!("index {} out of bounds for list of length {}", i, items.len()),
                                &expr.span,
                            ))
                        } else {
                            Ok(items[i as usize].clone())
                        }
                    }
                    _ => Err(err("invalid index operation", &expr.span)),
                }
            }
            ExprKind::Field { object, name } => match self.eval_expr(object)? {
                Value::Struct { fields, .. } => fields
                    .into_iter()
                    .find(|f| f.0 == *name)
                    .map(|f| f.1)
                    .ok_or_else(|| err(format!("no field '{}'", name), &expr.span)),
                _ => Err(err("value has no fields", &expr.span)),
            },
            ExprKind::Call { callee, args } => self.eval_call(callee, args, &expr.span),
        }
    }

    fn compare_ord(op: &BinaryOp, ord: Ordering) -> bool {
        match op {
            BinaryOp::Lt => ord == Ordering::Less,
            BinaryOp::LtEq => ord != Ordering::Greater,
            BinaryOp::Gt => ord == Ordering::Greater,
            BinaryOp::GtEq => ord != Ordering::Less,
            _ => false,
        }
    }

    pub(crate) fn apply_binary(op: &BinaryOp, l: Value, r: Value, span: &Span) -> Result<Value, Diagnostic> {
        match op {
            BinaryOp::Eq => return Ok(Value::Bool(l == r)),
            BinaryOp::NotEq => return Ok(Value::Bool(l != r)),
            _ => {}
        }
        let overflow = || err("integer overflow", span);
        match (op, l, r) {
            (BinaryOp::Add, Value::Int(a), Value::Int(b)) => a.checked_add(b).map(Value::Int).ok_or_else(overflow),
            (BinaryOp::Add, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (BinaryOp::Add, Value::Str(a), Value::Str(b)) => Ok(Value::Str(a + &b)),
            (BinaryOp::Sub, Value::Int(a), Value::Int(b)) => a.checked_sub(b).map(Value::Int).ok_or_else(overflow),
            (BinaryOp::Sub, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            (BinaryOp::Mul, Value::Int(a), Value::Int(b)) => a.checked_mul(b).map(Value::Int).ok_or_else(overflow),
            (BinaryOp::Mul, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (BinaryOp::Div, Value::Int(a), Value::Int(b)) => {
                if b == 0 {
                    Err(err("division by zero", span))
                } else {
                    a.checked_div(b).map(Value::Int).ok_or_else(overflow)
                }
            }
            (BinaryOp::Div, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
            (BinaryOp::Mod, Value::Int(a), Value::Int(b)) => {
                if b == 0 {
                    Err(err("division by zero", span))
                } else {
                    a.checked_rem(b).map(Value::Int).ok_or_else(overflow)
                }
            }
            (BinaryOp::Mod, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a % b)),
            (BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq, Value::Int(a), Value::Int(b)) => {
                Ok(Value::Bool(Self::compare_ord(op, a.cmp(&b))))
            }
            (BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq, Value::Float(a), Value::Float(b)) => {
                match a.partial_cmp(&b) {
                    Some(ord) => Ok(Value::Bool(Self::compare_ord(op, ord))),
                    None => Ok(Value::Bool(false)),
                }
            }
            (BinaryOp::Range, Value::Int(a), Value::Int(b)) => Ok(Value::Range(a, b)),
            _ => Err(err("invalid operands for this operator", span)),
        }
    }

    // ---------- calls ----------

    fn bind_args(
        &mut self,
        names: &[String],
        args: &[Argument],
        span: &Span,
    ) -> Result<Vec<Option<Value>>, Diagnostic> {
        let mut bound: Vec<Option<Value>> = vec![None; names.len()];
        for (i, arg) in args.iter().enumerate() {
            let v = self.eval_expr(&arg.value)?;
            let idx = match &arg.name {
                None => i,
                Some(n) => names
                    .iter()
                    .position(|x| x == n)
                    .ok_or_else(|| err(format!("no parameter named '{}'", n), span))?,
            };
            if idx >= bound.len() {
                return Err(err("too many arguments", span));
            }
            bound[idx] = Some(v);
        }
        Ok(bound)
    }

    fn eval_call(&mut self, callee: &Expr, args: &[Argument], span: &Span) -> Result<Value, Diagnostic> {
        let name = match &callee.kind {
            ExprKind::Ident(n) => n.clone(),
            _ => return Err(err("this value cannot be called", &callee.span)),
        };

        if name == "print" {
            let mut parts = Vec::new();
            for a in args {
                parts.push(self.eval_expr(&a.value)?.to_string());
            }
            self.emit(parts.join(" "));
            return Ok(Value::None);
        }

        if let Some(f) = self.functions.get(&name).cloned() {
            return self.call_function(&f, args, span);
        }

        if let Some(field_names) = self.structs.get(&name).cloned() {
            let bound = self.bind_args(&field_names, args, span)?;
            let mut fields = Vec::new();
            for (fname, val) in field_names.into_iter().zip(bound) {
                match val {
                    Some(v) => fields.push((fname, v)),
                    None => return Err(err(format!("missing field '{}'", fname), span)),
                }
            }
            return Ok(Value::Struct { name, fields });
        }

        Err(err(format!("undefined function '{}'", name), &callee.span))
    }

    fn call_function(&mut self, f: &FunctionDecl, args: &[Argument], span: &Span) -> Result<Value, Diagnostic> {
        if self.frames.len() >= MAX_CALL_DEPTH {
            return Err(err("stack overflow: call depth exceeded", span));
        }
        let names: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
        let bound = self.bind_args(&names, args, span)?;
        self.frames.push(vec![HashMap::new()]);
        let result = self.run_body(f, bound, span);
        self.frames.pop();
        result
    }

    fn run_body(&mut self, f: &FunctionDecl, bound: Vec<Option<Value>>, span: &Span) -> Result<Value, Diagnostic> {
        for (p, val) in f.params.iter().zip(bound) {
            let v = match val {
                Some(v) => v,
                None => match &p.default {
                    Some(d) => self.eval_expr(d)?,
                    None => return Err(err(format!("missing argument '{}'", p.name), span)),
                },
            };
            self.declare(&p.name, v);
        }
        match self.exec_stmts(&f.body)? {
            Flow::Return(v) => Ok(v),
            _ => Ok(Value::None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::lexer::Lexer;
    use crate::compiler::parser::Parser;
    use crate::compiler::semantic::Checker;

    fn build(src: &str) -> Program {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse_program().unwrap();
        Checker::new().check_program(&program).unwrap();
        program
    }

    fn run(src: &str) -> String {
        let program = build(src);
        let mut it = Interpreter::capturing();
        it.run(&program).unwrap();
        it.output().to_string()
    }

    fn run_err(src: &str) -> String {
        let program = build(src);
        let mut it = Interpreter::capturing();
        it.run(&program).unwrap_err().message
    }

    #[test]
    fn arithmetic() {
        assert_eq!(run("print(1 + 2 * 3)\n"), "7\n");
    }

    #[test]
    fn float_display() {
        assert_eq!(run("print(2.5 + 0.5)\nprint(10.0)\n"), "3.0\n10.0\n");
    }

    #[test]
    fn strings() {
        assert_eq!(run("print(\"a\" + \"b\")\nprint(\"x\", 1, true)\n"), "ab\nx 1 true\n");
    }

    #[test]
    fn variables() {
        assert_eq!(run("mut c: Int = 0\nc = c + 1\nc = c + 1\nprint(c)\n"), "2\n");
    }

    #[test]
    fn if_elif_else() {
        let src = "let n = 5\nif n > 10:\n    print(\"big\")\nelif n > 3:\n    print(\"mid\")\nelse:\n    print(\"small\")\n";
        assert_eq!(run(src), "mid\n");
    }

    #[test]
    fn while_loop() {
        assert_eq!(run("mut i = 0\nwhile i < 3:\n    print(i)\n    i = i + 1\n"), "0\n1\n2\n");
    }

    #[test]
    fn for_range() {
        assert_eq!(run("for i in 0..3:\n    print(i)\n"), "0\n1\n2\n");
    }

    #[test]
    fn for_list_with_break_and_continue() {
        let src = "for n in [1, 2, 3, 4, 5]:\n    if n == 2:\n        continue\n    if n == 4:\n        break\n    print(n)\n";
        assert_eq!(run(src), "1\n3\n");
    }

    #[test]
    fn function_call() {
        assert_eq!(run("fn add(a: Int, b: Int) -> Int:\n    return a + b\nprint(add(2, 3))\n"), "5\n");
    }

    #[test]
    fn recursion() {
        let src = "fn fact(n: Int) -> Int:\n    if n <= 1:\n        return 1\n    return n * fact(n - 1)\nprint(fact(10))\n";
        assert_eq!(run(src), "3628800\n");
    }

    #[test]
    fn default_and_named_arguments() {
        let src = "fn greet(name: String, times: Int = 2) -> None:\n    for i in 0..times:\n        print(name)\ngreet(\"hi\")\ngreet(\"yo\", times = 1)\n";
        assert_eq!(run(src), "hi\nhi\nyo\n");
    }

    #[test]
    fn struct_fields_and_display() {
        let src = "struct User:\n    name: String\n    age: Int\nmut u = User(name = \"A\", age = 20)\nu.age = 21\nprint(u.name)\nprint(u.age)\nprint(u)\n";
        assert_eq!(run(src), "A\n21\nUser(name = \"A\", age = 21)\n");
    }

    #[test]
    fn list_operations() {
        let src = "mut xs = [1, 2, 3]\nxs[1] = 20\nprint(xs)\nprint(xs[0] + xs[1])\n";
        assert_eq!(run(src), "[1, 20, 3]\n21\n");
    }

    #[test]
    fn global_access_from_function() {
        let src = "let g = 10\nfn show() -> None:\n    print(g)\nshow()\n";
        assert_eq!(run(src), "10\n");
    }

    #[test]
    fn short_circuit_and() {
        let src = "fn boom() -> Bool:\n    print(\"boom\")\n    return true\nlet a = false and boom()\nprint(a)\n";
        assert_eq!(run(src), "false\n");
    }

    #[test]
    fn list_of_strings_display() {
        assert_eq!(run("print([\"a\", \"b\"])\n"), "[\"a\", \"b\"]\n");
    }

    #[test]
    fn division_by_zero() {
        assert_eq!(run_err("let a = 10\nlet b = 0\nprint(a / b)\n"), "division by zero");
    }

    #[test]
    fn index_out_of_bounds() {
        assert!(run_err("let xs = [1, 2]\nprint(xs[5])\n").contains("out of bounds"));
    }

    #[test]
    fn integer_overflow() {
        assert_eq!(run_err("let a = 9223372036854775807\nprint(a + 1)\n"), "integer overflow");
    }

    #[test]
    fn stack_overflow_is_reported() {
        let msg = std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| run_err("fn f(n: Int) -> Int:\n    return f(n + 1)\nprint(f(0))\n"))
            .unwrap()
            .join()
            .unwrap();
        assert!(msg.contains("stack overflow"));
    }

    #[test]
    fn try_catches_runtime_error() {
        let src = "try:\n    print(1 / 0)\ncatch e:\n    print(\"caught\")\nprint(\"after\")\n";
        assert_eq!(run(src), "caught\nafter\n");
    }

    #[test]
    fn throw_from_function_is_caught() {
        let src = "fn f() -> None:\n    throw \"boom\"\ntry:\n    f()\ncatch e:\n    print(e)\n";
        assert_eq!(run(src), "boom\n");
    }

    #[test]
    fn try_without_error_skips_catch() {
        assert_eq!(run("try:\n    print(1)\ncatch e:\n    print(2)\n"), "1\n");
    }

    #[test]
    fn return_inside_try() {
        let src = "fn f() -> Int:\n    try:\n        return 5\n    catch e:\n        return 0\n    return 1\nprint(f())\n";
        assert_eq!(run(src), "5\n");
    }

    #[test]
    fn break_inside_try_pops_handler() {
        let src = "for i in 0..3:\n    try:\n        if i == 1:\n            break\n    catch e:\n        print(\"caught\")\ntry:\n    throw \"late\"\ncatch e:\n    print(e)\nprint(\"done\")\n";
        assert_eq!(run(src), "late\ndone\n");
    }

    #[test]
    fn continue_inside_try_pops_handler() {
        let src = "for i in 0..3:\n    try:\n        if i == 1:\n            continue\n        print(i)\n    catch e:\n        print(\"caught\")\ntry:\n    throw \"x\"\ncatch e:\n    print(e)\n";
        assert_eq!(run(src), "0\n2\nx\n");
    }
}

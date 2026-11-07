use crate::compiler::ast::BinaryOp;
use crate::compiler::diagnostics::Diagnostic;
use crate::compiler::ir::{Chunk, Instruction, IrProgram};
use crate::compiler::lexer::token::Span;
use crate::runtime::execution::Interpreter;
use crate::runtime::values::Value;

const MAX_CALL_DEPTH: usize = 1000;

fn err(message: impl Into<String>, span: &Span) -> Diagnostic {
    Diagnostic::new(message, span.clone())
}

/// An active `try` block.
struct Handler {
    catch_ip: usize,
    frame_depth: usize,
    stack_depth: usize,
}

struct CallFrame<'a> {
    chunk: &'a Chunk,
    ip: usize,
    locals: Vec<Value>,
}

pub struct Vm<'a> {
    program: &'a IrProgram,
    stack: Vec<Value>,
    globals: Vec<Value>,
    frames: Vec<CallFrame<'a>>,
    handlers: Vec<Handler>,
    capture: bool,
    output: String,
}

impl<'a> Vm<'a> {
    /// VM that prints to the terminal.
    pub fn new(program: &'a IrProgram) -> Self {
        Self::build(program, false)
    }

    /// VM that stores printed output (used by tests).
    pub fn capturing(program: &'a IrProgram) -> Self {
        Self::build(program, true)
    }

    fn build(program: &'a IrProgram, capture: bool) -> Self {
        Vm {
            program,
            stack: Vec::new(),
            globals: vec![Value::None; program.num_globals],
            frames: Vec::new(),
            handlers: Vec::new(),
            capture,
            output: String::new(),
        }
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    fn emit(&mut self, line: String) {
        if self.capture {
            self.output.push_str(&line);
            self.output.push('\n');
        } else {
            println!("{}", line);
        }
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().expect("VM stack underflow (compiler bug)")
    }

    fn binary(&mut self, op: BinaryOp, span: &Span) -> Result<(), Diagnostic> {
        let r = self.pop();
        let l = self.pop();
        let v = Interpreter::apply_binary(&op, l, r, span)?;
        self.stack.push(v);
        Ok(())
    }

    fn drop_stale_handlers(&mut self) {
        while self.handlers.last().map_or(false, |h| h.frame_depth > self.frames.len()) {
            self.handlers.pop();
        }
    }

    pub fn run(&mut self) -> Result<(), Diagnostic> {
        let program: &'a IrProgram = self.program;
        let main = &program.main;
        self.frames.push(CallFrame {
            chunk: &main.chunk,
            ip: 0,
            locals: vec![Value::None; main.num_locals],
        });
        loop {
            match self.run_loop() {
                Ok(()) => return Ok(()),
                Err(e) => {
                    let h = match self.handlers.pop() {
                        Some(h) => h,
                        None => return Err(e),
                    };
                    self.frames.truncate(h.frame_depth);
                    self.stack.truncate(h.stack_depth);
                    self.stack.push(Value::Str(e.message.clone()));
                    self.frames.last_mut().unwrap().ip = h.catch_ip;
                }
            }
        }
    }

    fn run_loop(&mut self) -> Result<(), Diagnostic> {
        let program: &'a IrProgram = self.program;

        loop {
            let chunk: &'a Chunk = self.frames.last().unwrap().chunk;
            let ip = self.frames.last().unwrap().ip;
            self.frames.last_mut().unwrap().ip = ip + 1;
            let ins = &chunk.code[ip];
            let span = &chunk.spans[ip];

            match ins {
                // constants
                Instruction::PushInt(n) => self.stack.push(Value::Int(*n)),
                Instruction::PushFloat(x) => self.stack.push(Value::Float(*x)),
                Instruction::PushBool(b) => self.stack.push(Value::Bool(*b)),
                Instruction::PushStr(s) => self.stack.push(Value::Str(s.clone())),
                Instruction::PushNone => self.stack.push(Value::None),

                // variables
                Instruction::LoadLocal(i) => {
                    let v = self.frames.last().unwrap().locals[*i].clone();
                    self.stack.push(v);
                }
                Instruction::StoreLocal(i) => {
                    let v = self.pop();
                    self.frames.last_mut().unwrap().locals[*i] = v;
                }
                Instruction::LoadGlobal(i) => {
                    let v = self.globals[*i].clone();
                    self.stack.push(v);
                }
                Instruction::StoreGlobal(i) => {
                    let v = self.pop();
                    self.globals[*i] = v;
                }

                // arithmetic and comparison
                Instruction::Add => self.binary(BinaryOp::Add, span)?,
                Instruction::Sub => self.binary(BinaryOp::Sub, span)?,
                Instruction::Mul => self.binary(BinaryOp::Mul, span)?,
                Instruction::Div => self.binary(BinaryOp::Div, span)?,
                Instruction::Mod => self.binary(BinaryOp::Mod, span)?,
                Instruction::Eq => self.binary(BinaryOp::Eq, span)?,
                Instruction::NotEq => self.binary(BinaryOp::NotEq, span)?,
                Instruction::Lt => self.binary(BinaryOp::Lt, span)?,
                Instruction::LtEq => self.binary(BinaryOp::LtEq, span)?,
                Instruction::Gt => self.binary(BinaryOp::Gt, span)?,
                Instruction::GtEq => self.binary(BinaryOp::GtEq, span)?,
                Instruction::MakeRange => self.binary(BinaryOp::Range, span)?,
                Instruction::Neg => {
                    let v = self.pop();
                    let r = match v {
                        Value::Int(n) => n
                            .checked_neg()
                            .map(Value::Int)
                            .ok_or_else(|| err("integer overflow", span))?,
                        Value::Float(x) => Value::Float(-x),
                        _ => return Err(err("invalid operand", span)),
                    };
                    self.stack.push(r);
                }
                Instruction::Not => {
                    let v = self.pop();
                    match v {
                        Value::Bool(b) => self.stack.push(Value::Bool(!b)),
                        _ => return Err(err("invalid operand", span)),
                    }
                }

                // control flow
                Instruction::Jump(target) => {
                    self.frames.last_mut().unwrap().ip = *target;
                }
                Instruction::JumpIfFalse(target) => match self.pop() {
                    Value::Bool(b) => {
                        if !b {
                            self.frames.last_mut().unwrap().ip = *target;
                        }
                    }
                    _ => return Err(err("condition must be Bool", span)),
                },
                Instruction::ForNext { iter, idx, var, exit } => {
                    let frame = self.frames.last_mut().unwrap();
                    let i = match &frame.locals[*idx] {
                        Value::Int(n) => *n,
                        _ => return Err(err("invalid loop state", span)),
                    };
                    let next = match &frame.locals[*iter] {
                        Value::Range(a, b) => match a.checked_add(i) {
                            Some(v) if v < *b => Some(Value::Int(v)),
                            _ => None,
                        },
                        Value::List(items) => items.get(i as usize).cloned(),
                        _ => return Err(err("cannot iterate over this value", span)),
                    };
                    match next {
                        Some(v) => {
                            frame.locals[*var] = v;
                            frame.locals[*idx] = Value::Int(i + 1);
                        }
                        None => frame.ip = *exit,
                    }
                }

                // functions
                Instruction::Call(id) => {
                    if self.frames.len() >= MAX_CALL_DEPTH {
                        return Err(err("stack overflow: call depth exceeded", span));
                    }
                    let callee = &program.functions[*id];
                    let mut locals = vec![Value::None; callee.num_locals];
                    for i in (0..callee.arity).rev() {
                        locals[i] = self.pop();
                    }
                    self.frames.push(CallFrame { chunk: &callee.chunk, ip: 0, locals });
                }
                Instruction::Return => {
                    let ret = self.pop();
                    self.frames.pop();
                    self.drop_stale_handlers();
                    if self.frames.is_empty() {
                        return Ok(());
                    }
                    self.stack.push(ret);
                }
                Instruction::Pop => {
                    self.pop();
                }

                // lists
                Instruction::MakeList(n) => {
                    let start = self.stack.len() - *n;
                    let items = self.stack.split_off(start);
                    self.stack.push(Value::List(items));
                }
                Instruction::Index => {
                    let idx = self.pop();
                    let obj = self.pop();
                    match (obj, idx) {
                        (Value::List(items), Value::Int(i)) => {
                            if i < 0 || (i as usize) >= items.len() {
                                return Err(err(
                                    format!("index {} out of bounds for list of length {}", i, items.len()),
                                    span,
                                ));
                            }
                            self.stack.push(items[i as usize].clone());
                        }
                        _ => return Err(err("invalid index operation", span)),
                    }
                }
                Instruction::StoreIndex => {
                    let idx = self.pop();
                    let obj = self.pop();
                    let val = self.pop();
                    match (obj, idx) {
                        (Value::List(mut items), Value::Int(i)) => {
                            if i < 0 || (i as usize) >= items.len() {
                                return Err(err(
                                    format!("index {} out of bounds for list of length {}", i, items.len()),
                                    span,
                                ));
                            }
                            items[i as usize] = val;
                            self.stack.push(Value::List(items));
                        }
                        _ => return Err(err("cannot index into this value", span)),
                    }
                }

                // structs
                Instruction::MakeStruct(id) => {
                    let info = &program.structs[*id];
                    let start = self.stack.len() - info.fields.len();
                    let values = self.stack.split_off(start);
                    let fields = info.fields.iter().cloned().zip(values).collect();
                    self.stack.push(Value::Struct { name: info.name.clone(), fields });
                }
                Instruction::GetField(name) => {
                    let obj = self.pop();
                    match obj {
                        Value::Struct { fields, .. } => match fields.into_iter().find(|f| f.0 == *name) {
                            Some(f) => self.stack.push(f.1),
                            None => return Err(err(format!("no field '{}'", name), span)),
                        },
                        _ => return Err(err("value has no fields", span)),
                    }
                }
                Instruction::SetField(name) => {
                    let obj = self.pop();
                    let val = self.pop();
                    match obj {
                        Value::Struct { name: struct_name, mut fields } => {
                            match fields.iter_mut().find(|f| f.0 == *name) {
                                Some(f) => f.1 = val,
                                None => return Err(err(format!("no field '{}'", name), span)),
                            }
                            self.stack.push(Value::Struct { name: struct_name, fields });
                        }
                        _ => return Err(err("value has no fields", span)),
                    }
                }

                // error handling
                Instruction::PushHandler(target) => {
                    self.handlers.push(Handler {
                        catch_ip: *target,
                        frame_depth: self.frames.len(),
                        stack_depth: self.stack.len(),
                    });
                }
                Instruction::PopHandler => {
                    self.handlers.pop();
                }
                Instruction::Throw => {
                    let v = self.pop();
                    return Err(err(v.to_string(), span));
                }

                // built-in
                Instruction::Print(n) => {
                    let start = self.stack.len() - *n;
                    let values = self.stack.split_off(start);
                    let line = values
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<String>>()
                        .join(" ");
                    self.emit(line);
                    self.stack.push(Value::None);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::ast::Program;
    use crate::compiler::ir::lower;
    use crate::compiler::lexer::Lexer;
    use crate::compiler::parser::Parser;
    use crate::compiler::semantic::Checker;

    fn build(src: &str) -> (Program, IrProgram) {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse_program().unwrap();
        Checker::new().check_program(&program).unwrap();
        let ir = lower::compile(&program).unwrap();
        (program, ir)
    }

    fn run_tree(src: &str) -> String {
        let (program, _) = build(src);
        let mut it = Interpreter::capturing();
        it.run(&program).unwrap();
        it.output().to_string()
    }

    fn run_vm(src: &str) -> String {
        let (_, ir) = build(src);
        let mut vm = Vm::capturing(&ir);
        vm.run().unwrap();
        vm.output().to_string()
    }

    fn error_in_tree(src: &str) -> String {
        let (program, _) = build(src);
        let mut it = Interpreter::capturing();
        it.run(&program).unwrap_err().message
    }

    fn error_in_vm(src: &str) -> String {
        let (_, ir) = build(src);
        let mut vm = Vm::capturing(&ir);
        vm.run().unwrap_err().message
    }

    /// Both engines must print exactly the expected text.
    fn same(src: &str, expected: &str) {
        assert_eq!(run_tree(src), expected, "tree-walker output");
        assert_eq!(run_vm(src), expected, "VM output");
    }

    #[test]
    fn arithmetic() {
        same("print(1 + 2 * 3)\n", "7\n");
    }

    #[test]
    fn float_display() {
        same("print(2.5 + 0.5)\nprint(10.0)\n", "3.0\n10.0\n");
    }

    #[test]
    fn strings() {
        same("print(\"a\" + \"b\")\nprint(\"x\", 1, true)\n", "ab\nx 1 true\n");
    }

    #[test]
    fn variables() {
        same("mut c: Int = 0\nc = c + 1\nc = c + 1\nprint(c)\n", "2\n");
    }

    #[test]
    fn if_elif_else() {
        let src = "let n = 5\nif n > 10:\n    print(\"big\")\nelif n > 3:\n    print(\"mid\")\nelse:\n    print(\"small\")\n";
        same(src, "mid\n");
    }

    #[test]
    fn while_loop() {
        same("mut i = 0\nwhile i < 3:\n    print(i)\n    i = i + 1\n", "0\n1\n2\n");
    }

    #[test]
    fn for_range() {
        same("for i in 0..3:\n    print(i)\n", "0\n1\n2\n");
    }

    #[test]
    fn for_list_with_break_and_continue() {
        let src = "for n in [1, 2, 3, 4, 5]:\n    if n == 2:\n        continue\n    if n == 4:\n        break\n    print(n)\n";
        same(src, "1\n3\n");
    }

    #[test]
    fn function_call() {
        same("fn add(a: Int, b: Int) -> Int:\n    return a + b\nprint(add(2, 3))\n", "5\n");
    }

    #[test]
    fn recursion() {
        let src = "fn fact(n: Int) -> Int:\n    if n <= 1:\n        return 1\n    return n * fact(n - 1)\nprint(fact(10))\n";
        same(src, "3628800\n");
    }

    #[test]
    fn fibonacci() {
        let src = "fn fib(n: Int) -> Int:\n    if n < 2:\n        return n\n    return fib(n - 1) + fib(n - 2)\nprint(fib(15))\n";
        same(src, "610\n");
    }

    #[test]
    fn default_and_named_arguments() {
        let src = "fn greet(name: String, times: Int = 2) -> None:\n    for i in 0..times:\n        print(name)\ngreet(\"hi\")\ngreet(\"yo\", times = 1)\n";
        same(src, "hi\nhi\nyo\n");
    }

    #[test]
    fn default_value_uses_global() {
        let src = "let base = 10\nfn f(a: Int, b: Int = base) -> Int:\n    return a + b\nprint(f(1))\n";
        same(src, "11\n");
    }

    #[test]
    fn struct_fields_and_display() {
        let src = "struct User:\n    name: String\n    age: Int\nmut u = User(name = \"A\", age = 20)\nu.age = 21\nprint(u.name)\nprint(u.age)\nprint(u)\n";
        same(src, "A\n21\nUser(name = \"A\", age = 21)\n");
    }

    #[test]
    fn nested_struct_assignment() {
        let src = "struct In:\n    v: Int\nstruct Out:\n    inner: In\nmut o = Out(inner = In(v = 1))\no.inner.v = 42\nprint(o.inner.v)\n";
        same(src, "42\n");
    }

    #[test]
    fn list_operations() {
        let src = "mut xs = [1, 2, 3]\nxs[1] = 20\nprint(xs)\nprint(xs[0] + xs[1])\n";
        same(src, "[1, 20, 3]\n21\n");
    }

    #[test]
    fn nested_list_assignment() {
        let src = "mut grid = [[1, 2], [3, 4]]\ngrid[1][0] = 9\nprint(grid)\n";
        same(src, "[[1, 2], [9, 4]]\n");
    }

    #[test]
    fn global_access_and_mutation_from_function() {
        let src = "mut g = 0\nfn bump(n: Int) -> None:\n    g = g + n\nbump(2)\nbump(3)\nprint(g)\n";
        same(src, "5\n");
    }

    #[test]
    fn short_circuit_and() {
        let src = "fn boom() -> Bool:\n    print(\"boom\")\n    return true\nlet a = false and boom()\nprint(a)\n";
        same(src, "false\n");
    }

    #[test]
    fn list_of_strings_display() {
        same("print([\"a\", \"b\"])\n", "[\"a\", \"b\"]\n");
    }

    #[test]
    fn nested_loops_with_break() {
        let src = "mut count = 0\nfor i in 0..5:\n    for j in 0..5:\n        if j == 3:\n            break\n        count = count + 1\nprint(count)\n";
        same(src, "15\n");
    }

    #[test]
    fn while_with_continue() {
        let src = "mut i = 0\nmut sum = 0\nwhile i < 10:\n    i = i + 1\n    if i % 2 == 0:\n        continue\n    sum = sum + i\nprint(sum)\n";
        same(src, "25\n");
    }

    #[test]
    fn division_by_zero_matches() {
        let src = "let a = 10\nlet b = 0\nprint(a / b)\n";
        assert_eq!(error_in_vm(src), "division by zero");
        assert_eq!(error_in_tree(src), error_in_vm(src));
    }

    #[test]
    fn index_out_of_bounds_matches() {
        let src = "let xs = [1, 2]\nprint(xs[5])\n";
        assert!(error_in_vm(src).contains("out of bounds"));
        assert_eq!(error_in_tree(src), error_in_vm(src));
    }

    #[test]
    fn integer_overflow_matches() {
        let src = "let a = 9223372036854775807\nprint(a + 1)\n";
        assert_eq!(error_in_vm(src), "integer overflow");
        assert_eq!(error_in_tree(src), error_in_vm(src));
    }

    #[test]
    fn stack_overflow_is_reported() {
        let src = "fn f(n: Int) -> Int:\n    return f(n + 1)\nprint(f(0))\n";
        assert!(error_in_vm(src).contains("stack overflow"));
    }

    #[test]
    fn run_demo_example_matches() {
        let src = include_str!("../../../examples/run_demo.rca");
        let tree = run_tree(src);
        assert!(tree.contains("Adult"));
        assert_eq!(tree, run_vm(src));
    }

    #[test]
    fn try_catches_runtime_error() {
        let src = "try:\n    print(1 / 0)\ncatch e:\n    print(\"caught\")\nprint(\"after\")\n";
        assert_eq!(run_vm(src), "caught\nafter\n");
    }

    #[test]
    fn throw_from_function_is_caught() {
        let src = "fn f() -> None:\n    throw \"boom\"\ntry:\n    f()\ncatch e:\n    print(e)\n";
        assert_eq!(run_vm(src), "boom\n");
    }

    #[test]
    fn try_without_error_skips_catch() {
        assert_eq!(run_vm("try:\n    print(1)\ncatch e:\n    print(2)\n"), "1\n");
    }

    #[test]
    fn return_inside_try() {
        let src = "fn f() -> Int:\n    try:\n        return 5\n    catch e:\n        return 0\n    return 1\nprint(f())\n";
        assert_eq!(run_vm(src), "5\n");
    }

    #[test]
    fn break_inside_try_pops_handler() {
        let src = "for i in 0..3:\n    try:\n        if i == 1:\n            break\n    catch e:\n        print(\"caught\")\ntry:\n    throw \"late\"\ncatch e:\n    print(e)\nprint(\"done\")\n";
        assert_eq!(run_vm(src), "late\ndone\n");
    }

    #[test]
    fn continue_inside_try_pops_handler() {
        let src = "for i in 0..3:\n    try:\n        if i == 1:\n            continue\n        print(i)\n    catch e:\n        print(\"caught\")\ntry:\n    throw \"x\"\ncatch e:\n    print(e)\n";
        assert_eq!(run_vm(src), "0\n2\nx\n");
    }
}

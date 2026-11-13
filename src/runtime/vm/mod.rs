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
        while self
            .handlers
            .last()
            .is_some_and(|h| h.frame_depth > self.frames.len())
        {
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
                Instruction::PushEnum(n, v) => self.stack.push(Value::Enum {
                    name: n.clone(),
                    variant: v.clone(),
                }),
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
                Instruction::ForNext {
                    iter,
                    idx,
                    var,
                    exit,
                } => {
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
                    self.frames.push(CallFrame {
                        chunk: &callee.chunk,
                        ip: 0,
                        locals,
                    });
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

                Instruction::ReadFile => {
                    let path = match self.pop() {
                        Value::Str(s) => s,
                        _ => return Err(err("read_file() needs a String path", span)),
                    };
                    match std::fs::read_to_string(&path) {
                        Ok(contents) => self.stack.push(Value::Str(contents)),
                        Err(e) => return Err(err(format!("read_file('{}'): {}", path, e), span)),
                    }
                }
                Instruction::WriteFile => {
                    let content = match self.pop() {
                        Value::Str(s) => s,
                        _ => return Err(err("write_file() needs a String content", span)),
                    };
                    let path = match self.pop() {
                        Value::Str(s) => s,
                        _ => return Err(err("write_file() needs a String path", span)),
                    };
                    match std::fs::write(&path, &content) {
                        Ok(()) => self.stack.push(Value::None),
                        Err(e) => return Err(err(format!("write_file('{}'): {}", path, e), span)),
                    }
                }

                Instruction::MathAbs => {
                    let v = self.pop();
                    let r = match v {
                        Value::Int(n) => n
                            .checked_abs()
                            .map(Value::Int)
                            .ok_or_else(|| err("integer overflow", span))?,
                        Value::Float(x) => Value::Float(x.abs()),
                        _ => return Err(err("abs() needs a number", span)),
                    };
                    self.stack.push(r);
                }
                Instruction::MathPow => {
                    let exp = self.pop();
                    let base = self.pop();
                    let r = match (base, exp) {
                        (Value::Int(b), Value::Int(e)) if e >= 0 => {
                            Value::Int(b.saturating_pow(e as u32))
                        }
                        (b, e) => {
                            let bf = match b {
                                Value::Int(i) => i as f64,
                                Value::Float(x) => x,
                                _ => return Err(err("pow() needs numbers", span)),
                            };
                            let ef = match e {
                                Value::Int(i) => i as f64,
                                Value::Float(x) => x,
                                _ => return Err(err("pow() needs numbers", span)),
                            };
                            Value::Float(bf.powf(ef))
                        }
                    };
                    self.stack.push(r);
                }
                Instruction::MathFloor | Instruction::MathCeil | Instruction::MathRound => {
                    let v = self.pop();
                    let x = match v {
                        Value::Int(i) => i as f64,
                        Value::Float(x) => x,
                        _ => return Err(err("math function needs a number", span)),
                    };
                    let r = match ins {
                        Instruction::MathFloor => x.floor(),
                        Instruction::MathCeil => x.ceil(),
                        _ => x.round(),
                    };
                    self.stack.push(Value::Float(r));
                }
                Instruction::StrTrim => {
                    let s = match self.pop() {
                        Value::Str(s) => s,
                        _ => return Err(err("trim() needs a String", span)),
                    };
                    self.stack.push(Value::Str(s.trim().to_string()));
                }
                Instruction::StrSplit => {
                    let sep = match self.pop() {
                        Value::Str(s) => s,
                        _ => return Err(err("split() needs a separator", span)),
                    };
                    let s = match self.pop() {
                        Value::Str(s) => s,
                        _ => return Err(err("split() needs a String", span)),
                    };
                    let parts = s.split(&sep).map(|p| Value::Str(p.to_string())).collect();
                    self.stack.push(Value::List(parts));
                }
                Instruction::JsonParse => {
                    let s = match self.pop() {
                        Value::Str(s) => s,
                        _ => return Err(err("json_parse() needs a String", span)),
                    };
                    match crate::runtime::json::parse(&s) {
                        Ok(v) => self.stack.push(v),
                        Err(m) => return Err(err(format!("json_parse(): {}", m), span)),
                    }
                }
                Instruction::JsonStringify => {
                    let v = self.pop();
                    match crate::runtime::json::stringify(&v) {
                        Ok(s) => self.stack.push(Value::Str(s)),
                        Err(m) => return Err(err(format!("json_stringify(): {}", m), span)),
                    }
                }
                Instruction::TcpConnect => {
                    let port = match self.pop() {
                        Value::Int(i) => i as u16,
                        _ => return Err(err("tcp_connect() needs an Int port", span)),
                    };
                    let host = match self.pop() {
                        Value::Str(s) => s,
                        _ => return Err(err("tcp_connect() needs a String host", span)),
                    };
                    let addr = format!("{}:{}", host, port);
                    match std::net::TcpStream::connect(&addr) {
                        Ok(stream) => self.stack.push(Value::Socket(crate::runtime::net::Socket {
                            stream: std::sync::Arc::new(std::sync::Mutex::new(stream)),
                        })),
                        Err(e) => return Err(err(format!("tcp_connect failed: {}", e), span)),
                    }
                }
                Instruction::TcpSend => {
                    let data = match self.pop() {
                        Value::Str(s) => s,
                        _ => return Err(err("tcp_send() needs a String", span)),
                    };
                    let sock = match self.pop() {
                        Value::Socket(s) => s,
                        _ => return Err(err("tcp_send() needs a Socket", span)),
                    };
                    use std::io::Write;
                    let mut stream = sock.stream.lock().unwrap();
                    match stream.write(data.as_bytes()) {
                        Ok(n) => self.stack.push(Value::Int(n as i64)),
                        Err(e) => return Err(err(format!("tcp_send failed: {}", e), span)),
                    }
                }
                Instruction::TcpRecv => {
                    let max_bytes = match self.pop() {
                        Value::Int(i) => i as usize,
                        _ => return Err(err("tcp_recv() needs an Int", span)),
                    };
                    let sock = match self.pop() {
                        Value::Socket(s) => s,
                        _ => return Err(err("tcp_recv() needs a Socket", span)),
                    };
                    use std::io::Read;
                    let mut buf = vec![0u8; max_bytes];
                    let mut stream = sock.stream.lock().unwrap();
                    match stream.read(&mut buf) {
                        Ok(n) => self
                            .stack
                            .push(Value::Str(String::from_utf8_lossy(&buf[..n]).to_string())),
                        Err(e) => return Err(err(format!("tcp_recv failed: {}", e), span)),
                    }
                }
                Instruction::Sha256 => {
                    let s = match self.pop() {
                        Value::Str(s) => s,
                        _ => return Err(err("sha256() needs a String", span)),
                    };
                    self.stack
                        .push(Value::Str(crate::runtime::crypto::sha256(&s)));
                }
                Instruction::HttpGet => {
                    let url = match self.pop() {
                        Value::Str(s) => s,
                        _ => return Err(err("http_get() needs a String URL", span)),
                    };
                    match crate::runtime::http::http_get(&url) {
                        Ok(body) => self.stack.push(Value::Str(body)),
                        Err(m) => return Err(err(format!("http_get(): {}", m), span)),
                    }
                }
                Instruction::TcpClose => {
                    let _sock = match self.pop() {
                        Value::Socket(s) => s,
                        _ => return Err(err("tcp_close() needs a Socket", span)),
                    };
                    self.stack.push(Value::None);
                }

                Instruction::MapNew => {
                    self.stack.push(Value::Map(Vec::new()));
                }
                Instruction::MapSet => {
                    let val = self.pop();
                    let key = self.pop();
                    let mut map = match self.pop() {
                        Value::Map(m) => m,
                        _ => return Err(err("map_set() needs a Map", span)),
                    };
                    if let Some(pos) = map.iter().position(|(k, _)| k == &key) {
                        map[pos] = (key, val);
                    } else {
                        map.push((key, val));
                    }
                    self.stack.push(Value::Map(map));
                }
                Instruction::MapGet(_) => {
                    let key = self.pop();
                    let map = match self.pop() {
                        Value::Map(m) => m,
                        _ => return Err(err("map_get() needs a Map", span)),
                    };
                    match map.into_iter().find(|(k, _)| *k == key) {
                        Some((_, v)) => self.stack.push(v),
                        None => return Err(err("map_get(): key not found".to_string(), span)),
                    }
                }
                Instruction::MapHas => {
                    let key = self.pop();
                    let map = match self.pop() {
                        Value::Map(m) => m,
                        _ => return Err(err("map_has() needs a Map", span)),
                    };
                    self.stack
                        .push(Value::Bool(map.into_iter().any(|(k, _)| k == key)));
                }
                Instruction::MapLen => {
                    let map = match self.pop() {
                        Value::Map(m) => m,
                        _ => return Err(err("map_len() needs a Map", span)),
                    };
                    self.stack.push(Value::Int(map.len() as i64));
                }
                Instruction::StrReplace => {
                    let rep = match self.pop() {
                        Value::Str(s) => s,
                        _ => return Err(err("replace() needs a String", span)),
                    };
                    let target = match self.pop() {
                        Value::Str(s) => s,
                        _ => return Err(err("replace() needs a String", span)),
                    };
                    let s = match self.pop() {
                        Value::Str(s) => s,
                        _ => return Err(err("replace() needs a String", span)),
                    };
                    self.stack.push(Value::Str(s.replace(&target, &rep)));
                }
                Instruction::TimeNow => {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    self.stack.push(Value::Int(now as i64));
                }
                Instruction::TimeSleep => {
                    let ms = match self.pop() {
                        Value::Int(n) if n >= 0 => n as u64,
                        _ => return Err(err("sleep() needs a non-negative Int (ms)", span)),
                    };
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                    self.stack.push(Value::None);
                }
                Instruction::FileExists => {
                    let path = match self.pop() {
                        Value::Str(s) => s,
                        _ => return Err(err("file_exists() needs a String path", span)),
                    };
                    self.stack
                        .push(Value::Bool(std::path::Path::new(&path).exists()));
                }

                // lists
                Instruction::MakeTuple(n) => {
                    let start = self.stack.len() - *n;
                    let items = self.stack.split_off(start);
                    self.stack.push(Value::Tuple(items));
                }
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
                                    format!(
                                        "index {} out of bounds for list of length {}",
                                        i,
                                        items.len()
                                    ),
                                    span,
                                ));
                            }
                            self.stack.push(items[i as usize].clone());
                        }
                        (t @ Value::Tensor { .. }, Value::Int(i)) => {
                            match crate::runtime::values::tensor_index(&t, i) {
                                Ok(v) => self.stack.push(v),
                                Err(m) => return Err(err(m, span)),
                            }
                        }
                        (Value::Tuple(items), Value::Int(i)) => match items.get(i as usize) {
                            Some(v) if i >= 0 => self.stack.push(v.clone()),
                            _ => return Err(err("tuple index out of range", span)),
                        },
                        _ => return Err(err("invalid index operation", span)),
                    }
                }
                Instruction::IndexAt(i) => {
                    let i = *i;
                    let obj = self.pop();
                    match obj {
                        Value::List(items) => {
                            if i < 0 || (i as usize) >= items.len() {
                                return Err(err(
                                    format!(
                                        "index {} out of bounds for list of length {}",
                                        i,
                                        items.len()
                                    ),
                                    span,
                                ));
                            }
                            self.stack.push(items[i as usize].clone());
                        }
                        t @ Value::Tensor { .. } => {
                            match crate::runtime::values::tensor_index(&t, i) {
                                Ok(v) => self.stack.push(v),
                                Err(m) => return Err(err(m, span)),
                            }
                        }
                        Value::Tuple(items) => match items.get(i as usize) {
                            Some(v) if i >= 0 => self.stack.push(v.clone()),
                            _ => return Err(err("tuple index out of range", span)),
                        },
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
                                    format!(
                                        "index {} out of bounds for list of length {}",
                                        i,
                                        items.len()
                                    ),
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
                    self.stack.push(Value::Struct {
                        name: info.name.clone(),
                        fields,
                    });
                }
                Instruction::GetField(name) => {
                    let obj = self.pop();
                    match obj {
                        Value::Struct { fields, .. } => {
                            match fields.into_iter().find(|f| f.0 == *name) {
                                Some(f) => self.stack.push(f.1),
                                None => return Err(err(format!("no field '{}'", name), span)),
                            }
                        }
                        _ => return Err(err("value has no fields", span)),
                    }
                }
                Instruction::SetField(name) => {
                    let obj = self.pop();
                    let val = self.pop();
                    match obj {
                        Value::Struct {
                            name: struct_name,
                            mut fields,
                        } => {
                            match fields.iter_mut().find(|f| f.0 == *name) {
                                Some(f) => f.1 = val,
                                None => return Err(err(format!("no field '{}'", name), span)),
                            }
                            self.stack.push(Value::Struct {
                                name: struct_name,
                                fields,
                            });
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

                Instruction::Convert(kind) => {
                    let v = self.pop();
                    match v.convert(*kind) {
                        Ok(r) => self.stack.push(r),
                        Err(m) => return Err(err(m, span)),
                    }
                }

                Instruction::ListPush => {
                    let list = self.pop();
                    let val = self.pop();
                    match list {
                        Value::List(mut items) => {
                            items.push(val);
                            self.stack.push(Value::List(items));
                        }
                        _ => return Err(err("push() needs a List", span)),
                    }
                }

                Instruction::MakeTensor => {
                    let v = self.pop();
                    match crate::runtime::values::tensor_from_value(&v) {
                        Ok(t) => self.stack.push(t),
                        Err(m) => return Err(err(m, span)),
                    }
                }
                Instruction::FillTensor(x) => {
                    let v = self.pop();
                    match crate::runtime::values::tensor_filled(&v, *x) {
                        Ok(t) => self.stack.push(t),
                        Err(m) => return Err(err(m, span)),
                    }
                }
                Instruction::MatMul => {
                    let b = self.pop();
                    let a = self.pop();
                    match crate::runtime::values::tensor_matmul(&a, &b) {
                        Ok(t) => self.stack.push(t),
                        Err(m) => return Err(err(m, span)),
                    }
                }
                Instruction::TensorSum
                | Instruction::TensorMean
                | Instruction::TensorMax
                | Instruction::TensorMin => {
                    let v = self.pop();
                    let kind = match ins {
                        Instruction::TensorSum => 's',
                        Instruction::TensorMean => 'a',
                        Instruction::TensorMax => 'x',
                        _ => 'n',
                    };
                    match crate::runtime::values::tensor_reduce(&v, kind) {
                        Ok(t) => self.stack.push(t),
                        Err(m) => return Err(err(m, span)),
                    }
                }
                Instruction::TensorTranspose => {
                    let v = self.pop();
                    match crate::runtime::values::tensor_transpose(&v) {
                        Ok(t) => self.stack.push(t),
                        Err(m) => return Err(err(m, span)),
                    }
                }
                Instruction::TensorMap(kind) => {
                    let v = self.pop();
                    match crate::runtime::values::tensor_map(&v, *kind) {
                        Ok(t) => self.stack.push(t),
                        Err(m) => return Err(err(m, span)),
                    }
                }
                Instruction::TensorSoftmax => {
                    let v = self.pop();
                    match crate::runtime::values::tensor_softmax(&v) {
                        Ok(t) => self.stack.push(t),
                        Err(m) => return Err(err(m, span)),
                    }
                }
                Instruction::Conv2d => {
                    let b = self.pop();
                    let a = self.pop();
                    match crate::runtime::values::tensor_conv2d(&a, &b) {
                        Ok(t) => self.stack.push(t),
                        Err(m) => return Err(err(m, span)),
                    }
                }
                Instruction::Track => {
                    let v = self.pop();
                    match crate::runtime::values::tensor_track(&v) {
                        Ok(t) => self.stack.push(t),
                        Err(m) => return Err(err(m, span)),
                    }
                }
                Instruction::Grad => {
                    let x = self.pop();
                    let y = self.pop();
                    match crate::runtime::values::tensor_grad(&y, &x) {
                        Ok(t) => self.stack.push(t),
                        Err(m) => return Err(err(m, span)),
                    }
                }
                Instruction::TensorReshape => {
                    let s = self.pop();
                    let v = self.pop();
                    match crate::runtime::values::tensor_reshape(&v, &s) {
                        Ok(t) => self.stack.push(t),
                        Err(m) => return Err(err(m, span)),
                    }
                }
                Instruction::TensorShape => {
                    let v = self.pop();
                    match crate::runtime::values::tensor_shape(&v) {
                        Ok(t) => self.stack.push(t),
                        Err(m) => return Err(err(m, span)),
                    }
                }

                Instruction::Len => {
                    let v = self.pop();
                    match v {
                        Value::Str(s) => self.stack.push(Value::Int(s.chars().count() as i64)),
                        Value::List(items) => self.stack.push(Value::Int(items.len() as i64)),
                        Value::Tuple(items) => self.stack.push(Value::Int(items.len() as i64)),
                        Value::Tensor { shape, .. } => self.stack.push(Value::Int(shape[0] as i64)),
                        _ => return Err(err("len() needs a String or List", span)),
                    }
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
        same(
            "print(\"a\" + \"b\")\nprint(\"x\", 1, true)\n",
            "ab\nx 1 true\n",
        );
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
        same(
            "mut i = 0\nwhile i < 3:\n    print(i)\n    i = i + 1\n",
            "0\n1\n2\n",
        );
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
        same(
            "fn add(a: Int, b: Int) -> Int:\n    return a + b\nprint(add(2, 3))\n",
            "5\n",
        );
    }

    #[test]
    fn recursion() {
        let src = "fn fact(n: Int) -> Int:\n    if n <= 1:\n        return 1\n    return n * fact(n - 1)\nprint(fact(10))\n";
        same(src, "3628800\n");
    }

    #[test]
    fn tensor_math_matches() {
        let src = "let x = Tensor([[-1.0, 0.0], [2.0, 4.0]])\nprint(relu(x))\nprint(sqrt(Tensor([4.0, 9.0, 16.0])))\nprint(exp(Tensor([0.0, 0.0])))\nprint(sigmoid(Tensor([0.0])))\nprint(log(Tensor([1.0])))\n";
        same(
            src,
            "Tensor(shape=[2, 2], [[0.0, 0.0], [2.0, 4.0]])\nTensor(shape=[3], [2.0, 3.0, 4.0])\nTensor(shape=[2], [1.0, 1.0])\nTensor(shape=[1], [0.5])\nTensor(shape=[1], [0.0])\n",
        );
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
        let src =
            "let base = 10\nfn f(a: Int, b: Int = base) -> Int:\n    return a + b\nprint(f(1))\n";
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
        let src =
            "mut g = 0\nfn bump(n: Int) -> None:\n    g = g + n\nbump(2)\nbump(3)\nprint(g)\n";
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
    fn tensor_math_domain_errors_match() {
        for (src, msg) in [
            ("print(log(Tensor([0.0])))\n", "log() needs positive values"),
            (
                "print(sqrt(Tensor([-1.0])))\n",
                "sqrt() needs non-negative values",
            ),
            (
                "print(exp(Tensor([1000.0])))\n",
                "exp() result is not finite",
            ),
        ] {
            assert_eq!(error_in_vm(src), msg);
            assert_eq!(error_in_tree(src), error_in_vm(src));
        }
    }

    #[test]
    fn autodiff_square_matches() {
        let src = "let x = track(Tensor([2.0]))\nlet y = x * x\nprint(grad(y, x))\n";
        same(src, "Tensor(shape=[1], [4.0])\n");
    }

    #[test]
    fn autodiff_chain_matches() {
        let src =
            "let x = track(Tensor([3.0]))\nlet y = x * x * x + x\nprint(grad(y, x))\nprint(y)\n";
        same(
            src,
            "Tensor(shape=[1], [28.0])\nTensor(shape=[1], [30.0])\n",
        );
    }

    #[test]
    fn autodiff_sub_div_matches() {
        let src = "let x = track(Tensor([6.0]))\nlet y = track(Tensor([2.0]))\nlet z = x / y - y\nprint(z)\nprint(grad(z, x))\nprint(grad(z, y))\n";
        same(
            src,
            "Tensor(shape=[1], [1.0])\nTensor(shape=[1], [0.5])\nTensor(shape=[1], [-2.5])\n",
        );
    }

    #[test]
    fn autodiff_mixed_constants_match() {
        let src = "let w = track(Tensor([3.0]))\nlet x = Tensor([2.0])\nlet y = w * x + 1.0\nprint(y)\nprint(grad(y, w))\nlet z = 10.0 - w / 2.0\nprint(z)\nprint(grad(z, w))\n";
        same(
            src,
            "Tensor(shape=[1], [7.0])\nTensor(shape=[1], [2.0])\nTensor(shape=[1], [8.5])\nTensor(shape=[1], [-0.5])\n",
        );
    }

    #[test]
    fn autodiff_sum_mean_matches() {
        let src = "let x = track(Tensor([1.0, 2.0, 3.0]))\nprint(grad(sum(x * x), x))\nprint(grad(mean(x * x), x))\nprint(sum(x))\n";
        same(
            src,
            "Tensor(shape=[3], [2.0, 4.0, 6.0])\nTensor(shape=[3], [0.6666666666666666, 1.3333333333333333, 2.0])\nTensor(shape=[1], [6.0])\n",
        );
    }

    #[test]
    fn autodiff_regression_example_matches() {
        let src = include_str!("../../../examples/autodiff_regression.rca");
        same(
            src,
            "Tensor(shape=[1], [8.5])\nTensor(shape=[2, 1], [[13.0], [18.0]])\n",
        );
    }

    #[test]
    fn autodiff_matmul_errors_match() {
        for (src, msg) in [
            (
                "let a = track(Tensor([[1.0, 2.0]]))\nlet b = Tensor([[1.0, 2.0]])\nprint(matmul(a, b))\n",
                "matmul shapes do not match: [1, 2] and [1, 2]",
            ),
            (
                "let a = track(Tensor([1.0, 2.0]))\nprint(max(a))\n",
                "max() and min() are not supported for tracked tensors yet",
            ),
            (
                "let a = Tensor([1.0, 2.0])\nprint(grad(sum(a), a))\n",
                "grad() needs two tracked Tensors (use track())",
            ),
        ] {
            assert_eq!(error_in_vm(src), msg);
            assert_eq!(error_in_tree(src), error_in_vm(src));
        }
    }

    #[test]
    fn autodiff_elementwise_matches() {
        let src = "let z = track(Tensor([0.0]))\nprint(grad(sum(sigmoid(z)), z))\nprint(grad(sum(exp(z)), z))\nprint(grad(sum(tanh(z)), z))\nlet p = track(Tensor([4.0]))\nprint(grad(sum(sqrt(p)), p))\nprint(grad(sum(log(p)), p))\nlet r = track(Tensor([-1.0, 2.0, 3.0]))\nprint(grad(sum(relu(r)), r))\n";
        same(
            src,
            "Tensor(shape=[1], [0.25])\nTensor(shape=[1], [1.0])\nTensor(shape=[1], [1.0])\nTensor(shape=[1], [0.25])\nTensor(shape=[1], [0.25])\nTensor(shape=[3], [0.0, 1.0, 1.0])\n",
        );
    }

    #[test]
    fn autodiff_transpose_reshape_matches() {
        let src = "let x = track(Tensor([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]))\nlet w = Tensor([[1.0, 10.0], [2.0, 20.0], [3.0, 30.0]])\nprint(grad(sum(transpose(x) * w), x))\nlet v = track(Tensor([1.0, 2.0, 3.0, 4.0]))\nlet m = Tensor([[1.0, 2.0], [3.0, 4.0]])\nprint(grad(sum(reshape(v, [2, 2]) * m), v))\n";
        same(
            src,
            "Tensor(shape=[2, 3], [[1.0, 2.0, 3.0], [10.0, 20.0, 30.0]])\nTensor(shape=[4], [1.0, 2.0, 3.0, 4.0])\n",
        );
    }

    #[test]
    fn autodiff_elementwise_errors_match() {
        for (src, msg) in [
            (
                "let x = track(Tensor([-1.0]))\nprint(log(x))\n",
                "log() needs positive values",
            ),
            (
                "let x = track(Tensor([1.0, 2.0, 3.0]))\nprint(reshape(x, [2, 2]))\n",
                "cannot reshape tensor of 3 elements into shape [2, 2]",
            ),
            (
                "let x = track(Tensor([0.0]))\nprint(grad(sum(sqrt(x)), x))\n",
                "grad(): gradient is not finite",
            ),
        ] {
            assert_eq!(error_in_vm(src), msg);
            assert_eq!(error_in_tree(src), error_in_vm(src));
        }
    }

    #[test]
    fn autodiff_logistic_example_matches() {
        let src = include_str!("../../../examples/autodiff_logistic.rca");
        let out = run_tree(src);
        assert_eq!(run_vm(src), out);
        assert!(out.starts_with("Tensor(shape=[1], ["));
    }

    #[test]
    fn autodiff_errors_match() {
        for (src, msg) in [
            (
                "let x = track(Tensor([2.0]))\nlet z = track(Tensor([1.0]))\nlet y = x * x\nprint(grad(y, z))\n",
                "grad(): the result does not depend on this tensor",
            ),
            (
                "let x = Tensor([2.0])\nprint(grad(x, x))\n",
                "grad() needs two tracked Tensors (use track())",
            ),
            (
                "let x = track(Tensor([1.0, 2.0]))\nlet y = x * x\nprint(grad(y, x))\n",
                "grad() needs a result with exactly one element",
            ),
            (
                "let x = track(Tensor([1.0, 2.0]))\nprint(x + Tensor([1.0, 2.0, 3.0]))\n",
                "cannot broadcast shapes [2] and [3]",
            ),
            (
                "let x = track(Tensor([2.0]))\nlet y = x / 0.0\nprint(grad(y, x))\n",
                "grad(): gradient is not finite",
            ),
        ] {
            assert_eq!(error_in_vm(src), msg);
            assert_eq!(error_in_tree(src), error_in_vm(src));
        }
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
        assert_eq!(
            run_vm("try:\n    print(1)\ncatch e:\n    print(2)\n"),
            "1\n"
        );
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

    #[test]
    fn len_of_list_and_string() {
        assert_eq!(
            run_vm("print(len([1, 2, 3]))\nprint(len(\"hello\"))\nprint(len(\"\"))\n"),
            "3\n5\n0\n"
        );
    }

    #[test]
    fn conversions_work() {
        assert_eq!(
            run_vm(
                "print(str(42) + \"!\")\nprint(int(\"123\") + 1)\nprint(float(3) / 2.0)\nprint(int(2.9))\nprint(int(true))\n"
            ),
            "42!\n124\n1.5\n2\n1\n"
        );
    }

    #[test]
    fn failed_conversion_can_be_caught() {
        assert_eq!(
            run_vm("try:\n    print(int(\"abc\"))\ncatch e:\n    print(e)\n"),
            "cannot convert \"abc\" to Int\n"
        );
    }

    #[test]
    fn push_appends() {
        assert_eq!(
            run_vm("mut xs = [1]\nxs.push(2)\nxs.push(3)\nprint(xs)\nprint(len(xs))\n"),
            "[1, 2, 3]\n3\n"
        );
    }

    // ----- enum and match: VM and tree interpreter must agree -----

    #[test]
    fn enum_print_and_equality() {
        same(
            "enum Color:\n    Red\n    Green\nlet a = Color.Red\nprint(a)\nprint(a == Color.Red)\nprint(a != Color.Green)\nprint(a == Color.Green)\n",
            "Color.Red\ntrue\ntrue\nfalse\n",
        );
    }

    #[test]
    fn match_on_enum() {
        same(
            "enum C:\n    A\n    B\n    D\nfn f(c: C) -> Int:\n    mut r = 0\n    match c:\n        C.A:\n            r = 1\n        C.B:\n            r = 2\n        C.D:\n            r = 3\n    return r\nprint(f(C.A))\nprint(f(C.B))\nprint(f(C.D))\n",
            "1\n2\n3\n",
        );
    }

    #[test]
    fn match_int_with_wildcard() {
        same(
            "for i in 0..3:\n    match i:\n        0:\n            print(\"zero\")\n        _:\n            print(\"other\")\n",
            "zero\nother\nother\n",
        );
    }

    #[test]
    fn match_string_and_bool() {
        same(
            "match \"hi\":\n    \"hi\":\n        print(1)\n    _:\n        print(2)\nmatch false:\n    true:\n        print(3)\n    false:\n        print(4)\n",
            "1\n4\n",
        );
    }

    #[test]
    fn match_nested_and_arm_scope() {
        same(
            "match 1:\n    1:\n        let x = 10\n        match 2:\n            2:\n                print(x)\n            _:\n                print(0)\n    _:\n        print(-1)\nlet x = 5\nprint(x)\n",
            "10\n5\n",
        );
    }

    #[test]
    fn match_subject_evaluated_once() {
        same(
            "mut n = 0\nfn next() -> Int:\n    return 1\nmatch next():\n    0:\n        print(0)\n    1:\n        print(1)\n    _:\n        print(2)\n",
            "1\n",
        );
    }

    // ----- tuples: VM and tree interpreter must agree -----

    #[test]
    fn tuple_print_index_len() {
        same(
            "let p = (1, \"a\", true)\nprint(p)\nprint(p[0])\nprint(p[1])\nprint(len(p))\n",
            "(1, \"a\", true)\n1\na\n3\n",
        );
    }

    #[test]
    fn tuple_equality_and_function() {
        same(
            "fn pair(a: Int) -> Int:\n    let t = (a, a + 1)\n    return t[1]\nprint(pair(4))\nprint((1, 2) == (1, 2))\nprint((1, 2) != (1, 3))\n",
            "5\ntrue\ntrue\n",
        );
    }
}

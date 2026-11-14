use super::{Chunk, ConvKind, Function, Instruction, IrProgram, StructInfo};
use crate::compiler::ast::*;
use crate::compiler::diagnostics::Diagnostic;
use crate::compiler::lexer::token::Span;
use crate::compiler::types::Type;
use std::collections::HashMap;

#[derive(Clone, Copy)]
enum VarRef {
    Local(usize),
    Global(usize),
}

struct LocalVar {
    name: String,
    slot: usize,
    depth: usize,
}

struct LoopCtx {
    start: usize,
    break_jumps: Vec<usize>,
    try_depth: usize,
}

struct FnState {
    chunk: Chunk,
    locals: Vec<LocalVar>,
    scope_depth: usize,
    next_slot: usize,
    loops: Vec<LoopCtx>,
    try_depth: usize,
    is_main: bool,
}

impl FnState {
    fn new(is_main: bool) -> Self {
        FnState {
            chunk: Chunk::default(),
            locals: Vec::new(),
            scope_depth: 0,
            next_slot: 0,
            loops: Vec::new(),
            try_depth: 0,
            is_main,
        }
    }
}

#[derive(Clone)]
struct FuncSig {
    params: Vec<String>,
    defaults: Vec<Option<Expr>>,
}

struct Compiler {
    state: FnState,
    globals: HashMap<String, usize>,
    functions: Vec<Function>,
    func_ids: HashMap<String, usize>,
    func_sigs: Vec<FuncSig>,
    struct_ids: HashMap<String, usize>,
    structs: Vec<StructInfo>,
    prefix: String,
    aliases: HashMap<String, String>,
    module_aliases: HashMap<String, String>,
    map_key_types: HashMap<(String, String), Type>,
}

fn err(message: impl Into<String>, span: &Span) -> Diagnostic {
    Diagnostic::new(message, span.clone())
}

/// Compiles a type-checked program to IR.
pub fn compile(program: &Program) -> Result<IrProgram, Diagnostic> {
    let mut c = Compiler::new();
    c.declare_items(program);
    for stmt in &program.statements {
        match &stmt.kind {
            StmtKind::Function(f) => c.compile_function(f, &stmt.span)?,
            StmtKind::Struct(_) => {}
            _ => c.compile_stmt(stmt)?,
        }
    }
    Ok(c.finish())
}

impl Compiler {
    fn new() -> Self {
        Compiler {
            state: FnState::new(true),
            globals: HashMap::new(),
            functions: Vec::new(),
            func_ids: HashMap::new(),
            func_sigs: Vec::new(),
            struct_ids: HashMap::new(),
            structs: Vec::new(),
            prefix: String::new(),
            aliases: HashMap::new(),
            module_aliases: HashMap::new(),
            map_key_types: HashMap::new(),
        }
    }

    fn finish(mut self) -> IrProgram {
        let span = Span { line: 1, column: 1 };
        self.emit(Instruction::PushNone, &span);
        self.emit(Instruction::Return, &span);
        IrProgram {
            main: Function {
                name: "<main>".to_string(),
                arity: 0,
                num_locals: self.state.next_slot,
                param_types: vec![],
                return_type: Type::None,
                chunk: self.state.chunk,
            },
            functions: self.functions,
            structs: self.structs,
            num_globals: self.globals.len(),
        }
    }

    /// Gives every struct and function an id before any code is compiled,
    /// so functions can be called before they are defined.
    fn declare_items(&mut self, program: &Program) {
        for stmt in &program.statements {
            match &stmt.kind {
                StmtKind::Struct(s) => {
                    let id = self.structs.len();
                    let k = self.key(&s.name);
                    self.struct_ids.insert(k, id);
                    self.structs.push(StructInfo {
                        name: s.name.clone(),
                        fields: s.fields.iter().map(|f| f.name.clone()).collect(),
                        field_types: s
                            .fields
                            .iter()
                            .map(|f| {
                                if !f.ty.elements.is_empty() {
                                    Type::Unknown
                                } else {
                                    Type::from_name(&f.ty.name).unwrap_or(Type::Unknown)
                                }
                            })
                            .collect(),
                    });
                }
                StmtKind::Function(f) => {
                    let id = self.functions.len();
                    let k = self.key(&f.name);
                    self.func_ids.insert(k, id);
                    self.func_sigs.push(FuncSig {
                        params: f.params.iter().map(|p| p.name.clone()).collect(),
                        defaults: f.params.iter().map(|p| p.default.clone()).collect(),
                    });
                    self.functions.push(Function {
                        name: f.name.clone(),
                        arity: f.params.len(),
                        num_locals: 0,
                        param_types: f
                            .params
                            .iter()
                            .map(|p| {
                                if !p.ty.elements.is_empty() {
                                    Type::Unknown
                                } else {
                                    Type::from_name(&p.ty.name).unwrap_or(Type::Unknown)
                                }
                            })
                            .collect(),
                        return_type: f
                            .return_type
                            .as_ref()
                            .map(|te| Type::from_name(&te.name).unwrap_or(Type::Unknown))
                            .unwrap_or(Type::None),
                        chunk: Chunk::default(),
                    });
                }
                _ => {}
            }
        }
    }

    // ---------- emit helpers ----------

    fn infer_expr_type(&self, e: &Expr) -> Type {
        match &e.kind {
            ExprKind::Int(_) => Type::Int,
            ExprKind::Float(_) => Type::Float,
            ExprKind::Str(_) => Type::String,
            ExprKind::Bool(_) => Type::Bool,
            ExprKind::NoneLit => Type::None,
            _ => Type::Unknown,
        }
    }

    fn emit(&mut self, ins: Instruction, span: &Span) {
        self.state.chunk.code.push(ins);
        self.state.chunk.spans.push(span.clone());
    }

    fn emit_jump(&mut self, ins: Instruction, span: &Span) -> usize {
        self.emit(ins, span);
        self.state.chunk.code.len() - 1
    }

    fn here(&self) -> usize {
        self.state.chunk.code.len()
    }

    /// Makes the jump at `index` land on the next instruction to be emitted.
    fn patch(&mut self, index: usize) {
        let target = self.here();
        match &mut self.state.chunk.code[index] {
            Instruction::Jump(t) | Instruction::JumpIfFalse(t) | Instruction::PushHandler(t) => {
                *t = target
            }
            Instruction::ForNext { exit, .. } => *exit = target,
            _ => {}
        }
    }

    // ---------- scopes and variables ----------

    fn begin_scope(&mut self) {
        self.state.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.state.scope_depth -= 1;
        let depth = self.state.scope_depth;
        while let Some(l) = self.state.locals.last() {
            if l.depth > depth {
                self.state.locals.pop();
            } else {
                break;
            }
        }
    }

    fn declare_local(&mut self, name: &str) -> usize {
        let slot = self.state.next_slot;
        self.state.next_slot += 1;
        self.state.locals.push(LocalVar {
            name: name.to_string(),
            slot,
            depth: self.state.scope_depth,
        });
        slot
    }

    /// Top-level variables (outside any block) are globals; everything else is a local.
    fn declare_var(&mut self, name: &str) -> VarRef {
        if self.state.is_main && self.state.scope_depth == 0 {
            let slot = match self.globals.get(&self.key(name)) {
                Some(&s) => s,
                None => {
                    let s = self.globals.len();
                    let gk = self.key(name);
                    self.globals.insert(gk, s);
                    s
                }
            };
            VarRef::Global(slot)
        } else {
            VarRef::Local(self.declare_local(name))
        }
    }

    fn resolve(&self, name: &str) -> Option<VarRef> {
        for l in self.state.locals.iter().rev() {
            if l.name == name {
                return Some(VarRef::Local(l.slot));
            }
        }
        self.globals
            .get(&self.key(name))
            .map(|&s| VarRef::Global(s))
    }

    fn emit_store(&mut self, var: VarRef, span: &Span) {
        match var {
            VarRef::Local(s) => self.emit(Instruction::StoreLocal(s), span),
            VarRef::Global(s) => self.emit(Instruction::StoreGlobal(s), span),
        }
    }

    // ---------- functions ----------

    fn compile_function(&mut self, f: &FunctionDecl, span: &Span) -> Result<(), Diagnostic> {
        let fkey = self.key(&f.name);
        let id = match self.func_ids.get(&fkey) {
            Some(&id) => id,
            None => return Err(err(format!("unknown function '{}'", f.name), span)),
        };
        let saved = std::mem::replace(&mut self.state, FnState::new(false));
        for p in &f.params {
            self.declare_local(&p.name);
        }
        let result = self.compile_stmts(&f.body);
        if result.is_ok() {
            self.emit(Instruction::PushNone, span);
            self.emit(Instruction::Return, span);
        }
        let state = std::mem::replace(&mut self.state, saved);
        result?;
        self.functions[id] = Function {
            name: f.name.clone(),
            arity: f.params.len(),
            num_locals: state.next_slot,
            param_types: f
                .params
                .iter()
                .map(|p| {
                    if !p.ty.elements.is_empty() {
                        Type::Unknown
                    } else {
                        Type::from_name(&p.ty.name).unwrap_or(Type::Unknown)
                    }
                })
                .collect(),
            return_type: f
                .return_type
                .as_ref()
                .map(|te| Type::from_name(&te.name).unwrap_or(Type::Unknown))
                .unwrap_or(Type::None),
            chunk: state.chunk,
        };
        Ok(())
    }

    // ---------- statements ----------

    fn compile_stmts(&mut self, stmts: &[Stmt]) -> Result<(), Diagnostic> {
        for s in stmts {
            self.compile_stmt(s)?;
        }
        Ok(())
    }

    fn compile_block(&mut self, stmts: &[Stmt]) -> Result<(), Diagnostic> {
        self.begin_scope();
        let result = self.compile_stmts(stmts);
        self.end_scope();
        result
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), Diagnostic> {
        let span = &stmt.span;
        match &stmt.kind {
            StmtKind::Let { name, value, .. } => {
                self.map_key_types.retain(|(v, _), _| v != name);
                self.compile_expr(value)?;
                let var = self.declare_var(name);
                self.emit_store(var, span);
            }
            StmtKind::Assign { target, value } => {
                let mut tracked = false;
                if let ExprKind::Ident(target_name) = &target.kind {
                    if let ExprKind::Call { callee, args } = &value.kind
                        && let ExprKind::Ident(fname) = &callee.kind
                        && fname == "map_set"
                        && args.len() == 3
                        && let (ExprKind::Ident(m_name), ExprKind::Str(key)) =
                            (&args[0].value.kind, &args[1].value.kind)
                        && m_name == target_name
                    {
                        let vt = self.infer_expr_type(&args[2].value);
                        self.map_key_types
                            .insert((target_name.clone(), key.clone()), vt);
                        tracked = true;
                    }
                    if !tracked {
                        self.map_key_types.retain(|(v, _), _| v != target_name);
                    }
                }
                self.compile_expr(value)?;
                self.emit_store_to(target)?;
            }
            StmtKind::Expr(Expr {
                kind: ExprKind::Call { callee, args },
                ..
            }) if matches!(&callee.kind, ExprKind::Field { name, .. } if name == "push") => {
                if args.len() != 1 {
                    return Err(err("push() takes exactly 1 argument", span));
                }
                if let ExprKind::Field { object, .. } = &callee.kind {
                    self.compile_expr(&args[0].value)?;
                    self.compile_expr(object)?;
                    self.emit(Instruction::ListPush, span);
                    self.emit_store_to(object)?;
                }
            }
            StmtKind::Expr(e) => {
                self.compile_expr(e)?;
                self.emit(Instruction::Pop, span);
            }
            StmtKind::Return(value) => {
                match value {
                    Some(e) => self.compile_expr(e)?,
                    None => self.emit(Instruction::PushNone, span),
                }
                self.emit(Instruction::Return, span);
            }
            StmtKind::Break => {
                let pops = match self.state.loops.last() {
                    Some(ctx) => self.state.try_depth - ctx.try_depth,
                    None => 0,
                };
                for _ in 0..pops {
                    self.emit(Instruction::PopHandler, span);
                }
                let jump = self.emit_jump(Instruction::Jump(0), span);
                match self.state.loops.last_mut() {
                    Some(ctx) => ctx.break_jumps.push(jump),
                    None => return Err(err("'break' outside of a loop", span)),
                }
            }
            StmtKind::Continue => {
                let pops = match self.state.loops.last() {
                    Some(ctx) => self.state.try_depth - ctx.try_depth,
                    None => 0,
                };
                for _ in 0..pops {
                    self.emit(Instruction::PopHandler, span);
                }
                let start = match self.state.loops.last() {
                    Some(ctx) => ctx.start,
                    None => return Err(err("'continue' outside of a loop", span)),
                };
                self.emit(Instruction::Jump(start), span);
            }
            StmtKind::If {
                condition,
                then_block,
                else_block,
            } => {
                self.compile_expr(condition)?;
                let jump_false = self.emit_jump(Instruction::JumpIfFalse(0), span);
                self.compile_block(then_block)?;
                match else_block {
                    Some(block) => {
                        let jump_end = self.emit_jump(Instruction::Jump(0), span);
                        self.patch(jump_false);
                        self.compile_block(block)?;
                        self.patch(jump_end);
                    }
                    None => self.patch(jump_false),
                }
            }
            StmtKind::Try {
                body,
                catch_var,
                catch_block,
            } => {
                let handler = self.emit_jump(Instruction::PushHandler(0), span);
                self.state.try_depth += 1;
                let body_result = self.compile_block(body);
                self.state.try_depth -= 1;
                body_result?;
                self.emit(Instruction::PopHandler, span);
                let jump_end = self.emit_jump(Instruction::Jump(0), span);
                self.patch(handler);
                self.begin_scope();
                let var = self.declare_var(catch_var);
                self.emit_store(var, span);
                let result = self.compile_stmts(catch_block);
                self.end_scope();
                result?;
                self.patch(jump_end);
            }
            StmtKind::Throw(e) => {
                self.compile_expr(e)?;
                self.emit(Instruction::Throw, span);
            }
            StmtKind::While { condition, body } => {
                let start = self.here();
                self.compile_expr(condition)?;
                let exit = self.emit_jump(Instruction::JumpIfFalse(0), span);
                self.state.loops.push(LoopCtx {
                    start,
                    break_jumps: Vec::new(),
                    try_depth: self.state.try_depth,
                });
                let result = self.compile_block(body);
                let ctx = self.state.loops.pop().unwrap();
                result?;
                self.emit(Instruction::Jump(start), span);
                self.patch(exit);
                for j in ctx.break_jumps {
                    self.patch(j);
                }
            }
            StmtKind::For { var, iter, body } => {
                self.compile_expr(iter)?;
                self.begin_scope();
                let iter_slot = self.declare_local("$iter");
                self.emit(Instruction::StoreLocal(iter_slot), span);
                self.emit(Instruction::PushInt(0), span);
                let idx_slot = self.declare_local("$idx");
                self.emit(Instruction::StoreLocal(idx_slot), span);
                let var_slot = self.declare_local(var);

                let start = self.here();
                let next = self.emit_jump(
                    Instruction::ForNext {
                        iter: iter_slot,
                        idx: idx_slot,
                        var: var_slot,
                        exit: 0,
                    },
                    span,
                );
                self.state.loops.push(LoopCtx {
                    start,
                    break_jumps: Vec::new(),
                    try_depth: self.state.try_depth,
                });
                let result = self.compile_block(body);
                let ctx = self.state.loops.pop().unwrap();
                result?;
                self.emit(Instruction::Jump(start), span);
                self.patch(next);
                for j in ctx.break_jumps {
                    self.patch(j);
                }
                self.end_scope();
            }
            // Already handled when the program was scanned.
            StmtKind::Function(_) | StmtKind::Struct(_) | StmtKind::Enum(_) => {}
            StmtKind::LetTuple {
                names,
                mutable,
                value,
            } => {
                for s in crate::compiler::ast::desugar_let_tuple(names, *mutable, value, span) {
                    self.compile_stmt(&s)?;
                }
            }
            StmtKind::Match { subject, arms } => {
                let desugared = crate::compiler::ast::desugar_match(subject, arms, span);
                self.compile_stmt(&desugared)?;
            }
            StmtKind::Import { .. } | StmtKind::FromImport { .. } => {}
        }
        Ok(())
    }

    /// Stores the value on top of the stack into an assignment target.
    fn emit_store_to(&mut self, target: &Expr) -> Result<(), Diagnostic> {
        let span = &target.span;
        match &target.kind {
            ExprKind::Ident(name) => {
                let var = self
                    .resolve(name)
                    .ok_or_else(|| err(format!("undefined variable '{}'", name), span))?;
                self.emit_store(var, span);
            }
            ExprKind::Index { object, index } => {
                self.compile_expr(object)?;
                self.compile_expr(index)?;
                self.emit(Instruction::StoreIndex, span);
                self.emit_store_to(object)?;
            }
            ExprKind::Field { object, name } => {
                self.compile_expr(object)?;
                self.emit(Instruction::SetField(name.clone()), span);
                self.emit_store_to(object)?;
            }
            _ => return Err(err("invalid assignment target", span)),
        }
        Ok(())
    }

    // ---------- expressions ----------

    fn compile_expr(&mut self, expr: &Expr) -> Result<(), Diagnostic> {
        let span = &expr.span;
        match &expr.kind {
            ExprKind::Int(n) => self.emit(Instruction::PushInt(*n), span),
            ExprKind::Float(x) => self.emit(Instruction::PushFloat(*x), span),
            ExprKind::Str(s) => self.emit(Instruction::PushStr(s.clone()), span),
            ExprKind::Bool(b) => self.emit(Instruction::PushBool(*b), span),
            ExprKind::NoneLit => self.emit(Instruction::PushNone, span),
            ExprKind::Ident(name) => match self.resolve(name) {
                Some(VarRef::Local(s)) => self.emit(Instruction::LoadLocal(s), span),
                Some(VarRef::Global(s)) => self.emit(Instruction::LoadGlobal(s), span),
                None => return Err(err(format!("undefined variable '{}'", name), span)),
            },
            ExprKind::Unary { op, operand } => {
                self.compile_expr(operand)?;
                match op {
                    UnaryOp::Neg => self.emit(Instruction::Neg, span),
                    UnaryOp::Not => self.emit(Instruction::Not, span),
                }
            }
            ExprKind::Binary { op, left, right } => match op {
                BinaryOp::And => {
                    self.compile_expr(left)?;
                    let jump_false = self.emit_jump(Instruction::JumpIfFalse(0), span);
                    self.compile_expr(right)?;
                    let jump_end = self.emit_jump(Instruction::Jump(0), span);
                    self.patch(jump_false);
                    self.emit(Instruction::PushBool(false), span);
                    self.patch(jump_end);
                }
                BinaryOp::Or => {
                    self.compile_expr(left)?;
                    let jump_false = self.emit_jump(Instruction::JumpIfFalse(0), span);
                    self.emit(Instruction::PushBool(true), span);
                    let jump_end = self.emit_jump(Instruction::Jump(0), span);
                    self.patch(jump_false);
                    self.compile_expr(right)?;
                    self.patch(jump_end);
                }
                _ => {
                    self.compile_expr(left)?;
                    self.compile_expr(right)?;
                    let ins = match op {
                        BinaryOp::Add => Instruction::Add,
                        BinaryOp::Sub => Instruction::Sub,
                        BinaryOp::Mul => Instruction::Mul,
                        BinaryOp::Div => Instruction::Div,
                        BinaryOp::Mod => Instruction::Mod,
                        BinaryOp::Eq => Instruction::Eq,
                        BinaryOp::NotEq => Instruction::NotEq,
                        BinaryOp::Lt => Instruction::Lt,
                        BinaryOp::LtEq => Instruction::LtEq,
                        BinaryOp::Gt => Instruction::Gt,
                        BinaryOp::GtEq => Instruction::GtEq,
                        BinaryOp::Range => Instruction::MakeRange,
                        BinaryOp::And | BinaryOp::Or => unreachable!(),
                    };
                    self.emit(ins, span);
                }
            },
            ExprKind::Tuple(items) => {
                for item in items {
                    self.compile_expr(item)?;
                }
                self.emit(Instruction::MakeTuple(items.len()), span);
            }
            ExprKind::List(items) => {
                for item in items {
                    self.compile_expr(item)?;
                }
                self.emit(Instruction::MakeList(items.len()), span);
            }
            ExprKind::Index { object, index } => {
                self.compile_expr(object)?;
                if let ExprKind::Int(n) = &index.kind {
                    self.emit(Instruction::IndexAt(*n), span);
                } else {
                    self.compile_expr(index)?;
                    self.emit(Instruction::Index, span);
                }
            }
            ExprKind::Field { object, name } => {
                if let ExprKind::Ident(en) = &object.kind
                    && self.resolve(en).is_none()
                    && !self.module_aliases.contains_key(en)
                {
                    self.emit(Instruction::PushEnum(en.clone(), name.clone()), span);
                    return Ok(());
                }
                self.compile_expr(object)?;
                self.emit(Instruction::GetField(name.clone()), span);
            }
            ExprKind::Call { callee, args } => self.compile_call(callee, args, span)?,
        }
        Ok(())
    }

    fn compile_call(
        &mut self,
        callee: &Expr,
        args: &[Argument],
        span: &Span,
    ) -> Result<(), Diagnostic> {
        if let ExprKind::Field { object, name: item } = &callee.kind
            && let ExprKind::Ident(m) = &object.kind
            && let Some(module) = self.module_aliases.get(m).cloned()
            && self.resolve(m).is_none()
        {
            if let Some(builtin) = crate::compiler::stdlib::builtin_name(&module, item) {
                let ident = Expr {
                    kind: ExprKind::Ident(builtin),
                    span: callee.span.clone(),
                };
                return self.compile_call(&ident, args, span);
            }
            let key = format!("{}::{}", module, item);
            if let Some(&id) = self.func_ids.get(&key) {
                let sig = self.func_sigs[id].clone();
                self.emit_bound_args(&sig.params, &sig.defaults, args, span)?;
                self.emit(Instruction::Call(id), span);
                return Ok(());
            }
            if let Some(&id) = self.struct_ids.get(&key) {
                let fields = self.structs[id].fields.clone();
                let no_defaults: Vec<Option<Expr>> = vec![None; fields.len()];
                self.emit_bound_args(&fields, &no_defaults, args, span)?;
                self.emit(Instruction::MakeStruct(id), span);
                return Ok(());
            }
            return Err(err(
                format!("module '{}' has no item '{}'", module, item),
                &callee.span,
            ));
        }
        let name = match &callee.kind {
            ExprKind::Ident(n) => n.clone(),
            _ => return Err(err("this value cannot be called", &callee.span)),
        };

        if name == "print" {
            for a in args {
                self.compile_expr(&a.value)?;
            }
            self.emit(Instruction::Print(args.len()), span);
            return Ok(());
        }

        if name == "matmul" {
            if args.len() != 2 {
                return Err(err("matmul() takes exactly 2 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(Instruction::MatMul, span);
            return Ok(());
        }

        if name == "linear" {
            if args.len() != 3 {
                return Err(err("linear() takes exactly 3 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(Instruction::MatMul, span);
            self.compile_expr(&args[2].value)?;
            self.emit(Instruction::Add, span);
            return Ok(());
        }

        if matches!(name.as_str(), "sum" | "mean" | "max" | "min" | "transpose") {
            if args.len() != 1 {
                return Err(err(format!("{}() takes exactly 1 argument", name), span));
            }
            self.compile_expr(&args[0].value)?;
            let ins = match name.as_str() {
                "sum" => Instruction::TensorSum,
                "mean" => Instruction::TensorMean,
                "max" => Instruction::TensorMax,
                "min" => Instruction::TensorMin,
                _ => Instruction::TensorTranspose,
            };
            self.emit(ins, span);
            return Ok(());
        }

        if matches!(
            name.as_str(),
            "exp" | "log" | "sqrt" | "tanh" | "relu" | "sigmoid"
        ) {
            if args.len() != 1 {
                return Err(err(format!("{}() takes exactly 1 argument", name), span));
            }
            self.compile_expr(&args[0].value)?;
            let kind = match name.as_str() {
                "exp" => 'e',
                "log" => 'l',
                "sqrt" => 'q',
                "tanh" => 't',
                "relu" => 'r',
                _ => 's',
            };
            self.emit(Instruction::TensorMap(kind), span);
            return Ok(());
        }

        if name == "softmax" {
            if args.len() != 1 {
                return Err(err("softmax() takes exactly 1 argument", span));
            }
            self.compile_expr(&args[0].value)?;
            self.emit(Instruction::TensorSoftmax, span);
            return Ok(());
        }
        if name == "conv2d" {
            if args.len() != 2 {
                return Err(err("conv2d() takes exactly 2 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(Instruction::Conv2d, span);
            return Ok(());
        }
        if name == "mse" {
            if args.len() != 2 {
                return Err(err("mse() takes exactly 2 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(Instruction::TensorMse, span);
            return Ok(());
        }
        if name == "sgd" {
            if args.len() != 3 {
                return Err(err("sgd() takes exactly 3 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.compile_expr(&args[2].value)?;
            self.emit(Instruction::TensorSgd, span);
            return Ok(());
        }
        if name == "track" {
            if args.len() != 1 {
                return Err(err("track() takes exactly 1 argument", span));
            }
            self.compile_expr(&args[0].value)?;
            self.emit(Instruction::Track, span);
            return Ok(());
        }

        if name == "grad" {
            if args.len() != 2 {
                return Err(err("grad() takes exactly 2 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(Instruction::Grad, span);
            return Ok(());
        }

        if name == "reshape" {
            if args.len() != 2 {
                return Err(err("reshape() takes exactly 2 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(Instruction::TensorReshape, span);
            return Ok(());
        }

        if name == "Tensor" || name == "zeros" || name == "ones" || name == "shape" {
            if args.len() != 1 {
                return Err(err(format!("{}() takes exactly 1 argument", name), span));
            }
            self.compile_expr(&args[0].value)?;
            let ins = match name.as_str() {
                "Tensor" => Instruction::MakeTensor,
                "zeros" => Instruction::FillTensor(0.0),
                "ones" => Instruction::FillTensor(1.0),
                _ => Instruction::TensorShape,
            };
            self.emit(ins, span);
            return Ok(());
        }

        if name == "str" || name == "int" || name == "float" {
            if args.len() != 1 {
                return Err(err(format!("{}() takes exactly 1 argument", name), span));
            }
            self.compile_expr(&args[0].value)?;
            let kind = match name.as_str() {
                "str" => ConvKind::Str,
                "int" => ConvKind::Int,
                _ => ConvKind::Float,
            };
            self.emit(Instruction::Convert(kind), span);
            return Ok(());
        }

        if name == "len" {
            if args.len() != 1 {
                return Err(err("len() takes exactly 1 argument", span));
            }
            self.compile_expr(&args[0].value)?;
            self.emit(Instruction::Len, span);
            return Ok(());
        }

        if name == "json_parse" {
            if args.len() != 1 {
                return Err(err("json_parse() takes exactly 1 argument", span));
            }
            self.compile_expr(&args[0].value)?;
            self.emit(Instruction::JsonParse, span);
            return Ok(());
        }
        if name == "json_stringify" {
            if args.len() != 1 {
                return Err(err("json_stringify() takes exactly 1 argument", span));
            }
            self.compile_expr(&args[0].value)?;
            self.emit(Instruction::JsonStringify, span);
            return Ok(());
        }

        if name == "udp_bind" {
            if args.len() != 2 {
                return Err(err("udp_bind() takes exactly 2 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(Instruction::UdpBind, span);
            return Ok(());
        }
        if name == "udp_send" {
            if args.len() != 4 {
                return Err(err("udp_send() takes exactly 4 arguments", span));
            }
            for a in args {
                self.compile_expr(&a.value)?;
            }
            self.emit(Instruction::UdpSend, span);
            return Ok(());
        }
        if name == "udp_recv" {
            if args.len() != 2 {
                return Err(err("udp_recv() takes exactly 2 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(Instruction::UdpRecv, span);
            return Ok(());
        }
        if name == "udp_close" {
            if args.len() != 1 {
                return Err(err("udp_close() takes exactly 1 argument", span));
            }
            self.compile_expr(&args[0].value)?;
            self.emit(Instruction::UdpClose, span);
            return Ok(());
        }
        if name == "tcp_connect" {
            if args.len() != 2 {
                return Err(err("tcp_connect() takes exactly 2 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(Instruction::TcpConnect, span);
            return Ok(());
        }
        if name == "tcp_send" {
            if args.len() != 2 {
                return Err(err("tcp_send() takes exactly 2 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(Instruction::TcpSend, span);
            return Ok(());
        }
        if name == "tcp_recv" {
            if args.len() != 2 {
                return Err(err("tcp_recv() takes exactly 2 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(Instruction::TcpRecv, span);
            return Ok(());
        }
        if name == "tcp_close" {
            if args.len() != 1 {
                return Err(err("tcp_close() takes exactly 1 argument", span));
            }
            self.compile_expr(&args[0].value)?;
            self.emit(Instruction::TcpClose, span);
            return Ok(());
        }

        if name == "http_post" {
            if args.len() != 2 {
                return Err(err("http_post() takes exactly 2 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(Instruction::HttpPost, span);
            return Ok(());
        }
        if name == "http_get" {
            if args.len() != 1 {
                return Err(err("http_get() takes exactly 1 argument", span));
            }
            self.compile_expr(&args[0].value)?;
            self.emit(Instruction::HttpGet, span);
            return Ok(());
        }

        if name == "sha256" {
            if args.len() != 1 {
                return Err(err("sha256() takes exactly 1 argument", span));
            }
            self.compile_expr(&args[0].value)?;
            self.emit(Instruction::Sha256, span);
            return Ok(());
        }
        if name == "sha512" {
            if args.len() != 1 {
                return Err(err("sha512() takes exactly 1 argument", span));
            }
            self.compile_expr(&args[0].value)?;
            self.emit(Instruction::Sha512, span);
            return Ok(());
        }
        if name == "hmac_sha256" {
            if args.len() != 2 {
                return Err(err("hmac_sha256() takes exactly 2 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(Instruction::HmacSha256, span);
            return Ok(());
        }

        let fkey = self.item_key(&name);

        if name == "abs" {
            if args.len() != 1 {
                return Err(err("abs() takes exactly 1 argument", span));
            }
            self.compile_expr(&args[0].value)?;
            self.emit(Instruction::MathAbs, span);
            return Ok(());
        }

        if name == "pow" {
            if args.len() != 2 {
                return Err(err("pow() takes exactly 2 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(Instruction::MathPow, span);
            return Ok(());
        }

        if matches!(name.as_str(), "floor" | "ceil" | "round") {
            if args.len() != 1 {
                return Err(err(format!("{}() takes exactly 1 argument", name), span));
            }
            self.compile_expr(&args[0].value)?;
            let ins = match name.as_str() {
                "floor" => Instruction::MathFloor,
                "ceil" => Instruction::MathCeil,
                _ => Instruction::MathRound,
            };
            self.emit(ins, span);
            return Ok(());
        }

        if name == "map_new" {
            self.emit(Instruction::MapNew, span);
            return Ok(());
        }
        if name == "map_set" {
            if args.len() != 3 {
                return Err(err("map_set() takes exactly 3 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.compile_expr(&args[2].value)?;
            self.emit(Instruction::MapSet, span);
            return Ok(());
        }
        if matches!(name.as_str(), "map_get" | "map_has") {
            if args.len() != 2 {
                return Err(err(format!("{}() takes exactly 2 arguments", name), span));
            }
            let ret_ty = if name == "map_get" {
                if let (ExprKind::Ident(m_name), ExprKind::Str(key)) =
                    (&args[0].value.kind, &args[1].value.kind)
                {
                    self.map_key_types
                        .get(&(m_name.clone(), key.clone()))
                        .cloned()
                        .unwrap_or(Type::Unknown)
                } else {
                    Type::Unknown
                }
            } else {
                Type::Bool
            };
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(
                if name == "map_get" {
                    Instruction::MapGet(ret_ty)
                } else {
                    Instruction::MapHas
                },
                span,
            );
            return Ok(());
        }
        if name == "map_len" {
            if args.len() != 1 {
                return Err(err("map_len() takes exactly 1 argument", span));
            }
            self.compile_expr(&args[0].value)?;
            self.emit(Instruction::MapLen, span);
            return Ok(());
        }
        if name == "trim" {
            if args.len() != 1 {
                return Err(err("trim() takes exactly 1 argument", span));
            }
            self.compile_expr(&args[0].value)?;
            self.emit(Instruction::StrTrim, span);
            return Ok(());
        }
        if name == "split" {
            if args.len() != 2 {
                return Err(err("split() takes exactly 2 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(Instruction::StrSplit, span);
            return Ok(());
        }
        if name == "replace" {
            if args.len() != 3 {
                return Err(err("replace() takes exactly 3 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.compile_expr(&args[2].value)?;
            self.emit(Instruction::StrReplace, span);
            return Ok(());
        }
        if name == "now" {
            if !args.is_empty() {
                return Err(err("now() takes exactly 0 arguments", span));
            }
            self.emit(Instruction::TimeNow, span);
            return Ok(());
        }

        if name == "sleep" {
            if args.len() != 1 {
                return Err(err("sleep() takes exactly 1 argument", span));
            }
            self.compile_expr(&args[0].value)?;
            self.emit(Instruction::TimeSleep, span);
            return Ok(());
        }

        if matches!(name.as_str(), "read_file" | "file_exists") {
            if args.len() != 1 {
                return Err(err(format!("{}() takes exactly 1 argument", name), span));
            }
            self.compile_expr(&args[0].value)?;
            let ins = if name == "read_file" {
                Instruction::ReadFile
            } else {
                Instruction::FileExists
            };
            self.emit(ins, span);
            return Ok(());
        }

        if name == "write_file" {
            if args.len() != 2 {
                return Err(err("write_file() takes exactly 2 arguments", span));
            }
            self.compile_expr(&args[0].value)?;
            self.compile_expr(&args[1].value)?;
            self.emit(Instruction::WriteFile, span);
            return Ok(());
        }

        if let Some(&id) = self.func_ids.get(&fkey) {
            let sig = self.func_sigs[id].clone();
            self.emit_bound_args(&sig.params, &sig.defaults, args, span)?;
            self.emit(Instruction::Call(id), span);
            return Ok(());
        }

        let skey = self.item_key(&name);
        if let Some(&id) = self.struct_ids.get(&skey) {
            let fields = self.structs[id].fields.clone();
            let no_defaults: Vec<Option<Expr>> = vec![None; fields.len()];
            self.emit_bound_args(&fields, &no_defaults, args, span)?;
            self.emit(Instruction::MakeStruct(id), span);
            return Ok(());
        }

        Err(err(format!("undefined function '{}'", name), &callee.span))
    }

    /// Emits arguments in parameter order: positional and named arguments
    /// are matched to parameters, missing ones use the default value.
    fn emit_bound_args(
        &mut self,
        names: &[String],
        defaults: &[Option<Expr>],
        args: &[Argument],
        span: &Span,
    ) -> Result<(), Diagnostic> {
        let mut slots: Vec<Option<&Expr>> = vec![None; names.len()];
        for (i, arg) in args.iter().enumerate() {
            let idx = match &arg.name {
                None => i,
                Some(n) => names
                    .iter()
                    .position(|x| x == n)
                    .ok_or_else(|| err(format!("no parameter named '{}'", n), span))?,
            };
            if idx >= slots.len() {
                return Err(err("too many arguments", span));
            }
            slots[idx] = Some(&arg.value);
        }

        for i in 0..names.len() {
            match slots[i] {
                Some(e) => self.compile_expr(e)?,
                None => match &defaults[i] {
                    Some(d) => {
                        // Default values can only see globals, never the caller's locals.
                        let saved = std::mem::take(&mut self.state.locals);
                        let result = self.compile_expr(d);
                        self.state.locals = saved;
                        result?;
                    }
                    None => {
                        return Err(err(format!("missing argument '{}'", names[i]), span));
                    }
                },
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::lexer::Lexer;
    use crate::compiler::parser::Parser;
    use Instruction as I;

    fn ir(src: &str) -> IrProgram {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse_program().unwrap();
        compile(&program).unwrap()
    }

    fn main_code(src: &str) -> Vec<Instruction> {
        ir(src).main.chunk.code
    }

    #[test]
    fn let_and_arithmetic() {
        assert_eq!(
            main_code("let x = 1 + 2\n"),
            vec![
                I::PushInt(1),
                I::PushInt(2),
                I::Add,
                I::StoreGlobal(0),
                I::PushNone,
                I::Return
            ]
        );
    }

    #[test]
    fn globals_and_locals() {
        let code = main_code("let a = 1\nif true:\n    let b = 2\n    print(b)\nprint(a)\n");
        assert!(code.contains(&I::LoadLocal(0)));
        assert!(code.contains(&I::LoadGlobal(0)));
        assert!(code.contains(&I::StoreLocal(0)));
    }

    #[test]
    fn if_else_jumps() {
        assert_eq!(
            main_code("mut x = 1\nif true:\n    x = 2\nelse:\n    x = 3\n"),
            vec![
                I::PushInt(1),
                I::StoreGlobal(0),
                I::PushBool(true),
                I::JumpIfFalse(7),
                I::PushInt(2),
                I::StoreGlobal(0),
                I::Jump(9),
                I::PushInt(3),
                I::StoreGlobal(0),
                I::PushNone,
                I::Return,
            ]
        );
    }

    #[test]
    fn while_loop_jumps() {
        assert_eq!(
            main_code("mut i = 0\nwhile i < 3:\n    i = i + 1\n"),
            vec![
                I::PushInt(0),
                I::StoreGlobal(0),
                I::LoadGlobal(0),
                I::PushInt(3),
                I::Lt,
                I::JumpIfFalse(11),
                I::LoadGlobal(0),
                I::PushInt(1),
                I::Add,
                I::StoreGlobal(0),
                I::Jump(2),
                I::PushNone,
                I::Return,
            ]
        );
    }

    #[test]
    fn function_and_call() {
        let p = ir("fn add(a: Int, b: Int) -> Int:\n    return a + b\nprint(add(1, 2))\n");
        assert_eq!(p.functions[0].name, "add");
        assert_eq!(p.functions[0].arity, 2);
        assert_eq!(
            p.functions[0].chunk.code,
            vec![
                I::LoadLocal(0),
                I::LoadLocal(1),
                I::Add,
                I::Return,
                I::PushNone,
                I::Return
            ]
        );
        assert_eq!(
            p.main.chunk.code,
            vec![
                I::PushInt(1),
                I::PushInt(2),
                I::Call(0),
                I::Print(1),
                I::Pop,
                I::PushNone,
                I::Return
            ]
        );
    }

    #[test]
    fn named_and_default_arguments() {
        let src = "fn f(a: Int, b: Int = 5) -> Int:\n    return a + b\nlet r = f(b = 1, a = 2)\nlet s = f(7)\n";
        assert_eq!(
            main_code(src),
            vec![
                I::PushInt(2),
                I::PushInt(1),
                I::Call(0),
                I::StoreGlobal(0),
                I::PushInt(7),
                I::PushInt(5),
                I::Call(0),
                I::StoreGlobal(1),
                I::PushNone,
                I::Return,
            ]
        );
    }

    #[test]
    fn struct_constructor_uses_field_order() {
        let p = ir("struct P:\n    x: Int\n    y: Int\nlet p = P(y = 2, x = 1)\n");
        assert_eq!(p.structs[0].fields, vec!["x".to_string(), "y".to_string()]);
        assert_eq!(
            p.main.chunk.code,
            vec![
                I::PushInt(1),
                I::PushInt(2),
                I::MakeStruct(0),
                I::StoreGlobal(0),
                I::PushNone,
                I::Return
            ]
        );
    }

    #[test]
    fn for_loop_shape() {
        let p = ir("for i in 0..3:\n    print(i)\n");
        let code = &p.main.chunk.code;
        assert_eq!(p.main.num_locals, 3);
        assert_eq!(code[2], I::MakeRange);
        assert_eq!(
            code[6],
            I::ForNext {
                iter: 0,
                idx: 1,
                var: 2,
                exit: 11
            }
        );
        assert_eq!(code[10], I::Jump(6));
    }

    #[test]
    fn short_circuit_and() {
        assert_eq!(
            main_code("let a = true and false\n"),
            vec![
                I::PushBool(true),
                I::JumpIfFalse(4),
                I::PushBool(false),
                I::Jump(5),
                I::PushBool(false),
                I::StoreGlobal(0),
                I::PushNone,
                I::Return,
            ]
        );
    }

    #[test]
    fn index_assignment() {
        assert_eq!(
            main_code("mut xs = [1, 2]\nxs[0] = 5\n"),
            vec![
                I::PushInt(1),
                I::PushInt(2),
                I::MakeList(2),
                I::StoreGlobal(0),
                I::PushInt(5),
                I::LoadGlobal(0),
                I::PushInt(0),
                I::StoreIndex,
                I::StoreGlobal(0),
                I::PushNone,
                I::Return,
            ]
        );
    }

    #[test]
    fn field_assignment() {
        assert_eq!(
            main_code("struct P:\n    x: Int\nmut p = P(x = 1)\np.x = 2\n"),
            vec![
                I::PushInt(1),
                I::MakeStruct(0),
                I::StoreGlobal(0),
                I::PushInt(2),
                I::LoadGlobal(0),
                I::SetField("x".to_string()),
                I::StoreGlobal(0),
                I::PushNone,
                I::Return,
            ]
        );
    }

    #[test]
    fn disassembly_lists_functions() {
        let text =
            ir("fn add(a: Int, b: Int) -> Int:\n    return a + b\nprint(add(1, 2))\n").to_string();
        assert!(text.contains("== main"));
        assert!(text.contains("== fn add"));
    }
}

impl Compiler {
    /// Name of an item declared in the module being compiled.
    /// The entry module has an empty prefix, so its names stay unchanged.
    fn key(&self, name: &str) -> String {
        if self.prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}::{}", self.prefix, name)
        }
    }

    /// Resolves a function or struct name: names brought in by
    /// `from m import x` first, then the current module's own items.
    fn item_key(&self, name: &str) -> String {
        match self.aliases.get(name) {
            Some(full) => full.clone(),
            None => self.key(name),
        }
    }
}

fn import_aliases(program: &Program) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for stmt in &program.statements {
        if let StmtKind::FromImport { module, names } = &stmt.kind {
            for item in names {
                let local = item.alias.clone().unwrap_or_else(|| item.name.clone());
                map.insert(local, format!("{}::{}", module, item.name));
            }
        }
    }
    map
}

/// Compiles modules (dependency order, entry module last) into one IR program.
/// Top-level code of imported modules runs first, once, before the entry module.
pub fn compile_modules(
    modules: &[crate::compiler::modules::Module],
) -> Result<IrProgram, Diagnostic> {
    let mut c = Compiler::new();
    let last = modules.len().saturating_sub(1);
    for (i, m) in modules.iter().enumerate() {
        c.prefix = if i == last {
            String::new()
        } else {
            m.name.clone()
        };
        c.declare_items(&m.program);
    }
    for (i, m) in modules.iter().enumerate() {
        c.prefix = if i == last {
            String::new()
        } else {
            m.name.clone()
        };
        c.aliases = import_aliases(&m.program);
        c.module_aliases = module_aliases_of(&m.program);
        for stmt in &m.program.statements {
            match &stmt.kind {
                StmtKind::Function(f) => c.compile_function(f, &stmt.span)?,
                StmtKind::Struct(_) => {}
                _ => c.compile_stmt(stmt)?,
            }
        }
    }
    Ok(c.finish())
}

#[cfg(test)]
mod module_tests {
    use super::*;
    use crate::compiler::ir::Instruction;
    use crate::compiler::lexer::Lexer;
    use crate::compiler::modules::Module;
    use crate::compiler::parser::Parser;
    use std::path::PathBuf;

    fn module(name: &str, src: &str) -> Module {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse_program().unwrap();
        Module {
            name: name.to_string(),
            path: PathBuf::new(),
            program,
        }
    }

    const LIB: &str =
        "fn helper() -> Int:\n    return 100\npub fn value() -> Int:\n    return helper() + 1\n";
    const MAIN: &str = "from lib import value\nfn helper() -> Int:\n    return 1\nprint(helper())\nprint(value())\n";

    #[test]
    fn same_function_name_in_two_modules_gets_two_functions() {
        let ir = compile_modules(&[module("lib", LIB), module("main", MAIN)]).unwrap();
        assert_eq!(ir.functions.len(), 3);
    }

    #[test]
    fn calls_go_to_the_right_module() {
        let ir = compile_modules(&[module("lib", LIB), module("main", MAIN)]).unwrap();
        // ids: lib::helper = 0, lib::value = 1, main's helper = 2
        assert!(ir.functions[1].chunk.code.contains(&Instruction::Call(0)));
        assert!(ir.main.chunk.code.contains(&Instruction::Call(2)));
        assert!(ir.main.chunk.code.contains(&Instruction::Call(1)));
    }
}

fn module_aliases_of(program: &Program) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for stmt in &program.statements {
        if let StmtKind::Import { module, alias } = &stmt.kind {
            map.insert(
                alias.clone().unwrap_or_else(|| module.clone()),
                module.clone(),
            );
        }
    }
    map
}

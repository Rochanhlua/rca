use crate::compiler::lexer::token::Span;
use std::fmt;

pub mod lower;

/// Stack-based bytecode instruction.
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    // constants
    PushInt(i64),
    PushFloat(f64),
    PushBool(bool),
    PushStr(String),
    PushNone,

    // variables (slot numbers resolved at compile time)
    LoadLocal(usize),
    StoreLocal(usize),
    LoadGlobal(usize),
    StoreGlobal(usize),

    // arithmetic and logic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Not,

    // comparison
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    MakeRange,

    // control flow (targets are instruction indexes)
    Jump(usize),
    /// Pops a Bool; jumps if it is false.
    JumpIfFalse(usize),
    /// For-loop step: reads the iterable in local `iter` (Range or List) and
    /// the counter in local `idx`. If finished, jumps to `exit`; otherwise
    /// stores the next element in local `var` and increments the counter.
    ForNext { iter: usize, idx: usize, var: usize, exit: usize },

    // functions
    Call(usize),
    Return,
    Pop,

    // lists
    MakeList(usize),
    Index,
    /// Stack: [value, list, index] -> new list
    StoreIndex,

    // structs
    /// Pops one value per field (in declaration order), pushes the struct.
    MakeStruct(usize),
    GetField(String),
    /// Stack: [value, struct] -> new struct
    SetField(String),

    // built-in
    /// Pops `n` values, prints them, pushes None.
    Print(usize),
}

#[derive(Debug, Clone, Default)]
pub struct Chunk {
    pub code: Vec<Instruction>,
    /// Source position of each instruction (same length as `code`).
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub arity: usize,
    pub num_locals: usize,
    pub chunk: Chunk,
}

#[derive(Debug, Clone)]
pub struct StructInfo {
    pub name: String,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct IrProgram {
    pub main: Function,
    pub functions: Vec<Function>,
    pub structs: Vec<StructInfo>,
    pub num_globals: usize,
}

fn write_chunk(f: &mut fmt::Formatter, chunk: &Chunk) -> fmt::Result {
    for (i, ins) in chunk.code.iter().enumerate() {
        writeln!(
            f,
            "{:04}  {:<28} ; line {}",
            i,
            format!("{:?}", ins),
            chunk.spans[i].line
        )?;
    }
    Ok(())
}

impl fmt::Display for IrProgram {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "== main (locals: {}, globals: {}) ==",
            self.main.num_locals, self.num_globals
        )?;
        write_chunk(f, &self.main.chunk)?;
        for func in &self.functions {
            writeln!(f)?;
            writeln!(
                f,
                "== fn {} (arity: {}, locals: {}) ==",
                func.name, func.arity, func.num_locals
            )?;
            write_chunk(f, &func.chunk)?;
        }
        Ok(())
    }
}

pub mod validate;

impl Instruction {
    /// Indexes of the instructions that can run right after this one.
    pub fn successors(&self, index: usize) -> Vec<usize> {
        match self {
            Instruction::Jump(t) => vec![*t],
            Instruction::JumpIfFalse(t) => vec![index + 1, *t],
            Instruction::ForNext { exit, .. } => vec![index + 1, *exit],
            Instruction::Return => vec![],
            _ => vec![index + 1],
        }
    }
}

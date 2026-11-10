use crate::compiler::lexer::token::Span;
use std::fmt;

pub mod lower;

/// Target type of a `Convert` instruction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConvKind {
    Str,
    Int,
    Float,
}

/// Stack-based bytecode instruction.
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    // constants
    PushInt(i64),
    PushFloat(f64),
    PushBool(bool),
    PushStr(String),
    /// Pushes an enum value (enum name, variant name).
    PushEnum(String, String),
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
    ForNext {
        iter: usize,
        idx: usize,
        var: usize,
        exit: usize,
    },

    // functions
    Call(usize),
    Return,
    Pop,

    // lists
    MakeList(usize),
    /// Pops n values, pushes a Tuple.
    MakeTuple(usize),
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

    // error handling
    /// Registers a catch handler; `catch_target` is the instruction index of the catch block.
    PushHandler(usize),
    /// Removes the innermost handler (the try block finished without error).
    PopHandler,
    /// Pops a String and raises it as an error.
    Throw,

    // built-in
    /// Pops a String or List, pushes its length.
    Len,
    /// Pops a List (of Float, nested), pushes a Tensor.
    MakeTensor,
    /// Pops a List of Int (the shape), pushes a Tensor filled with the value.
    FillTensor(f64),
    /// Pops a Tensor, pushes its shape as a List of Int.
    TensorShape,
    /// Pops two rank-2 Tensors (b then a), pushes the matrix product a x b.
    MatMul,
    /// Pops a Tensor, pushes the sum of all elements (Float).
    TensorSum,
    /// Pops a Tensor, pushes the mean (Float).
    TensorMean,
    /// Pops a Tensor, pushes the largest element (Float).
    TensorMax,
    /// Pops a Tensor, pushes the smallest element (Float).
    TensorMin,
    /// Pops a Tensor, pushes its transpose.
    TensorTranspose,
    /// Pops a Tensor, pushes it with an elementwise function applied.
    /// kind: 'e' exp, 'l' log, 'q' sqrt, 't' tanh, 'r' relu, 's' sigmoid.
    TensorMap(char),
    /// Pops a Tensor, pushes it as a differentiable (tracked) tensor.
    Track,
    /// Pops x then y (both tracked), pushes the gradient of y with respect to x.
    Grad,
    /// Pops a shape List then a Tensor, pushes the reshaped Tensor.
    TensorReshape,
    /// Pops a value, converts it, pushes the result.
    Convert(ConvKind),
    /// Stack: [value, list] -> new list with value appended.
    ListPush,
    /// Pops a String path, pushes the file's contents as a String.
    ReadFile,
    /// Pops path (deeper) and content (top), writes to the file, pushes None.
    WriteFile,
    /// Pops a String path, pushes a Bool (true if the file exists).
    FileExists,
    /// Pushes the current Unix timestamp in milliseconds.
    TimeNow,
    /// Pops an Int (milliseconds) and sleeps.
    TimeSleep,

    /// Pops a number, pushes its absolute value.
    MathAbs,
    /// Pops exponent (deeper) and base (top), pushes base^exp.
    MathPow,
    /// Pops a Float, pushes its floor.
    MathFloor,
    /// Pops a Float, pushes its ceiling.
    MathCeil,
    /// Pops a Float, pushes its rounded value.
    MathRound,
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
            Instruction::PushHandler(t) => vec![index + 1, *t],
            Instruction::ForNext { exit, .. } => vec![index + 1, *exit],
            Instruction::Return | Instruction::Throw => vec![],
            _ => vec![index + 1],
        }
    }
}

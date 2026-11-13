use crate::compiler::lexer::token::Span;
use crate::compiler::types::Type;
use std::fmt;

pub mod lower;
pub mod validate;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConvKind {
    Str,
    Int,
    Float,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    // constants
    PushInt(i64),
    PushFloat(f64),
    PushBool(bool),
    PushStr(String),
    PushEnum(String, String),
    PushNone,

    // variables
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

    // control flow
    Jump(usize),
    JumpIfFalse(usize),
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
    MakeTuple(usize),
    Index,
    IndexAt(i64),
    StoreIndex,

    // structs
    MakeStruct(usize),
    GetField(String),
    SetField(String),

    // built-in
    Print(usize),
    Len,
    MakeTensor,
    FillTensor(f64),
    TensorShape,
    MatMul,
    TensorSum,
    TensorMean,
    TensorMax,
    TensorMin,
    TensorTranspose,
    TensorMap(char),
    TensorSoftmax,
    Conv2d,
    TensorMse,
    TensorSgd,
    Track,
    Grad,
    TensorReshape,
    Convert(ConvKind),
    ListPush,

    // error handling
    PushHandler(usize),
    PopHandler,
    Throw,

    // file system
    ReadFile,
    WriteFile,
    FileExists,

    // time
    TimeNow,
    TimeSleep,

    // math
    MathAbs,
    MathPow,
    MathFloor,
    MathCeil,
    MathRound,

    // string
    StrSplit,
    StrTrim,
    StrReplace,

    // collections
    MapNew,
    MapSet,
    MapGet(Type),
    MapHas,
    MapLen,

    // json
    JsonParse,
    JsonStringify,

    // network (TCP)
    TcpConnect,
    TcpSend,
    TcpRecv,
    TcpClose,
    UdpBind,
    UdpSend,
    UdpRecv,
    UdpClose,

    // HTTP
    HttpGet,
    HttpPost,

    // crypto
    Sha256,
}

impl Instruction {
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

#[derive(Debug, Clone, Default)]
pub struct Chunk {
    pub code: Vec<Instruction>,
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub arity: usize,
    pub num_locals: usize,
    pub param_types: Vec<Type>,
    pub return_type: Type,
    pub chunk: Chunk,
}

#[derive(Debug, Clone)]
pub struct StructInfo {
    pub name: String,
    pub fields: Vec<String>,
    pub field_types: Vec<Type>,
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

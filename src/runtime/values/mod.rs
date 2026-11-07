use crate::compiler::ir::ConvKind;
use std::fmt;

/// A runtime value. Values are copied on assignment (value semantics).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    None,
    List(Vec<Value>),
    /// Start (inclusive) and end (exclusive), like `0..10`.
    Range(i64, i64),
    Struct { name: String, fields: Vec<(String, Value)> },
}

impl Value {
    /// Like Display, but strings are quoted (used inside lists and structs).
    fn repr(&self) -> String {
        match self {
            Value::Str(s) => format!("{:?}", s),
            _ => self.to_string(),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(x) => write!(f, "{:?}", x),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Str(s) => write!(f, "{}", s),
            Value::None => write!(f, "none"),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item.repr())?;
                }
                write!(f, "]")
            }
            Value::Range(a, b) => write!(f, "{}..{}", a, b),
            Value::Struct { name, fields } => {
                write!(f, "{}(", name)?;
                for (i, (n, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{} = {}", n, v.repr())?;
                }
                write!(f, ")")
            }
        }
    }
}

impl Value {
    /// Implements the built-in str(), int() and float() conversions.
    pub fn convert(self, kind: ConvKind) -> Result<Value, String> {
        match kind {
            ConvKind::Str => Ok(Value::Str(self.to_string())),
            ConvKind::Int => match self {
                Value::Int(n) => Ok(Value::Int(n)),
                Value::Float(x) => {
                    if x.is_finite() && x >= -9.2e18 && x <= 9.2e18 {
                        Ok(Value::Int(x as i64))
                    } else {
                        Err(format!("cannot convert {} to Int", x))
                    }
                }
                Value::Bool(b) => Ok(Value::Int(if b { 1 } else { 0 })),
                Value::Str(s) => s
                    .trim()
                    .parse::<i64>()
                    .map(Value::Int)
                    .map_err(|_| format!("cannot convert \"{}\" to Int", s)),
                _ => Err("cannot convert this value to Int".to_string()),
            },
            ConvKind::Float => match self {
                Value::Float(x) => Ok(Value::Float(x)),
                Value::Int(n) => Ok(Value::Float(n as f64)),
                Value::Str(s) => s
                    .trim()
                    .parse::<f64>()
                    .map(Value::Float)
                    .map_err(|_| format!("cannot convert \"{}\" to Float", s)),
                _ => Err("cannot convert this value to Float".to_string()),
            },
        }
    }
}

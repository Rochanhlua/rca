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

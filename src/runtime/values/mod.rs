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
    /// Row-major flat storage. Invariant: shape.iter().product() == data.len().
    Tensor { shape: Vec<usize>, data: Vec<f64> },
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
            Value::Tensor { shape, data } => {
                write!(f, "Tensor(shape=[")?;
                for (i, d) in shape.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", d)?;
                }
                write!(f, "], ")?;
                fmt_tensor(f, shape, data)?;
                write!(f, ")")
            }
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

/// Prints tensor data as nested brackets following the shape.
fn fmt_tensor(f: &mut fmt::Formatter, shape: &[usize], data: &[f64]) -> fmt::Result {
    if shape.len() <= 1 {
        write!(f, "[")?;
        for (i, x) in data.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{:?}", x)?;
        }
        return write!(f, "]");
    }
    let chunk = data.len() / shape[0].max(1);
    write!(f, "[")?;
    for i in 0..shape[0] {
        if i > 0 {
            write!(f, ", ")?;
        }
        fmt_tensor(f, &shape[1..], &data[i * chunk..(i + 1) * chunk])?;
    }
    write!(f, "]")
}

/// Why a tensor could not be built from a value.
pub fn tensor_from_value(v: &Value) -> Result<Value, String> {
    fn walk(v: &Value, depth: usize, shape: &mut Vec<usize>, data: &mut Vec<f64>) -> Result<(), String> {
        match v {
            Value::Float(x) => {
                if depth != shape.len() {
                    return Err("tensor rows must all have the same length".to_string());
                }
                data.push(*x);
                Ok(())
            }
            Value::List(items) => {
                if depth == shape.len() {
                    if !data.is_empty() {
                        return Err("tensor rows must all have the same length".to_string());
                    }
                    shape.push(items.len());
                } else if shape[depth] != items.len() {
                    return Err("tensor rows must all have the same length".to_string());
                }
                for item in items {
                    walk(item, depth + 1, shape, data)?;
                }
                Ok(())
            }
            _ => Err("Tensor() needs a List of Float values".to_string()),
        }
    }
    let mut shape = Vec::new();
    let mut data = Vec::new();
    walk(v, 0, &mut shape, &mut data)?;
    if shape.is_empty() {
        return Err("Tensor() needs a List".to_string());
    }
    let mut total: usize = 1;
    for d in &shape {
        total = total.checked_mul(*d).ok_or_else(|| "tensor is too large".to_string())?;
    }
    if total != data.len() {
        return Err("tensor rows must all have the same length".to_string());
    }
    Ok(Value::Tensor { shape, data })
}

#[cfg(test)]
mod tensor_tests {
    use super::*;

    fn f(x: f64) -> Value {
        Value::Float(x)
    }

    #[test]
    fn builds_1d_tensor() {
        let v = Value::List(vec![f(1.0), f(2.0), f(3.0)]);
        assert_eq!(tensor_from_value(&v), Ok(Value::Tensor { shape: vec![3], data: vec![1.0, 2.0, 3.0] }));
    }

    #[test]
    fn builds_2d_tensor() {
        let v = Value::List(vec![
            Value::List(vec![f(1.0), f(2.0)]),
            Value::List(vec![f(3.0), f(4.0)]),
        ]);
        assert_eq!(
            tensor_from_value(&v),
            Ok(Value::Tensor { shape: vec![2, 2], data: vec![1.0, 2.0, 3.0, 4.0] })
        );
    }

    #[test]
    fn ragged_input_is_error() {
        let v = Value::List(vec![
            Value::List(vec![f(1.0)]),
            Value::List(vec![f(2.0), f(3.0)]),
        ]);
        assert!(tensor_from_value(&v).is_err());
    }

    #[test]
    fn int_element_is_error() {
        let v = Value::List(vec![Value::Int(1)]);
        assert!(tensor_from_value(&v).is_err());
    }

    #[test]
    fn display_shows_shape_and_values() {
        let t = Value::Tensor { shape: vec![2, 2], data: vec![1.0, 2.0, 3.0, 4.0] };
        assert_eq!(t.to_string(), "Tensor(shape=[2, 2], [[1.0, 2.0], [3.0, 4.0]])");
    }
}

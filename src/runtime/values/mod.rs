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
    Tensor {
        shape: Vec<usize>,
        data: Vec<f64>,
    },
    Struct {
        name: String,
        fields: Vec<(String, Value)>,
    },
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
    fn walk(
        v: &Value,
        depth: usize,
        shape: &mut Vec<usize>,
        data: &mut Vec<f64>,
    ) -> Result<(), String> {
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
        total = total
            .checked_mul(*d)
            .ok_or_else(|| "tensor is too large".to_string())?;
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
        assert_eq!(
            tensor_from_value(&v),
            Ok(Value::Tensor {
                shape: vec![3],
                data: vec![1.0, 2.0, 3.0]
            })
        );
    }

    #[test]
    fn builds_2d_tensor() {
        let v = Value::List(vec![
            Value::List(vec![f(1.0), f(2.0)]),
            Value::List(vec![f(3.0), f(4.0)]),
        ]);
        assert_eq!(
            tensor_from_value(&v),
            Ok(Value::Tensor {
                shape: vec![2, 2],
                data: vec![1.0, 2.0, 3.0, 4.0]
            })
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
        let t = Value::Tensor {
            shape: vec![2, 2],
            data: vec![1.0, 2.0, 3.0, 4.0],
        };
        assert_eq!(
            t.to_string(),
            "Tensor(shape=[2, 2], [[1.0, 2.0], [3.0, 4.0]])"
        );
    }
}

/// Largest tensor RCA will allocate (elements). Bigger is a runtime error.
pub const MAX_TENSOR_ELEMENTS: usize = 10_000_000;

/// zeros([2, 3]) / ones([3]): builds a tensor filled with `fill` from a List of Int.
pub fn tensor_filled(v: &Value, fill: f64) -> Result<Value, String> {
    let items = match v {
        Value::List(i) => i,
        _ => return Err("shape must be a List of Int".to_string()),
    };
    if items.is_empty() {
        return Err("shape must not be empty".to_string());
    }
    let mut shape = Vec::new();
    let mut total: usize = 1;
    for it in items {
        match it {
            Value::Int(n) if *n >= 0 => {
                let d = *n as usize;
                total = total.checked_mul(d).ok_or("tensor is too large")?;
                shape.push(d);
            }
            _ => return Err("shape must be a List of non-negative Int".to_string()),
        }
    }
    if total > MAX_TENSOR_ELEMENTS {
        return Err("tensor is too large".to_string());
    }
    Ok(Value::Tensor {
        shape,
        data: vec![fill; total],
    })
}

/// shape(t): the shape of a tensor as a List of Int.
pub fn tensor_shape(v: &Value) -> Result<Value, String> {
    match v {
        Value::Tensor { shape, .. } => Ok(Value::List(
            shape.iter().map(|d| Value::Int(*d as i64)).collect(),
        )),
        _ => Err("shape() needs a Tensor".to_string()),
    }
}

#[cfg(test)]
mod tensor_fill_tests {
    use super::*;

    #[test]
    fn zeros_and_ones() {
        let shape = Value::List(vec![Value::Int(2), Value::Int(2)]);
        assert_eq!(
            tensor_filled(&shape, 0.0),
            Ok(Value::Tensor {
                shape: vec![2, 2],
                data: vec![0.0; 4]
            })
        );
        assert_eq!(
            tensor_filled(&Value::List(vec![Value::Int(3)]), 1.0),
            Ok(Value::Tensor {
                shape: vec![3],
                data: vec![1.0; 3]
            })
        );
    }

    #[test]
    fn bad_shapes_are_errors() {
        assert!(tensor_filled(&Value::List(vec![Value::Int(-1)]), 0.0).is_err());
        assert!(tensor_filled(&Value::List(vec![]), 0.0).is_err());
        assert!(tensor_filled(&Value::Int(3), 0.0).is_err());
        assert!(
            tensor_filled(
                &Value::List(vec![Value::Int(100000), Value::Int(100000)]),
                0.0
            )
            .is_err()
        );
    }

    #[test]
    fn shape_of_tensor() {
        let t = Value::Tensor {
            shape: vec![2, 3],
            data: vec![0.0; 6],
        };
        assert_eq!(
            tensor_shape(&t),
            Ok(Value::List(vec![Value::Int(2), Value::Int(3)]))
        );
        assert!(tensor_shape(&Value::Int(1)).is_err());
    }
}

/// Elementwise `+`, `-`, `*` (op is '+', '-' or '*').
/// Tensor op Tensor needs equal shapes (no broadcasting yet).
/// Tensor op Float and Float op Tensor apply the scalar to every element.
pub fn tensor_arith(op: char, l: &Value, r: &Value) -> Result<Value, String> {
    let f = |a: f64, b: f64| match op {
        '+' => a + b,
        '-' => a - b,
        _ => a * b,
    };
    match (l, r) {
        (
            Value::Tensor {
                shape: s1,
                data: d1,
            },
            Value::Tensor {
                shape: s2,
                data: d2,
            },
        ) => {
            if s1 != s2 {
                return Err(format!("tensor shapes do not match: {:?} and {:?}", s1, s2));
            }
            Ok(Value::Tensor {
                shape: s1.clone(),
                data: d1.iter().zip(d2.iter()).map(|(a, b)| f(*a, *b)).collect(),
            })
        }
        (Value::Tensor { shape, data }, Value::Float(x)) => Ok(Value::Tensor {
            shape: shape.clone(),
            data: data.iter().map(|a| f(*a, *x)).collect(),
        }),
        (Value::Float(x), Value::Tensor { shape, data }) => Ok(Value::Tensor {
            shape: shape.clone(),
            data: data.iter().map(|a| f(*x, *a)).collect(),
        }),
        _ => Err("tensor arithmetic needs a Tensor with a Tensor or a Float".to_string()),
    }
}

#[cfg(test)]
mod tensor_arith_tests {
    use super::*;

    fn t(shape: Vec<usize>, data: Vec<f64>) -> Value {
        Value::Tensor { shape, data }
    }

    #[test]
    fn tensor_with_tensor() {
        let a = t(vec![2], vec![1.0, 2.0]);
        let b = t(vec![2], vec![10.0, 20.0]);
        assert_eq!(tensor_arith('+', &a, &b), Ok(t(vec![2], vec![11.0, 22.0])));
        assert_eq!(tensor_arith('-', &b, &a), Ok(t(vec![2], vec![9.0, 18.0])));
        assert_eq!(tensor_arith('*', &a, &b), Ok(t(vec![2], vec![10.0, 40.0])));
    }

    #[test]
    fn tensor_with_scalar_both_sides() {
        let a = t(vec![2], vec![1.0, 2.0]);
        assert_eq!(
            tensor_arith('*', &a, &Value::Float(3.0)),
            Ok(t(vec![2], vec![3.0, 6.0]))
        );
        assert_eq!(
            tensor_arith('-', &Value::Float(10.0), &a),
            Ok(t(vec![2], vec![9.0, 8.0]))
        );
    }

    #[test]
    fn shape_mismatch_is_error() {
        let a = t(vec![2], vec![1.0, 2.0]);
        let b = t(vec![3], vec![1.0, 2.0, 3.0]);
        assert!(tensor_arith('+', &a, &b).is_err());
        assert!(tensor_arith('+', &a, &Value::Int(1)).is_err());
    }
}

/// t[i]: rank 1 gives a Float, rank 2+ gives the i-th sub-tensor (row).
pub fn tensor_index(v: &Value, i: i64) -> Result<Value, String> {
    let (shape, data) = match v {
        Value::Tensor { shape, data } => (shape, data),
        _ => return Err("cannot index into this value".to_string()),
    };
    if shape.is_empty() {
        return Err("cannot index a tensor with no dimensions".to_string());
    }
    if i < 0 || (i as usize) >= shape[0] {
        return Err(format!(
            "index {} out of bounds for tensor of length {}",
            i, shape[0]
        ));
    }
    let i = i as usize;
    if shape.len() == 1 {
        return Ok(Value::Float(data[i]));
    }
    let stride: usize = shape[1..].iter().product();
    Ok(Value::Tensor {
        shape: shape[1..].to_vec(),
        data: data[i * stride..(i + 1) * stride].to_vec(),
    })
}

#[cfg(test)]
mod tensor_index_tests {
    use super::*;

    #[test]
    fn rank1_gives_float() {
        let t = Value::Tensor {
            shape: vec![3],
            data: vec![1.0, 2.0, 3.0],
        };
        assert_eq!(tensor_index(&t, 1), Ok(Value::Float(2.0)));
    }

    #[test]
    fn rank2_gives_row() {
        let t = Value::Tensor {
            shape: vec![2, 3],
            data: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        };
        assert_eq!(
            tensor_index(&t, 1),
            Ok(Value::Tensor {
                shape: vec![3],
                data: vec![4.0, 5.0, 6.0]
            })
        );
    }

    #[test]
    fn bad_index_is_error() {
        let t = Value::Tensor {
            shape: vec![2, 3],
            data: vec![0.0; 6],
        };
        assert!(tensor_index(&t, 2).is_err());
        assert!(tensor_index(&t, -1).is_err());
        assert!(tensor_index(&Value::Int(1), 0).is_err());
    }
}

/// matmul(a, b): [m, n] x [n, p] = [m, p]. Rank 2 only for now.
pub fn tensor_matmul(l: &Value, r: &Value) -> Result<Value, String> {
    let (s1, d1, s2, d2) = match (l, r) {
        (
            Value::Tensor {
                shape: s1,
                data: d1,
            },
            Value::Tensor {
                shape: s2,
                data: d2,
            },
        ) => (s1, d1, s2, d2),
        _ => return Err("matmul() needs two Tensors".to_string()),
    };
    if s1.len() != 2 || s2.len() != 2 {
        return Err(format!(
            "matmul needs rank 2 tensors, found shapes {:?} and {:?}",
            s1, s2
        ));
    }
    let (m, n) = (s1[0], s1[1]);
    let (n2, p) = (s2[0], s2[1]);
    if n != n2 {
        return Err(format!("matmul shapes do not match: {:?} and {:?}", s1, s2));
    }
    match m.checked_mul(p) {
        Some(total) if total <= MAX_TENSOR_ELEMENTS => {}
        _ => return Err("tensor is too large".to_string()),
    }
    let mut out = vec![0.0; m * p];
    for i in 0..m {
        for k in 0..n {
            let a = d1[i * n + k];
            for j in 0..p {
                out[i * p + j] += a * d2[k * p + j];
            }
        }
    }
    Ok(Value::Tensor {
        shape: vec![m, p],
        data: out,
    })
}

#[cfg(test)]
mod tensor_matmul_tests {
    use super::*;

    fn t(shape: Vec<usize>, data: Vec<f64>) -> Value {
        Value::Tensor { shape, data }
    }

    #[test]
    fn square_matmul() {
        let a = t(vec![2, 2], vec![1.0, 2.0, 3.0, 4.0]);
        let b = t(vec![2, 2], vec![5.0, 6.0, 7.0, 8.0]);
        assert_eq!(
            tensor_matmul(&a, &b),
            Ok(t(vec![2, 2], vec![19.0, 22.0, 43.0, 50.0]))
        );
    }

    #[test]
    fn non_square_matmul() {
        let a = t(vec![2, 3], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let b = t(vec![3, 2], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(
            tensor_matmul(&a, &b),
            Ok(t(vec![2, 2], vec![22.0, 28.0, 49.0, 64.0]))
        );
    }

    #[test]
    fn bad_shapes_are_errors() {
        let a = t(vec![2, 3], vec![0.0; 6]);
        assert!(tensor_matmul(&a, &a).is_err());
        let v = t(vec![3], vec![0.0; 3]);
        assert!(tensor_matmul(&v, &a).is_err());
        assert!(tensor_matmul(&Value::Int(1), &a).is_err());
    }
}

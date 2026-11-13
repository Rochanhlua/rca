use crate::compiler::ir::ConvKind;
use std::fmt;
use std::sync::Arc;

/// A runtime value. Values are copied on assignment (value semantics).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    None,
    List(Vec<Value>),
    /// Key-value map. Uses Vec of pairs for simplicity and to preserve order.
    Map(Vec<(Value, Value)>),
    Tuple(Vec<Value>),
    /// Start (inclusive) and end (exclusive), like `0..10`.
    Range(i64, i64),
    /// Row-major flat storage. Invariant: shape.iter().product() == data.len().
    Tensor {
        shape: Vec<usize>,
        data: Arc<Vec<f64>>,
    },
    /// A tensor that remembers how it was computed (automatic differentiation).
    Tracked(Arc<crate::runtime::autodiff::Tracked>),
    Enum {
        name: String,
        variant: String,
    },
    /// A TCP socket.
    Socket(crate::runtime::net::Socket),
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
            Value::Socket(_) => "<socket>".to_string(),
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
            Value::Tuple(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item.repr())?;
                }
                write!(f, ")")
            }
            Value::Map(map) => write!(f, "Map(size={})", map.len()),
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
            Value::Tracked(t) => {
                write!(f, "Tensor(shape=[")?;
                for (i, d) in t.shape.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", d)?;
                }
                write!(f, "], ")?;
                fmt_tensor(f, &t.shape, &t.data)?;
                write!(f, ")")
            }
            Value::Enum { name, variant } => write!(f, "{}.{}", name, variant),
            Value::Socket(_) => write!(f, "<socket>"),
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
                    if x.is_finite() && (-9.2e18..=9.2e18).contains(&x) {
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
    Ok(Value::Tensor {
        shape,
        data: data.into(),
    })
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
                data: vec![1.0, 2.0, 3.0].into()
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
                data: vec![1.0, 2.0, 3.0, 4.0].into()
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
            data: vec![1.0, 2.0, 3.0, 4.0].into(),
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
        data: vec![fill; total].into(),
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
                data: vec![0.0; 4].into()
            })
        );
        assert_eq!(
            tensor_filled(&Value::List(vec![Value::Int(3)]), 1.0),
            Ok(Value::Tensor {
                shape: vec![3],
                data: vec![1.0; 3].into()
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
            data: vec![0.0; 6].into(),
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
    if matches!(l, Value::Tracked(_)) || matches!(r, Value::Tracked(_)) {
        return tracked_arith(op, l, r);
    }
    if let (
        Value::Tensor {
            shape: s1,
            data: d1,
        },
        Value::Tensor {
            shape: s2,
            data: d2,
        },
    ) = (l, r)
        && s1 != s2
    {
        return tensor_broadcast(op, s1, d1, s2, d2);
    }
    let f = |a: f64, b: f64| match op {
        '+' => a + b,
        '-' => a - b,
        '/' => a / b,
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
                data: d1
                    .iter()
                    .zip(d2.iter())
                    .map(|(a, b)| f(*a, *b))
                    .collect::<Vec<f64>>()
                    .into(),
            })
        }
        (Value::Tensor { shape, data }, Value::Float(x)) => Ok(Value::Tensor {
            shape: shape.clone(),
            data: data.iter().map(|a| f(*a, *x)).collect::<Vec<f64>>().into(),
        }),
        (Value::Float(x), Value::Tensor { shape, data }) => Ok(Value::Tensor {
            shape: shape.clone(),
            data: data.iter().map(|a| f(*x, *a)).collect::<Vec<f64>>().into(),
        }),
        _ => Err("tensor arithmetic needs a Tensor with a Tensor or a Float".to_string()),
    }
}

#[cfg(test)]
mod tensor_arith_tests {
    use super::*;

    fn t(shape: Vec<usize>, data: Vec<f64>) -> Value {
        Value::Tensor {
            shape,
            data: data.into(),
        }
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
        data: data[i * stride..(i + 1) * stride].to_vec().into(),
    })
}

#[cfg(test)]
mod tensor_index_tests {
    use super::*;

    #[test]
    fn rank1_gives_float() {
        let t = Value::Tensor {
            shape: vec![3],
            data: vec![1.0, 2.0, 3.0].into(),
        };
        assert_eq!(tensor_index(&t, 1), Ok(Value::Float(2.0)));
    }

    #[test]
    fn rank2_gives_row() {
        let t = Value::Tensor {
            shape: vec![2, 3],
            data: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0].into(),
        };
        assert_eq!(
            tensor_index(&t, 1),
            Ok(Value::Tensor {
                shape: vec![3],
                data: vec![4.0, 5.0, 6.0].into()
            })
        );
    }

    #[test]
    fn bad_index_is_error() {
        let t = Value::Tensor {
            shape: vec![2, 3],
            data: vec![0.0; 6].into(),
        };
        assert!(tensor_index(&t, 2).is_err());
        assert!(tensor_index(&t, -1).is_err());
        assert!(tensor_index(&Value::Int(1), 0).is_err());
    }
}

/// matmul(a, b): [m, n] x [n, p] = [m, p]. Rank 2 only for now.
pub fn tensor_matmul(l: &Value, r: &Value) -> Result<Value, String> {
    if matches!(l, Value::Tracked(_)) || matches!(r, Value::Tracked(_)) {
        return tracked_matmul(l, r);
    }
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
        data: out.into(),
    })
}

#[cfg(test)]
mod tensor_matmul_tests {
    use super::*;

    fn t(shape: Vec<usize>, data: Vec<f64>) -> Value {
        Value::Tensor {
            shape,
            data: data.into(),
        }
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

/// Strides for reading an input of shape `s` as if it had `rank` dimensions.
/// Missing leading dims and dims of size 1 get stride 0 (the value is reused).
fn broadcast_strides(s: &[usize], rank: usize) -> Vec<usize> {
    let mut st = vec![0usize; rank];
    let mut acc = 1usize;
    for i in (0..s.len()).rev() {
        let pos = i + rank - s.len();
        st[pos] = if s[i] == 1 { 0 } else { acc };
        acc *= s[i];
    }
    st
}

/// NumPy-style broadcasting: shapes are right-aligned; each pair of dims
/// must be equal or one of them must be 1.
fn tensor_broadcast(
    op: char,
    s1: &[usize],
    d1: &[f64],
    s2: &[usize],
    d2: &[f64],
) -> Result<Value, String> {
    let rank = s1.len().max(s2.len());
    let mut out_shape = vec![0usize; rank];
    for i in 0..rank {
        let a = if i + s1.len() >= rank {
            s1[i + s1.len() - rank]
        } else {
            1
        };
        let b = if i + s2.len() >= rank {
            s2[i + s2.len() - rank]
        } else {
            1
        };
        out_shape[i] = if a == b || b == 1 {
            a
        } else if a == 1 {
            b
        } else {
            return Err(format!("cannot broadcast shapes {:?} and {:?}", s1, s2));
        };
    }
    let mut total: usize = 1;
    for d in &out_shape {
        total = total.checked_mul(*d).ok_or("tensor is too large")?;
    }
    if total > MAX_TENSOR_ELEMENTS {
        return Err("tensor is too large".to_string());
    }
    let st1 = broadcast_strides(s1, rank);
    let st2 = broadcast_strides(s2, rank);
    let mut data = Vec::with_capacity(total);
    for flat in 0..total {
        let mut rem = flat;
        let (mut ia, mut ib) = (0usize, 0usize);
        for d in (0..rank).rev() {
            let idx = rem % out_shape[d];
            rem /= out_shape[d];
            ia += idx * st1[d];
            ib += idx * st2[d];
        }
        let (a, b) = (d1[ia], d2[ib]);
        data.push(match op {
            '+' => a + b,
            '-' => a - b,
            '/' => a / b,
            _ => a * b,
        });
    }
    Ok(Value::Tensor {
        shape: out_shape,
        data: data.into(),
    })
}

#[cfg(test)]
mod tensor_broadcast_tests {
    use super::*;

    fn t(shape: Vec<usize>, data: Vec<f64>) -> Value {
        Value::Tensor {
            shape,
            data: data.into(),
        }
    }

    #[test]
    fn row_vector_over_matrix() {
        let a = t(vec![2, 3], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let row = t(vec![3], vec![10.0, 20.0, 30.0]);
        assert_eq!(
            tensor_arith('+', &a, &row),
            Ok(t(vec![2, 3], vec![11.0, 22.0, 33.0, 14.0, 25.0, 36.0]))
        );
        assert_eq!(
            tensor_arith('+', &row, &a),
            Ok(t(vec![2, 3], vec![11.0, 22.0, 33.0, 14.0, 25.0, 36.0]))
        );
    }

    #[test]
    fn column_and_row_make_matrix() {
        let col = t(vec![2, 1], vec![100.0, 200.0]);
        let row = t(vec![3], vec![1.0, 2.0, 3.0]);
        assert_eq!(
            tensor_arith('+', &col, &row),
            Ok(t(
                vec![2, 3],
                vec![101.0, 102.0, 103.0, 201.0, 202.0, 203.0]
            ))
        );
    }

    #[test]
    fn incompatible_shapes_are_errors() {
        let a = t(vec![2, 3], vec![0.0; 6]);
        let b = t(vec![2], vec![0.0; 2]);
        assert!(tensor_arith('+', &a, &b).is_err());
        let c = t(vec![3], vec![0.0; 3]);
        let d = t(vec![4], vec![0.0; 4]);
        assert!(tensor_arith('*', &c, &d).is_err());
    }
}

/// sum / mean / max / min of every element. kind: 's', 'a' (mean), 'x' (max), 'n' (min).
pub fn tensor_reduce(v: &Value, kind: char) -> Result<Value, String> {
    if let Value::Tracked(t) = v {
        return tracked_reduce(t, kind);
    }
    let data = match v {
        Value::Tensor { data, .. } => data,
        _ => return Err("this function needs a Tensor".to_string()),
    };
    if data.is_empty() && kind != 's' {
        return Err("cannot take mean, max or min of an empty tensor".to_string());
    }
    let r = match kind {
        's' => data.iter().sum::<f64>(),
        'a' => data.iter().sum::<f64>() / data.len() as f64,
        'x' => data.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        _ => data.iter().cloned().fold(f64::INFINITY, f64::min),
    };
    Ok(Value::Float(r))
}

/// transpose(t): rank 1 is unchanged, rank 2 swaps rows and columns.
pub fn tensor_transpose(v: &Value) -> Result<Value, String> {
    if let Value::Tracked(t) = v {
        // The plain call validates the rank and gives the forward result.
        let plain = tensor_transpose(&plain_of(t))?;
        if t.shape.len() == 1 {
            return Ok(v.clone());
        }
        return match plain {
            Value::Tensor { shape, data } => Ok(Value::Tracked(Arc::new(
                crate::runtime::autodiff::transpose(t, shape, data),
            ))),
            _ => Err("transpose() needs a Tensor".to_string()),
        };
    }
    let (shape, data) = match v {
        Value::Tensor { shape, data } => (shape, data),
        _ => return Err("transpose() needs a Tensor".to_string()),
    };
    match shape.len() {
        1 => Ok(v.clone()),
        2 => {
            let (r, c) = (shape[0], shape[1]);
            let mut out = vec![0.0; r * c];
            for i in 0..r {
                for j in 0..c {
                    out[j * r + i] = data[i * c + j];
                }
            }
            Ok(Value::Tensor {
                shape: vec![c, r],
                data: out.into(),
            })
        }
        _ => Err("transpose() supports rank 1 and rank 2 tensors only".to_string()),
    }
}

/// reshape(t, [..]): same elements, new shape. Element count must stay equal.
pub fn tensor_reshape(v: &Value, new_shape: &Value) -> Result<Value, String> {
    if let Value::Tracked(t) = v {
        return match tensor_reshape(&plain_of(t), new_shape)? {
            Value::Tensor { shape, .. } => Ok(Value::Tracked(Arc::new(
                crate::runtime::autodiff::reshape(t, shape),
            ))),
            _ => Err("reshape() needs a Tensor".to_string()),
        };
    }
    let data = match v {
        Value::Tensor { data, .. } => data,
        _ => return Err("reshape() needs a Tensor".to_string()),
    };
    let items = match new_shape {
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
    if total != data.len() {
        return Err(format!(
            "cannot reshape tensor of {} elements into shape {:?}",
            data.len(),
            shape
        ));
    }
    Ok(Value::Tensor {
        shape,
        data: data.clone(),
    })
}

#[cfg(test)]
mod tensor_numeric_tests {
    use super::*;

    fn t(shape: Vec<usize>, data: Vec<f64>) -> Value {
        Value::Tensor {
            shape,
            data: data.into(),
        }
    }

    #[test]
    fn divide_tensor() {
        let a = t(vec![2], vec![1.0, 4.0]);
        assert_eq!(
            tensor_arith('/', &a, &Value::Float(2.0)),
            Ok(t(vec![2], vec![0.5, 2.0]))
        );
        let b = t(vec![2, 2], vec![2.0, 4.0, 6.0, 8.0]);
        assert_eq!(
            tensor_arith('/', &b, &t(vec![2], vec![2.0, 4.0])),
            Ok(t(vec![2, 2], vec![1.0, 1.0, 3.0, 2.0]))
        );
    }

    #[test]
    fn reductions() {
        let a = t(vec![2, 2], vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(tensor_reduce(&a, 's'), Ok(Value::Float(10.0)));
        assert_eq!(tensor_reduce(&a, 'a'), Ok(Value::Float(2.5)));
        assert_eq!(tensor_reduce(&a, 'x'), Ok(Value::Float(4.0)));
        assert_eq!(tensor_reduce(&a, 'n'), Ok(Value::Float(1.0)));
    }

    #[test]
    fn transpose_and_reshape() {
        let a = t(vec![2, 3], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(
            tensor_transpose(&a),
            Ok(t(vec![3, 2], vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]))
        );
        let shape = Value::List(vec![Value::Int(3), Value::Int(2)]);
        assert_eq!(
            tensor_reshape(&a, &shape),
            Ok(t(vec![3, 2], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]))
        );
    }

    #[test]
    fn bad_inputs_are_errors() {
        let a = t(vec![2, 3], vec![0.0; 6]);
        assert!(tensor_reshape(&a, &Value::List(vec![Value::Int(4)])).is_err());
        assert!(tensor_reshape(&a, &Value::List(vec![])).is_err());
        assert!(tensor_reduce(&t(vec![0], vec![]), 'x').is_err());
        assert!(tensor_reduce(&Value::Int(1), 's').is_err());
        assert!(tensor_transpose(&t(vec![1, 1, 1], vec![0.0])).is_err());
    }
}

#[cfg(test)]
mod enum_value_tests {
    use super::*;

    #[test]
    fn enum_prints_and_compares() {
        let a = Value::Enum {
            name: "Color".to_string(),
            variant: "Green".to_string(),
        };
        let b = Value::Enum {
            name: "Color".to_string(),
            variant: "Red".to_string(),
        };
        assert_eq!(format!("{}", a), "Color.Green");
        assert_eq!(a, a.clone());
        assert_ne!(a, b);
    }
}

/// Arithmetic where at least one operand is tracked. A plain Tensor or a Float
/// on the other side is treated as a constant (it gets no gradient).
fn tracked_arith(op: char, l: &Value, r: &Value) -> Result<Value, String> {
    use crate::runtime::autodiff;
    let constant = |other: &Value, like: &autodiff::Tracked| -> Result<autodiff::Tracked, String> {
        match other {
            Value::Tensor { shape, data } => Ok(autodiff::constant_like(shape, data, None)),
            Value::Float(x) => Ok(autodiff::constant_like(&like.shape, &like.data, Some(*x))),
            _ => Err("tracked tensors can only be combined with a Tensor or a Float".to_string()),
        }
    };
    let (a, b) = match (l, r) {
        (Value::Tracked(a), Value::Tracked(b)) => (a.clone(), b.clone()),
        (Value::Tracked(a), other) => (a.clone(), Arc::new(constant(other, a)?)),
        (other, Value::Tracked(b)) => (Arc::new(constant(other, b)?), b.clone()),
        _ => return Err("tracked arithmetic needs a tracked tensor".to_string()),
    };
    let t = match op {
        '+' => autodiff::add(&a, &b)?,
        '-' => autodiff::sub(&a, &b)?,
        '*' => autodiff::mul(&a, &b)?,
        '/' => autodiff::div(&a, &b)?,
        _ => return Err("this operator is not supported for tracked tensors yet".to_string()),
    };
    Ok(Value::Tracked(Arc::new(t)))
}

/// The plain Tensor holding the forward value of a tracked tensor.
fn plain_of(t: &crate::runtime::autodiff::Tracked) -> Value {
    Value::Tensor {
        shape: t.shape.clone(),
        data: t.data.clone(),
    }
}

/// matmul where at least one operand is tracked. A plain Tensor on the other
/// side is a constant. The forward result comes from the plain `tensor_matmul`.
fn tracked_matmul(l: &Value, r: &Value) -> Result<Value, String> {
    use crate::runtime::autodiff;
    let operand = |v: &Value| -> Result<Arc<autodiff::Tracked>, String> {
        match v {
            Value::Tracked(t) => Ok(t.clone()),
            Value::Tensor { shape, data } => {
                Ok(Arc::new(autodiff::constant_like(shape, data, None)))
            }
            _ => Err("matmul() needs two Tensors".to_string()),
        }
    };
    let a = operand(l)?;
    let b = operand(r)?;
    match tensor_matmul(&plain_of(&a), &plain_of(&b))? {
        Value::Tensor { shape, data } => Ok(Value::Tracked(Arc::new(autodiff::matmul(
            &a, &b, shape, data,
        )))),
        _ => Err("matmul() needs two Tensors".to_string()),
    }
}

/// sum / mean of a tracked tensor gives a tracked one-element Tensor (not a Float),
/// so that grad() can start from it. max and min are not differentiable yet.
fn tracked_reduce(t: &Arc<crate::runtime::autodiff::Tracked>, kind: char) -> Result<Value, String> {
    use crate::runtime::autodiff;
    match kind {
        's' => Ok(Value::Tracked(Arc::new(autodiff::sum(t)))),
        'a' => Ok(Value::Tracked(Arc::new(autodiff::mean(t)?))),
        _ => Err("max() and min() are not supported for tracked tensors yet".to_string()),
    }
}

#[cfg(test)]
mod tracked_matmul_tests {
    use super::*;

    fn t(shape: Vec<usize>, data: Vec<f64>) -> Value {
        Value::Tensor {
            shape,
            data: data.into(),
        }
    }

    /// sum(matmul(a, b)) computed with plain tensors only (the reference).
    fn plain_total(a: &[f64], b: &[f64]) -> f64 {
        let c = tensor_matmul(&t(vec![2, 3], a.to_vec()), &t(vec![3, 2], b.to_vec())).unwrap();
        match tensor_reduce(&c, 's').unwrap() {
            Value::Float(x) => x,
            _ => unreachable!(),
        }
    }

    fn tracked(shape: Vec<usize>, data: Vec<f64>) -> Value {
        tensor_track(&t(shape, data)).unwrap()
    }

    fn grad_data(y: &Value, x: &Value) -> Vec<f64> {
        match tensor_grad(y, x).unwrap() {
            Value::Tensor { data, .. } => data.to_vec(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn matmul_gradient_matches_numeric() {
        let a = vec![1.0, -2.0, 0.5, 3.0, 4.0, -1.5];
        let b = vec![2.0, 1.0, -1.0, 0.5, 3.0, -2.0];
        let ta = tracked(vec![2, 3], a.clone());
        let tb = tracked(vec![3, 2], b.clone());
        let y = tensor_reduce(&tensor_matmul(&ta, &tb).unwrap(), 's').unwrap();
        let ga = grad_data(&y, &ta);
        let gb = grad_data(&y, &tb);
        let h = 1e-6;
        for i in 0..6 {
            let mut hi = a.clone();
            let mut lo = a.clone();
            hi[i] += h;
            lo[i] -= h;
            let numeric = (plain_total(&hi, &b) - plain_total(&lo, &b)) / (2.0 * h);
            assert!((ga[i] - numeric).abs() < 1e-5, "dA[{}]", i);
            let mut hi = b.clone();
            let mut lo = b.clone();
            hi[i] += h;
            lo[i] -= h;
            let numeric = (plain_total(&a, &hi) - plain_total(&a, &lo)) / (2.0 * h);
            assert!((gb[i] - numeric).abs() < 1e-5, "dB[{}]", i);
        }
    }

    #[test]
    fn matmul_with_plain_operand_is_constant() {
        let w = tracked(vec![2, 1], vec![2.0, 1.0]);
        let x = t(vec![1, 2], vec![3.0, 4.0]);
        let y = tensor_matmul(&x, &w).unwrap();
        assert_eq!(grad_data(&y, &w), vec![3.0, 4.0]);
    }

    #[test]
    fn matmul_shape_errors_match_plain() {
        let a = tracked(vec![2, 3], vec![0.0; 6]);
        let b = tracked(vec![2, 2], vec![0.0; 4]);
        assert_eq!(
            tensor_matmul(&a, &b).unwrap_err(),
            tensor_matmul(&t(vec![2, 3], vec![0.0; 6]), &t(vec![2, 2], vec![0.0; 4])).unwrap_err()
        );
    }

    #[test]
    fn max_min_of_tracked_are_errors() {
        let a = tracked(vec![2], vec![1.0, 2.0]);
        assert!(tensor_reduce(&a, 'x').is_err());
        assert!(tensor_reduce(&a, 'n').is_err());
    }
}

#[cfg(test)]
mod tracked_elementwise_tests {
    use super::*;

    fn t(shape: Vec<usize>, data: Vec<f64>) -> Value {
        Value::Tensor {
            shape,
            data: data.into(),
        }
    }

    /// sum(map(x)) computed with plain tensors only (the reference).
    fn plain_total(kind: char, x: &[f64]) -> f64 {
        let y = tensor_map(&t(vec![x.len()], x.to_vec()), kind).unwrap();
        match tensor_reduce(&y, 's').unwrap() {
            Value::Float(v) => v,
            _ => unreachable!(),
        }
    }

    #[test]
    fn elementwise_gradients_match_numeric() {
        // Values avoid the kink of relu at 0 and are valid for log and sqrt when positive.
        let cases: [(char, [f64; 3]); 6] = [
            ('e', [0.3, -1.7, 2.5]),
            ('l', [0.3, 1.7, 2.5]),
            ('q', [0.3, 1.7, 2.5]),
            ('t', [0.3, -1.7, 2.5]),
            ('r', [-0.5, 0.4, 1.2]),
            ('s', [0.3, -1.7, 2.5]),
        ];
        for (kind, x) in cases {
            let tx = tensor_track(&t(vec![3], x.to_vec())).unwrap();
            let y = tensor_reduce(&tensor_map(&tx, kind).unwrap(), 's').unwrap();
            let g = match tensor_grad(&y, &tx).unwrap() {
                Value::Tensor { data, .. } => data.to_vec(),
                _ => unreachable!(),
            };
            let h = 1e-6;
            for i in 0..3 {
                let mut hi = x.to_vec();
                let mut lo = x.to_vec();
                hi[i] += h;
                lo[i] -= h;
                let numeric = (plain_total(kind, &hi) - plain_total(kind, &lo)) / (2.0 * h);
                assert!(
                    (g[i] - numeric).abs() < 1e-5,
                    "kind {} element {}: {} vs {}",
                    kind,
                    i,
                    g[i],
                    numeric
                );
            }
        }
    }

    #[test]
    fn domain_errors_match_plain() {
        let x = tensor_track(&t(vec![1], vec![-1.0])).unwrap();
        assert_eq!(
            tensor_map(&x, 'l').unwrap_err(),
            "log() needs positive values"
        );
        assert_eq!(
            tensor_map(&x, 'q').unwrap_err(),
            "sqrt() needs non-negative values"
        );
    }

    #[test]
    fn sqrt_at_zero_gradient_is_error() {
        let x = tensor_track(&t(vec![1], vec![0.0])).unwrap();
        let y = tensor_reduce(&tensor_map(&x, 'q').unwrap(), 's').unwrap();
        assert!(tensor_grad(&y, &x).is_err());
    }

    #[test]
    fn transpose_of_rank_1_is_same_tracked_value() {
        let x = tensor_track(&t(vec![2], vec![1.0, 2.0])).unwrap();
        assert_eq!(tensor_transpose(&x).unwrap(), x);
    }

    #[test]
    fn reshape_and_transpose_errors_match_plain() {
        let x = tensor_track(&t(vec![4], vec![0.0; 4])).unwrap();
        let bad = Value::List(vec![Value::Int(3)]);
        assert_eq!(
            tensor_reshape(&x, &bad).unwrap_err(),
            tensor_reshape(&t(vec![4], vec![0.0; 4]), &bad).unwrap_err()
        );
        let cube = tensor_track(&t(vec![1, 1, 1], vec![0.0])).unwrap();
        assert!(tensor_transpose(&cube).is_err());
    }
}

/// track(t): turn a Tensor into a differentiable leaf.
pub fn tensor_track(v: &Value) -> Result<Value, String> {
    match v {
        Value::Tensor { shape, data } => Ok(Value::Tracked(Arc::new(
            crate::runtime::autodiff::leaf(shape.clone(), data.clone()),
        ))),
        Value::Tracked(_) => Ok(v.clone()),
        _ => Err("track() needs a Tensor".to_string()),
    }
}

/// grad(y, x): gradient of y with respect to x, as a plain Tensor.
pub fn tensor_grad(y: &Value, x: &Value) -> Result<Value, String> {
    match (y, x) {
        (Value::Tracked(y), Value::Tracked(x)) => {
            let (shape, data) = crate::runtime::autodiff::grad(y, x)?;
            Ok(Value::Tensor {
                shape,
                data: data.into(),
            })
        }
        _ => Err("grad() needs two tracked Tensors (use track())".to_string()),
    }
}

/// Elementwise math on a Tensor.
/// kind: 'e' exp, 'l' log, 'q' sqrt, 't' tanh, 'r' relu, 's' sigmoid.
pub fn tensor_map(v: &Value, kind: char) -> Result<Value, String> {
    if let Value::Tracked(t) = v {
        return match tensor_map(&plain_of(t), kind)? {
            Value::Tensor { data, .. } => Ok(Value::Tracked(Arc::new(
                crate::runtime::autodiff::map(kind, t, data),
            ))),
            _ => Err("this function needs a Tensor".to_string()),
        };
    }
    let (shape, data) = match v {
        Value::Tensor { shape, data } => (shape, data),
        _ => return Err("this function needs a Tensor".to_string()),
    };
    let name = match kind {
        'e' => "exp",
        'l' => "log",
        'q' => "sqrt",
        't' => "tanh",
        'r' => "relu",
        _ => "sigmoid",
    };
    let mut out = Vec::with_capacity(data.len());
    for &x in data.iter() {
        let y = match kind {
            'e' => x.exp(),
            'l' => {
                if x <= 0.0 {
                    return Err("log() needs positive values".to_string());
                }
                x.ln()
            }
            'q' => {
                if x < 0.0 {
                    return Err("sqrt() needs non-negative values".to_string());
                }
                x.sqrt()
            }
            't' => x.tanh(),
            'r' => {
                if x > 0.0 {
                    x
                } else {
                    0.0
                }
            }
            _ => {
                if x >= 0.0 {
                    1.0 / (1.0 + (-x).exp())
                } else {
                    let e = x.exp();
                    e / (1.0 + e)
                }
            }
        };
        if !y.is_finite() {
            return Err(format!("{}() result is not finite", name));
        }
        out.push(y);
    }
    Ok(Value::Tensor {
        shape: shape.clone(),
        data: out.into(),
    })
}

pub fn tensor_linear(x: &Value, w: &Value, b: &Value) -> Result<Value, String> {
    let xw = tensor_matmul(x, w)?;
    tensor_arith('+', &xw, b)
}
#[cfg(test)]
mod tensor_map_tests {
    use super::*;

    fn t(shape: Vec<usize>, data: Vec<f64>) -> Value {
        Value::Tensor {
            shape,
            data: data.into(),
        }
    }
    fn data_of(v: &Value) -> Vec<f64> {
        match v {
            Value::Tensor { data, .. } => data.to_vec(),
            _ => panic!("not a tensor"),
        }
    }

    #[test]
    fn exp_log_sqrt_basic() {
        assert_eq!(
            data_of(&tensor_map(&t(vec![2], vec![0.0, 0.0]), 'e').unwrap()),
            vec![1.0, 1.0]
        );
        assert_eq!(
            data_of(&tensor_map(&t(vec![1], vec![1.0]), 'l').unwrap()),
            vec![0.0]
        );
        assert_eq!(
            data_of(&tensor_map(&t(vec![2], vec![4.0, 9.0]), 'q').unwrap()),
            vec![2.0, 3.0]
        );
    }

    #[test]
    fn relu_and_tanh() {
        let r = tensor_map(&t(vec![3], vec![-1.0, 0.0, 2.0]), 'r').unwrap();
        assert_eq!(data_of(&r), vec![0.0, 0.0, 2.0]);
        assert_eq!(
            data_of(&tensor_map(&t(vec![1], vec![0.0]), 't').unwrap()),
            vec![0.0]
        );
    }

    #[test]
    fn sigmoid_is_stable() {
        assert_eq!(
            data_of(&tensor_map(&t(vec![1], vec![0.0]), 's').unwrap()),
            vec![0.5]
        );
        let big = data_of(&tensor_map(&t(vec![2], vec![-1000.0, 1000.0]), 's').unwrap());
        assert_eq!(big, vec![0.0, 1.0]);
    }

    #[test]
    fn shape_is_kept() {
        let r = tensor_map(&t(vec![2, 2], vec![1.0, 4.0, 9.0, 16.0]), 'q').unwrap();
        match r {
            Value::Tensor { shape, .. } => assert_eq!(shape, vec![2, 2]),
            _ => panic!(),
        }
    }

    #[test]
    fn domain_and_overflow_errors() {
        assert!(tensor_map(&t(vec![1], vec![0.0]), 'l').is_err());
        assert!(tensor_map(&t(vec![1], vec![-1.0]), 'q').is_err());
        assert!(tensor_map(&t(vec![1], vec![1000.0]), 'e').is_err());
        assert!(tensor_map(&Value::Float(1.0), 'e').is_err());
    }
}

/// Softmax along the last axis. Subtracts per-slice max before exp for numerical stability.
pub fn tensor_softmax(v: &Value) -> Result<Value, String> {
    let (shape, data) = match v {
        Value::Tensor { shape, data } => (shape.clone(), data.clone()),
        _ => return Err("softmax() needs a Tensor".to_string()),
    };
    if shape.is_empty() {
        return Err("softmax() needs a tensor with at least one dimension".to_string());
    }
    let last = *shape.last().unwrap();
    if last == 0 {
        return Err("softmax() needs a non-empty last axis".to_string());
    }
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks(last) {
        let m = chunk.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exps: Vec<f64> = chunk.iter().map(|x| (*x - m).exp()).collect();
        let s: f64 = exps.iter().sum();
        if s == 0.0 || !s.is_finite() {
            return Err("softmax() normalization is not finite".to_string());
        }
        for e in exps {
            let y = e / s;
            if !y.is_finite() {
                return Err("softmax() result is not finite".to_string());
            }
            out.push(y);
        }
    }
    Ok(Value::Tensor {
        shape,
        data: out.into(),
    })
}

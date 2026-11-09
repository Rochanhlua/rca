//! Reverse-mode automatic differentiation.
//! Milestones 1-5 (5: broadcasting in + - * /): Add, Sub, Mul, Div, Sum, Mean, MatMul (rank 2), Transpose, Reshape and
//! elementwise math (exp, log, sqrt, tanh, relu, sigmoid); grad of a one-element result.
//! Plain tensors and Float scalars may be mixed in as constants (see `constant_like`).
//! Design: docs/autodiff-design.md

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Leaf,
    Add,
    Sub,
    Mul,
    Div,
    Sum,
    Mean,
    MatMul,
    Transpose,
    Reshape,
    Exp,
    Log,
    Sqrt,
    Tanh,
    Relu,
    Sigmoid,
}

/// One operation in the graph. A node's id is always larger than its parents' ids,
/// so decreasing id order is a valid reverse topological order.
#[derive(Debug, PartialEq)]
pub struct Node {
    pub id: u64,
    pub op: Op,
    pub parents: Vec<Arc<Node>>,
    /// Forward values the backward rule needs. Mul, Div and MatMul save both inputs;
    /// Log and Relu save their input; Exp, Sqrt, Tanh and Sigmoid save their output.
    pub saved: Vec<Arc<Vec<f64>>>,
    /// Sizes the backward rule needs: Sum and Mean store [element count],
    /// MatMul stores [m, k, n] for [m, k] x [k, n], Transpose stores [rows, cols]
    /// of its input. Empty for other ops.
    pub dims: Vec<usize>,
}

/// A tensor value that remembers how it was computed.
#[derive(Debug, PartialEq)]
pub struct Tracked {
    pub shape: Vec<usize>,
    pub data: Arc<Vec<f64>>,
    pub node: Arc<Node>,
}

/// A differentiable input (no parents).
pub fn leaf(shape: Vec<usize>, data: Arc<Vec<f64>>) -> Tracked {
    Tracked {
        shape,
        data,
        node: Arc::new(Node {
            id: next_id(),
            op: Op::Leaf,
            parents: vec![],
            saved: vec![],
            dims: vec![],
        }),
    }
}

fn broadcast_shape(s1: &[usize], s2: &[usize]) -> Result<Vec<usize>, String> {
    let rank = s1.len().max(s2.len());
    let mut out = vec![0usize; rank];
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
        out[i] = if a == b || b == 1 {
            a
        } else if a == 1 {
            b
        } else {
            return Err(format!("cannot broadcast shapes {:?} and {:?}", s1, s2));
        };
    }
    Ok(out)
}

fn broadcast_map(shape: &[usize], out_shape: &[usize]) -> Vec<usize> {
    let rank = out_shape.len();
    let total: usize = out_shape.iter().product();
    let mut strides = vec![0usize; rank];
    let mut acc = 1usize;
    for d in (0..shape.len()).rev() {
        let od = d + rank - shape.len();
        strides[od] = if shape[d] == 1 { 0 } else { acc };
        acc *= shape[d];
    }
    let mut map = Vec::with_capacity(total);
    for flat in 0..total {
        let mut rem = flat;
        let mut idx = 0usize;
        for d in (0..rank).rev() {
            let i = rem % out_shape[d];
            rem /= out_shape[d];
            idx += i * strides[d];
        }
        map.push(idx);
    }
    map
}

fn binary(
    op: Op,
    a: &Tracked,
    b: &Tracked,
    f: impl Fn(f64, f64) -> f64,
) -> Result<Tracked, String> {
    let out_shape = broadcast_shape(&a.shape, &b.shape)?;
    let ma = broadcast_map(&a.shape, &out_shape);
    let mb = broadcast_map(&b.shape, &out_shape);
    let data: Vec<f64> = ma
        .iter()
        .zip(mb.iter())
        .map(|(&i, &j)| f(a.data[i], b.data[j]))
        .collect();
    let saved = match op {
        Op::Mul | Op::Div => vec![a.data.clone(), b.data.clone()],
        _ => vec![],
    };
    let mut dims = vec![a.shape.len(), b.shape.len()];
    dims.extend(&a.shape);
    dims.extend(&b.shape);
    Ok(Tracked {
        shape: out_shape,
        data: Arc::new(data),
        node: Arc::new(Node {
            id: next_id(),
            op,
            parents: vec![a.node.clone(), b.node.clone()],
            saved,
            dims,
        }),
    })
}

pub fn add(a: &Tracked, b: &Tracked) -> Result<Tracked, String> {
    binary(Op::Add, a, b, |x, y| x + y)
}

pub fn sub(a: &Tracked, b: &Tracked) -> Result<Tracked, String> {
    binary(Op::Sub, a, b, |x, y| x - y)
}

pub fn mul(a: &Tracked, b: &Tracked) -> Result<Tracked, String> {
    binary(Op::Mul, a, b, |x, y| x * y)
}

pub fn div(a: &Tracked, b: &Tracked) -> Result<Tracked, String> {
    binary(Op::Div, a, b, |x, y| x / y)
}

/// Sum of every element, as a tracked one-element tensor.
pub fn sum(a: &Tracked) -> Tracked {
    let total: f64 = a.data.iter().sum();
    Tracked {
        shape: vec![1],
        data: Arc::new(vec![total]),
        node: Arc::new(Node {
            id: next_id(),
            op: Op::Sum,
            parents: vec![a.node.clone()],
            saved: vec![],
            dims: vec![a.data.len()],
        }),
    }
}

/// Mean of every element, as a tracked one-element tensor.
pub fn mean(a: &Tracked) -> Result<Tracked, String> {
    if a.data.is_empty() {
        return Err("cannot take mean, max or min of an empty tensor".to_string());
    }
    let n = a.data.len();
    let total: f64 = a.data.iter().sum();
    Ok(Tracked {
        shape: vec![1],
        data: Arc::new(vec![total / n as f64]),
        node: Arc::new(Node {
            id: next_id(),
            op: Op::Mean,
            parents: vec![a.node.clone()],
            saved: vec![],
            dims: vec![n],
        }),
    })
}

/// Matrix product node. The caller computes and validates the forward result
/// (shapes, size limits) with the plain `tensor_matmul`, so both paths agree.
/// `a` is [m, k] and `b` is [k, n]; `shape` and `data` are the [m, n] result.
pub fn matmul(a: &Tracked, b: &Tracked, shape: Vec<usize>, data: Arc<Vec<f64>>) -> Tracked {
    let dims = vec![a.shape[0], a.shape[1], b.shape[1]];
    Tracked {
        shape,
        data,
        node: Arc::new(Node {
            id: next_id(),
            op: Op::MatMul,
            parents: vec![a.node.clone(), b.node.clone()],
            saved: vec![a.data.clone(), b.data.clone()],
            dims,
        }),
    }
}

/// Transpose of a rank 2 tensor. The caller computes the forward result with the
/// plain `tensor_transpose`; `shape` and `data` are that result.
pub fn transpose(a: &Tracked, shape: Vec<usize>, data: Arc<Vec<f64>>) -> Tracked {
    Tracked {
        shape,
        data,
        node: Arc::new(Node {
            id: next_id(),
            op: Op::Transpose,
            parents: vec![a.node.clone()],
            saved: vec![],
            dims: vec![a.shape[0], a.shape[1]],
        }),
    }
}

/// Reshape: same elements in the same order, so the gradient passes straight through.
pub fn reshape(a: &Tracked, shape: Vec<usize>) -> Tracked {
    Tracked {
        shape,
        data: a.data.clone(),
        node: Arc::new(Node {
            id: next_id(),
            op: Op::Reshape,
            parents: vec![a.node.clone()],
            saved: vec![],
            dims: vec![],
        }),
    }
}

/// Elementwise math. kind: 'e' exp, 'l' log, 'q' sqrt, 't' tanh, 'r' relu, 's' sigmoid
/// (the same letters as `tensor_map`). `data` is the forward result, computed and
/// validated (domain, finiteness) by the plain `tensor_map`.
pub fn map(kind: char, a: &Tracked, data: Arc<Vec<f64>>) -> Tracked {
    let (op, saved) = match kind {
        'e' => (Op::Exp, data.clone()),
        'l' => (Op::Log, a.data.clone()),
        'q' => (Op::Sqrt, data.clone()),
        't' => (Op::Tanh, data.clone()),
        'r' => (Op::Relu, a.data.clone()),
        _ => (Op::Sigmoid, data.clone()),
    };
    Tracked {
        shape: a.shape.clone(),
        data,
        node: Arc::new(Node {
            id: next_id(),
            op,
            parents: vec![a.node.clone()],
            saved: vec![saved],
            dims: vec![],
        }),
    }
}

/// A plain tensor or Float scalar used together with a tracked tensor `like`.
/// It becomes a leaf nobody asks a gradient for, so it acts as a constant.
/// A Float is repeated to the shape of `like`. Shapes of a plain tensor are
/// checked by the operation itself.
pub fn constant_like(shape: &[usize], data: &Arc<Vec<f64>>, scalar: Option<f64>) -> Tracked {
    match scalar {
        Some(x) => leaf(shape.to_vec(), Arc::new(vec![x; data.len()])),
        None => leaf(shape.to_vec(), data.clone()),
    }
}

fn accumulate(grads: &mut HashMap<u64, Vec<f64>>, id: u64, g: &[f64]) {
    grads
        .entry(id)
        .and_modify(|existing| {
            for (e, v) in existing.iter_mut().zip(g) {
                *e += v;
            }
        })
        .or_insert_with(|| g.to_vec());
}

/// g[i] * local(g[i], saved[i]) for each element (the chain rule for one input).
fn scale(g: &[f64], saved: &[f64], local: impl Fn(f64, f64) -> f64) -> Vec<f64> {
    g.iter()
        .zip(saved.iter())
        .map(|(g, s)| g * local(*g, *s))
        .collect()
}

/// Sum a gradient of the broadcast shape back to the operand's own size.
fn unbroadcast(g: &[f64], map: &[usize], n: usize) -> Vec<f64> {
    let mut out = vec![0.0; n];
    for (i, &idx) in map.iter().enumerate() {
        out[idx] += g[i];
    }
    out
}

/// Backward rule for + - * / with broadcasting.
/// node.dims = [rank_a, rank_b, shape_a.., shape_b..].
fn binary_backward(node: &Node, g: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let ra = node.dims[0];
    let rb = node.dims[1];
    let sa = &node.dims[2..2 + ra];
    let sb = &node.dims[2 + ra..2 + ra + rb];
    let out_shape = broadcast_shape(sa, sb).expect("shapes were checked in the forward pass");
    let ma = broadcast_map(sa, &out_shape);
    let mb = broadcast_map(sb, &out_shape);
    let (ga, gb): (Vec<f64>, Vec<f64>) = match node.op {
        Op::Add => (g.to_vec(), g.to_vec()),
        Op::Sub => (g.to_vec(), g.iter().map(|v| -v).collect()),
        Op::Mul => {
            let a = &node.saved[0];
            let b = &node.saved[1];
            (
                g.iter().enumerate().map(|(i, v)| v * b[mb[i]]).collect(),
                g.iter().enumerate().map(|(i, v)| v * a[ma[i]]).collect(),
            )
        }
        Op::Div => {
            let a = &node.saved[0];
            let b = &node.saved[1];
            (
                g.iter().enumerate().map(|(i, v)| v / b[mb[i]]).collect(),
                g.iter()
                    .enumerate()
                    .map(|(i, v)| -v * a[ma[i]] / (b[mb[i]] * b[mb[i]]))
                    .collect(),
            )
        }
        _ => unreachable!(),
    };
    let na: usize = sa.iter().product();
    let nb: usize = sb.iter().product();
    (unbroadcast(&ga, &ma, na), unbroadcast(&gb, &mb, nb))
}

/// Gradient of `y` with respect to `x`. Returns (shape of x, gradient values).
pub fn grad(y: &Tracked, x: &Tracked) -> Result<(Vec<usize>, Vec<f64>), String> {
    if y.data.len() != 1 {
        return Err("grad() needs a result with exactly one element".to_string());
    }

    // Collect every node reachable from y.
    let mut nodes: HashMap<u64, Arc<Node>> = HashMap::new();
    let mut stack = vec![y.node.clone()];
    while let Some(n) = stack.pop() {
        if nodes.insert(n.id, n.clone()).is_some() {
            continue;
        }
        for p in &n.parents {
            stack.push(p.clone());
        }
    }
    if !nodes.contains_key(&x.node.id) {
        return Err("grad(): the result does not depend on this tensor".to_string());
    }

    let mut ids: Vec<u64> = nodes.keys().copied().collect();
    ids.sort_unstable_by(|a, b| b.cmp(a));

    let mut grads: HashMap<u64, Vec<f64>> = HashMap::new();
    grads.insert(y.node.id, vec![1.0]);

    for id in ids {
        let g = match grads.get(&id) {
            Some(g) => g.clone(),
            None => continue,
        };
        let node = &nodes[&id];
        if matches!(node.op, Op::Add | Op::Sub | Op::Mul | Op::Div) {
            let (ga, gb) = binary_backward(node, &g);
            accumulate(&mut grads, node.parents[0].id, &ga);
            accumulate(&mut grads, node.parents[1].id, &gb);
            continue;
        }
        match node.op {
            Op::Leaf => {}
            Op::Add => {
                accumulate(&mut grads, node.parents[0].id, &g);
                accumulate(&mut grads, node.parents[1].id, &g);
            }
            Op::Sum => {
                let ga = vec![g[0]; node.dims[0]];
                accumulate(&mut grads, node.parents[0].id, &ga);
            }
            Op::Mean => {
                let n = node.dims[0];
                let ga = vec![g[0] / n as f64; n];
                accumulate(&mut grads, node.parents[0].id, &ga);
            }
            Op::MatMul => {
                // C = A x B with A [m, k], B [k, n], G [m, n]:
                // dA = G x B^T, dB = A^T x G
                let (m, k, n) = (node.dims[0], node.dims[1], node.dims[2]);
                let a = &node.saved[0];
                let b = &node.saved[1];
                let mut ga = vec![0.0; m * k];
                let mut gb = vec![0.0; k * n];
                for i in 0..m {
                    for j in 0..n {
                        let gij = g[i * n + j];
                        for kk in 0..k {
                            ga[i * k + kk] += gij * b[kk * n + j];
                            gb[kk * n + j] += a[i * k + kk] * gij;
                        }
                    }
                }
                accumulate(&mut grads, node.parents[0].id, &ga);
                accumulate(&mut grads, node.parents[1].id, &gb);
            }
            Op::Transpose => {
                // The gradient has the output shape [c, r]; move it back to [r, c].
                let (r, c) = (node.dims[0], node.dims[1]);
                let mut ga = vec![0.0; r * c];
                for i in 0..r {
                    for j in 0..c {
                        ga[i * c + j] = g[j * r + i];
                    }
                }
                accumulate(&mut grads, node.parents[0].id, &ga);
            }
            Op::Reshape => {
                accumulate(&mut grads, node.parents[0].id, &g);
            }
            Op::Exp => {
                let ga = scale(&g, &node.saved[0], |_, y| y);
                accumulate(&mut grads, node.parents[0].id, &ga);
            }
            Op::Log => {
                let ga = scale(&g, &node.saved[0], |_, x| 1.0 / x);
                accumulate(&mut grads, node.parents[0].id, &ga);
            }
            Op::Sqrt => {
                let ga = scale(&g, &node.saved[0], |_, y| 0.5 / y);
                accumulate(&mut grads, node.parents[0].id, &ga);
            }
            Op::Tanh => {
                let ga = scale(&g, &node.saved[0], |_, y| 1.0 - y * y);
                accumulate(&mut grads, node.parents[0].id, &ga);
            }
            Op::Relu => {
                let ga = scale(&g, &node.saved[0], |_, x| if x > 0.0 { 1.0 } else { 0.0 });
                accumulate(&mut grads, node.parents[0].id, &ga);
            }
            Op::Sigmoid => {
                let ga = scale(&g, &node.saved[0], |_, y| y * (1.0 - y));
                accumulate(&mut grads, node.parents[0].id, &ga);
            }
            Op::Sub => {
                let neg: Vec<f64> = g.iter().map(|v| -v).collect();
                accumulate(&mut grads, node.parents[0].id, &g);
                accumulate(&mut grads, node.parents[1].id, &neg);
            }
            Op::Div => {
                // d(a/b)/da = 1/b, d(a/b)/db = -a/(b*b)
                let ga: Vec<f64> = g
                    .iter()
                    .zip(node.saved[1].iter())
                    .map(|(g, b)| g / b)
                    .collect();
                let gb: Vec<f64> = g
                    .iter()
                    .zip(node.saved[0].iter().zip(node.saved[1].iter()))
                    .map(|(g, (a, b))| -g * a / (b * b))
                    .collect();
                accumulate(&mut grads, node.parents[0].id, &ga);
                accumulate(&mut grads, node.parents[1].id, &gb);
            }
            Op::Mul => {
                let ga: Vec<f64> = g
                    .iter()
                    .zip(node.saved[1].iter())
                    .map(|(g, b)| g * b)
                    .collect();
                let gb: Vec<f64> = g
                    .iter()
                    .zip(node.saved[0].iter())
                    .map(|(g, a)| g * a)
                    .collect();
                accumulate(&mut grads, node.parents[0].id, &ga);
                accumulate(&mut grads, node.parents[1].id, &gb);
            }
        }
    }

    let result = grads
        .remove(&x.node.id)
        .ok_or_else(|| "grad(): no gradient reached this tensor".to_string())?;
    if result.iter().any(|v| !v.is_finite()) {
        return Err("grad(): gradient is not finite".to_string());
    }
    Ok((x.shape.clone(), result))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(v: f64) -> Tracked {
        leaf(vec![1], Arc::new(vec![v]))
    }

    fn g(y: &Tracked, x: &Tracked) -> f64 {
        grad(y, x).unwrap().1[0]
    }

    #[test]
    fn square_gradient() {
        let x = var(2.0);
        let y = mul(&x, &x).unwrap();
        assert_eq!(g(&y, &x), 4.0);
    }

    #[test]
    fn product_and_sum() {
        let x = var(3.0);
        let y = var(5.0);
        let z = add(&mul(&x, &y).unwrap(), &x).unwrap();
        assert_eq!(g(&z, &x), 6.0);
        assert_eq!(g(&z, &y), 3.0);
    }

    #[test]
    fn cube_by_chain() {
        let x = var(3.0);
        let y = mul(&mul(&x, &x).unwrap(), &x).unwrap();
        assert_eq!(g(&y, &x), 27.0);
    }

    fn f(v: f64) -> f64 {
        let x = var(v);
        let y = mul(&add(&mul(&x, &x).unwrap(), &x).unwrap(), &x).unwrap();
        y.data[0]
    }

    #[test]
    fn matches_numeric_gradient() {
        let x = var(1.5);
        let y = mul(&add(&mul(&x, &x).unwrap(), &x).unwrap(), &x).unwrap();
        let h = 1e-5;
        let numeric = (f(1.5 + h) - f(1.5 - h)) / (2.0 * h);
        assert!((g(&y, &x) - numeric).abs() < 1e-6);
    }

    #[test]
    fn sub_gradient() {
        let x = var(5.0);
        let y = var(3.0);
        let z = sub(&x, &y).unwrap();
        assert_eq!(z.data[0], 2.0);
        assert_eq!(g(&z, &x), 1.0);
        assert_eq!(g(&z, &y), -1.0);
    }

    #[test]
    fn sub_of_itself_is_zero() {
        let x = var(4.0);
        let z = sub(&x, &x).unwrap();
        assert_eq!(g(&z, &x), 0.0);
    }

    #[test]
    fn div_gradient() {
        let a = var(6.0);
        let b = var(2.0);
        let z = div(&a, &b).unwrap();
        assert_eq!(z.data[0], 3.0);
        assert_eq!(g(&z, &a), 0.5);
        assert_eq!(g(&z, &b), -1.5);
    }

    #[test]
    fn div_by_zero_gradient_is_error() {
        let a = var(1.0);
        let b = var(0.0);
        let z = div(&a, &b).unwrap();
        assert!(grad(&z, &a).is_err());
    }

    fn h(v: f64) -> f64 {
        // (x*x - x) / (x + 2)
        let x = var(v);
        let top = sub(&mul(&x, &x).unwrap(), &x).unwrap();
        let bottom = add(&x, &var(2.0)).unwrap();
        div(&top, &bottom).unwrap().data[0]
    }

    #[test]
    fn sub_div_match_numeric_gradient() {
        let x = var(1.5);
        let top = sub(&mul(&x, &x).unwrap(), &x).unwrap();
        let bottom = add(&x, &var(2.0)).unwrap();
        let y = div(&top, &bottom).unwrap();
        let step = 1e-5;
        let numeric = (h(1.5 + step) - h(1.5 - step)) / (2.0 * step);
        assert!((g(&y, &x) - numeric).abs() < 1e-6);
    }

    #[test]
    fn constant_gets_no_gradient_but_scales_result() {
        let x = var(3.0);
        let c = constant_like(&x.shape, &x.data, Some(4.0));
        let y = mul(&x, &c).unwrap();
        assert_eq!(y.data[0], 12.0);
        assert_eq!(g(&y, &x), 4.0);
    }

    #[test]
    fn sum_gradient() {
        let x = leaf(vec![3], Arc::new(vec![1.0, 2.0, 3.0]));
        let y = sum(&mul(&x, &x).unwrap());
        assert_eq!(y.data[0], 14.0);
        assert_eq!(grad(&y, &x).unwrap().1, vec![2.0, 4.0, 6.0]);
    }

    #[test]
    fn mean_gradient() {
        let x = leaf(vec![2], Arc::new(vec![2.0, 4.0]));
        let y = mean(&mul(&x, &x).unwrap()).unwrap();
        assert_eq!(y.data[0], 10.0);
        assert_eq!(grad(&y, &x).unwrap().1, vec![2.0, 4.0]);
    }

    #[test]
    fn mean_of_empty_is_error() {
        let x = leaf(vec![0], Arc::new(vec![]));
        assert!(mean(&x).is_err());
    }

    #[test]
    fn gradient_of_itself_is_one() {
        let x = var(7.0);
        assert_eq!(g(&x, &x), 1.0);
    }

    #[test]
    fn unrelated_tensor_is_error() {
        let x = var(2.0);
        let other = var(3.0);
        let y = mul(&x, &x).unwrap();
        assert!(grad(&y, &other).is_err());
    }

    #[test]
    fn non_scalar_result_is_error() {
        let x = leaf(vec![2], Arc::new(vec![1.0, 2.0]));
        let y = mul(&x, &x).unwrap();
        assert!(grad(&y, &x).is_err());
    }

    #[test]
    fn shape_mismatch_is_error() {
        let a = leaf(vec![2], Arc::new(vec![1.0, 2.0]));
        let b = leaf(vec![3], Arc::new(vec![1.0, 2.0, 3.0]));
        assert!(add(&a, &b).is_err());
        assert!(mul(&a, &b).is_err());
    }
}

/// Finite-difference gradient checks for every backward rule.
#[cfg(test)]
#[allow(clippy::needless_range_loop)]
mod gradcheck {
    use super::*;

    pub(super) fn t(shape: &[usize], data: &[f64]) -> Tracked {
        leaf(shape.to_vec(), Arc::new(data.to_vec()))
    }

    fn mm(a: &Tracked, b: &Tracked) -> Tracked {
        let (m, k, n) = (a.shape[0], a.shape[1], b.shape[1]);
        let mut out = vec![0.0; m * n];
        for i in 0..m {
            for j in 0..n {
                for p in 0..k {
                    out[i * n + j] += a.data[i * k + p] * b.data[p * n + j];
                }
            }
        }
        matmul(a, b, vec![m, n], Arc::new(out))
    }

    fn tr(a: &Tracked) -> Tracked {
        let (r, c) = (a.shape[0], a.shape[1]);
        let mut out = vec![0.0; r * c];
        for i in 0..r {
            for j in 0..c {
                out[j * r + i] = a.data[i * c + j];
            }
        }
        transpose(a, vec![c, r], Arc::new(out))
    }

    fn ew(kind: char, a: &Tracked) -> Tracked {
        let data: Vec<f64> = a
            .data
            .iter()
            .map(|v| match kind {
                'e' => v.exp(),
                'l' => v.ln(),
                'q' => v.sqrt(),
                't' => v.tanh(),
                'r' => v.max(0.0),
                _ => 1.0 / (1.0 + (-v).exp()),
            })
            .collect();
        map(kind, a, Arc::new(data))
    }

    /// Compares grad() with central finite differences for every element of x.
    pub(super) fn check(
        label: &str,
        shape: &[usize],
        data: &[f64],
        f: impl Fn(&Tracked) -> Tracked,
    ) {
        let x = t(shape, data);
        let y = f(&x);
        assert_eq!(y.data.len(), 1, "{label}: f must return one element");
        let (gshape, analytic) = grad(&y, &x).unwrap();
        assert_eq!(gshape, shape.to_vec(), "{label}: gradient shape");
        let h = 1e-5;
        for i in 0..data.len() {
            let mut plus = data.to_vec();
            let mut minus = data.to_vec();
            plus[i] += h;
            minus[i] -= h;
            let fp = f(&t(shape, &plus)).data[0];
            let fm = f(&t(shape, &minus)).data[0];
            let numeric = (fp - fm) / (2.0 * h);
            let tol = 1e-6 * analytic[i].abs().max(numeric.abs()).max(1.0);
            assert!(
                (analytic[i] - numeric).abs() < tol,
                "{label}[{i}]: analytic {} vs numeric {}",
                analytic[i],
                numeric
            );
        }
    }

    const V: [f64; 4] = [1.5, 2.5, -0.7, 3.0];
    const P: [f64; 4] = [0.5, 1.5, 2.5, 4.0];

    #[test]
    fn check_add_sub_mul_div() {
        let c = t(&[4], &[2.0, 3.0, 4.0, 5.0]);
        check("arith", &[4], &V, |x| {
            let num = sub(&mul(x, x).unwrap(), &c).unwrap();
            let den = add(&mul(x, x).unwrap(), &c).unwrap();
            sum(&div(&num, &den).unwrap())
        });
    }

    #[test]
    fn check_sum_and_mean() {
        check("sum", &[4], &V, |x| sum(&mul(x, x).unwrap()));
        check("mean", &[4], &V, |x| mean(&mul(x, x).unwrap()).unwrap());
    }

    #[test]
    fn check_elementwise_math() {
        check("exp", &[4], &V, |x| sum(&ew('e', x)));
        check("log", &[4], &P, |x| sum(&ew('l', x)));
        check("sqrt", &[4], &P, |x| sum(&ew('q', x)));
        check("tanh", &[4], &V, |x| sum(&ew('t', x)));
        check("sigmoid", &[4], &V, |x| sum(&ew('s', x)));
        check("relu", &[4], &[-1.3, 0.7, 2.2, -0.4], |x| {
            sum(&mul(&ew('r', x), x).unwrap())
        });
    }

    #[test]
    fn check_chain_of_ops() {
        check("chain", &[4], &P, |x| {
            let a = ew('e', &ew('s', x));
            let b = ew('l', &add(&mul(x, x).unwrap(), &a).unwrap());
            mean(&mul(&b, &ew('t', x)).unwrap()).unwrap()
        });
    }

    #[test]
    fn check_matmul_left_right_and_both() {
        let w = t(&[2, 2], &[1.0, -2.0, 3.0, 0.5]);
        let b = t(&[3, 2], &[1.0, 2.0, -1.0, 0.5, 2.0, -3.0]);
        let a = t(&[2, 3], &[0.5, -1.0, 2.0, 1.5, 0.3, -0.8]);
        let m23 = [1.0, 2.0, 3.0, -4.0, 5.0, 0.5];
        check("matmul left", &[2, 3], &m23, |x| {
            sum(&mul(&mm(x, &b), &w).unwrap())
        });
        check("matmul right", &[3, 2], &m23, |x| {
            sum(&mul(&mm(&a, x), &w).unwrap())
        });
        check("matmul both", &[2, 3], &m23, |x| {
            sum(&mul(&mm(x, &tr(x)), &w).unwrap())
        });
    }

    #[test]
    fn check_transpose_and_reshape() {
        let w = t(&[3, 2], &[1.0, 2.0, 3.0, -4.0, 5.0, 6.0]);
        check("transpose", &[2, 3], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], |x| {
            sum(&mul(&tr(x), &w).unwrap())
        });
        let m = t(&[2, 2], &[1.0, 2.0, 3.0, 4.0]);
        check("reshape", &[4], &V, |x| {
            sum(&mul(&reshape(x, vec![2, 2]), &m).unwrap())
        });
    }

    #[test]
    fn check_linear_regression_loss() {
        let xs = t(&[2, 2], &[1.0, 2.0, 3.0, 4.0]);
        let ys = t(&[2, 1], &[3.0, 6.0]);
        check("regression", &[2, 1], &[2.0, 1.0], |w| {
            let err = sub(&mm(&xs, w), &ys).unwrap();
            mean(&mul(&err, &err).unwrap()).unwrap()
        });
    }
}

/// Finite-difference checks for broadcasting in + - * /.
#[cfg(test)]
mod broadcast_gradcheck {
    use super::gradcheck::{check, t};
    use super::*;

    const M: [f64; 6] = [1.0, 2.0, 3.0, -4.0, 5.0, 0.5];

    fn sq(z: &Tracked) -> Tracked {
        sum(&mul(z, z).unwrap())
    }

    #[test]
    fn add_row_bias() {
        let m = t(&[2, 3], &M);
        let bias = t(&[3], &[0.5, -1.0, 2.0]);
        check(
            "bias",
            &[3],
            &[0.5, -1.0, 2.0],
            |b| sq(&add(&m, b).unwrap()),
        );
        check("matrix", &[2, 3], &M, |x| sq(&add(x, &bias).unwrap()));
    }

    #[test]
    fn sub_both_dims_broadcast() {
        let col = t(&[2, 1], &[1.5, -0.7]);
        let row = t(&[1, 3], &[2.0, 3.0, 4.0]);
        check("col", &[2, 1], &[1.5, -0.7], |x| sq(&sub(x, &row).unwrap()));
        check("row", &[1, 3], &[2.0, 3.0, 4.0], |x| {
            sq(&sub(&col, x).unwrap())
        });
    }

    #[test]
    fn mul_column_and_scalar() {
        let m = t(&[2, 3], &M);
        let col = t(&[2, 1], &[1.5, -0.7]);
        check("matrix", &[2, 3], &M, |x| sq(&mul(x, &col).unwrap()));
        check("col", &[2, 1], &[1.5, -0.7], |x| sq(&mul(&m, x).unwrap()));
        check("scalar", &[1], &[2.0], |x| sq(&mul(&m, x).unwrap()));
    }

    #[test]
    fn div_row_by_matrix() {
        let den = t(&[2, 3], &[1.5, 2.5, 0.5, 3.0, 2.0, 1.2]);
        let num = t(&[3], &[1.0, -2.0, 3.0]);
        check("num", &[3], &[1.0, -2.0, 3.0], |x| {
            sq(&div(x, &den).unwrap())
        });
        check("den", &[2, 3], &[1.5, 2.5, 0.5, 3.0, 2.0, 1.2], |x| {
            sq(&div(&num, x).unwrap())
        });
    }

    #[test]
    fn lower_rank_into_rank3() {
        let d: Vec<f64> = (1..=12).map(|v| v as f64 * 0.5).collect();
        let m3 = t(&[2, 2, 3], &d);
        check("rank3", &[2, 3], &M, |x| sq(&mul(&m3, x).unwrap()));
    }

    #[test]
    fn forward_values_follow_numpy_rule() {
        let a = t(&[2, 3], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let b = t(&[3], &[10.0, 20.0, 30.0]);
        let y = add(&a, &b).unwrap();
        assert_eq!(y.shape, vec![2, 3]);
        assert_eq!(*y.data, vec![11.0, 22.0, 33.0, 14.0, 25.0, 36.0]);
        let c = t(&[2, 1], &[100.0, 200.0]);
        let z = mul(&a, &c).unwrap();
        assert_eq!(*z.data, vec![100.0, 200.0, 300.0, 800.0, 1000.0, 1200.0]);
    }

    #[test]
    fn incompatible_shapes_error() {
        let e = add(&t(&[2], &[1.0, 2.0]), &t(&[3], &[1.0, 2.0, 3.0])).unwrap_err();
        assert_eq!(e, "cannot broadcast shapes [2] and [3]");
    }
}

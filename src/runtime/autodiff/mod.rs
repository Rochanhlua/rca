//! Reverse-mode automatic differentiation.
//! Milestone 1: Add and Mul on equal-shaped tensors, grad of a one-element result.
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
    Mul,
}

/// One operation in the graph. A node's id is always larger than its parents' ids,
/// so decreasing id order is a valid reverse topological order.
#[derive(Debug, PartialEq)]
pub struct Node {
    pub id: u64,
    pub op: Op,
    pub parents: Vec<Arc<Node>>,
    /// Forward values the backward rule needs (Mul saves both inputs).
    pub saved: Vec<Arc<Vec<f64>>>,
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
        }),
    }
}

fn check_same_shape(a: &Tracked, b: &Tracked) -> Result<(), String> {
    if a.shape != b.shape {
        return Err(
            "tracked tensors need equal shapes (broadcasting is not supported yet)".to_string(),
        );
    }
    Ok(())
}

pub fn add(a: &Tracked, b: &Tracked) -> Result<Tracked, String> {
    check_same_shape(a, b)?;
    let data: Vec<f64> = a
        .data
        .iter()
        .zip(b.data.iter())
        .map(|(x, y)| x + y)
        .collect();
    Ok(Tracked {
        shape: a.shape.clone(),
        data: Arc::new(data),
        node: Arc::new(Node {
            id: next_id(),
            op: Op::Add,
            parents: vec![a.node.clone(), b.node.clone()],
            saved: vec![],
        }),
    })
}

pub fn mul(a: &Tracked, b: &Tracked) -> Result<Tracked, String> {
    check_same_shape(a, b)?;
    let data: Vec<f64> = a
        .data
        .iter()
        .zip(b.data.iter())
        .map(|(x, y)| x * y)
        .collect();
    Ok(Tracked {
        shape: a.shape.clone(),
        data: Arc::new(data),
        node: Arc::new(Node {
            id: next_id(),
            op: Op::Mul,
            parents: vec![a.node.clone(), b.node.clone()],
            saved: vec![a.data.clone(), b.data.clone()],
        }),
    })
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
        match node.op {
            Op::Leaf => {}
            Op::Add => {
                accumulate(&mut grads, node.parents[0].id, &g);
                accumulate(&mut grads, node.parents[1].id, &g);
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
        let b = leaf(vec![1], Arc::new(vec![1.0]));
        assert!(add(&a, &b).is_err());
        assert!(mul(&a, &b).is_err());
    }
}

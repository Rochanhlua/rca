use rca::runtime::autodiff::{self, Op, add, div, leaf, mean, mul, reshape, sub, sum};
use std::sync::Arc;

fn tracked(shape: Vec<usize>, data: Vec<f64>) -> autodiff::Tracked {
    leaf(shape, Arc::new(data))
}

#[test]
fn test_leaf_creation() {
    let x = tracked(vec![2], vec![2.0, 3.0]);
    assert_eq!(x.shape, vec![2]);
    assert_eq!(&*x.data, &[2.0, 3.0]);
    assert_eq!(x.node.op, Op::Leaf);
}

#[test]
fn test_add_forward() {
    let a = tracked(vec![2], vec![1.0, 2.0]);
    let b = tracked(vec![2], vec![3.0, 4.0]);
    let c = add(&a, &b).unwrap();
    assert_eq!(&*c.data, &[4.0, 6.0]);
}

#[test]
fn test_sub_forward() {
    let a = tracked(vec![2], vec![5.0, 7.0]);
    let b = tracked(vec![2], vec![2.0, 3.0]);
    let c = sub(&a, &b).unwrap();
    assert_eq!(&*c.data, &[3.0, 4.0]);
}

#[test]
fn test_mul_forward() {
    let a = tracked(vec![2], vec![2.0, 3.0]);
    let b = tracked(vec![2], vec![4.0, 5.0]);
    let c = mul(&a, &b).unwrap();
    assert_eq!(&*c.data, &[8.0, 15.0]);
}

#[test]
fn test_div_forward() {
    let a = tracked(vec![2], vec![8.0, 9.0]);
    let b = tracked(vec![2], vec![2.0, 3.0]);
    let c = div(&a, &b).unwrap();
    assert_eq!(&*c.data, &[4.0, 3.0]);
}

#[test]
fn test_sum_forward() {
    let a = tracked(vec![3], vec![1.0, 2.0, 3.0]);
    let result = sum(&a);
    assert_eq!(result.shape, vec![1]);
    assert_eq!(&*result.data, &[6.0]);
}

#[test]
fn test_mean_forward() {
    let a = tracked(vec![3], vec![2.0, 4.0, 6.0]);
    let result = mean(&a).unwrap();
    assert_eq!(&*result.data, &[4.0]);
}

#[test]
fn test_mean_empty_tensor_error() {
    let a = tracked(vec![0], vec![]);
    assert!(mean(&a).is_err());
}

#[test]
fn test_reshape_preserves_data() {
    let a = tracked(vec![2, 2], vec![1.0, 2.0, 3.0, 4.0]);
    let result = reshape(&a, vec![4]);
    assert_eq!(result.shape, vec![4]);
    assert_eq!(&*result.data, &[1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn test_broadcast_add() {
    let a = tracked(vec![2, 1], vec![1.0, 2.0]);
    let b = tracked(vec![1, 3], vec![10.0, 20.0, 30.0]);
    let result = add(&a, &b).unwrap();
    assert_eq!(result.shape, vec![2, 3]);
    assert_eq!(&*result.data, &[11.0, 21.0, 31.0, 12.0, 22.0, 32.0]);
}

#[test]
fn test_incompatible_broadcast_error() {
    let a = tracked(vec![2], vec![1.0, 2.0]);
    let b = tracked(vec![3], vec![1.0, 2.0, 3.0]);
    assert!(add(&a, &b).is_err());
}

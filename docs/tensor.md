# RCA Tensors (v0.6 design)

Status: DESIGN ONLY. Nothing here is implemented yet.

## 1. Scope of the first version
- One new type: `Tensor`. Element type is Float (f64) only.
- CPU only. No GPU, no threads, no SIMD in this version.
- No automatic differentiation yet (separate milestone later).
- No performance claims until reproducible benchmarks exist.

## 2. Representation
- Row-major flat storage: `data: Vec<f64>` plus `shape: Vec<usize>`.
- Invariant: product of shape == data.len(). Always checked.
- Rank is not part of the static type (Tensor, not Tensor[2]). Shape errors are runtime errors.
- Value semantics, like every RCA value (see memory-model.md): assignment copies.
  Copy-on-write sharing is a later optimization, not a language change.
- Size limits: shape products use checked multiplication; a huge allocation is a runtime error, never a crash.

## 3. Construction
    let a = Tensor([[1.0, 2.0], [3.0, 4.0]])   # shape [2, 2]
    let z = zeros([2, 3])                       # shape [2, 3], all 0.0
    let o = ones([3])                           # shape [3], all 1.0

- `Tensor(...)` accepts a List of Float, or nested Lists of Float (rectangular).
- Integer elements are a type error. Strict typing stays: Int and Float are never mixed implicitly.
- Ragged input ([[1.0], [2.0, 3.0]]) is a runtime error.
- Constructors are built-in functions for now (like len, str). Moving them into a
  `tensor` standard-library module is a later change.

## 4. Operations by milestone
T1: type, construction, print, `shape(a)` (returns List of Int), `len(a)` (first dimension)
T2: elementwise + - * / between tensors of identical shape; tensor with Float scalar
T3: `a.matmul(b)` (2D x 2D), `a.transpose()`, `a.sum()`
T4: indexing `a[i]` and `a[i, j]` (bounds checked), general broadcasting
T5: reproducible benchmark and correctness tests against known results
Later: automatic differentiation, neural network layers, GPU abstraction.

## 5. Rules
- matmul: [m, k] x [k, n] -> [m, n]. Any other shapes are a runtime error naming both shapes.
- Elementwise ops: shapes must match exactly until T4 (broadcasting).
- Division follows IEEE 754 for floats (inf / NaN are allowed), unlike Int division by zero.
- Printing shows shape and values.
- Method-call syntax (`a.matmul(b)`) needs a general method mechanism. Today only
  `list.push` exists as a special case. T3 adds a small built-in method table.

## 6. Checker and runtime changes (planned)
- types: add `Type::Tensor`, name "Tensor".
- values: add `Value::Tensor { shape, data }`.
- checker: type rules for constructors and operators.
- IR/VM and interpreter: new instructions, results identical in both (tests required).

## 7. Rules for future changes
- Interpreter and VM must give identical results.
- Every operation needs tests with known expected values.
- Keep sections 2 and 5 true when adding features.

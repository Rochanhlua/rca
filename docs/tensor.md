# RCA Tensors

Status: IMPLEMENTED (CPU, Float only). Milestones T1-T6 are done.
Not implemented yet: automatic differentiation, method-call syntax, GPU, `a[i] = x`.

## 1. Scope
- One type: `Tensor`. Element type is Float (f64) only.
- CPU only. No GPU, no threads, no SIMD.
- No performance claims until reproducible benchmarks exist (none yet).

## 2. Representation
- Row-major flat storage: `data: Vec<f64>` plus `shape: Vec<usize>`.
- Invariant: product of shape == data.len(). Always checked.
- Rank is not part of the static type (Tensor, not Tensor[2]). Shape errors are runtime errors.
- Value semantics, like every RCA value (see memory-model.md): assignment copies.
- Size limit: MAX_TENSOR_ELEMENTS = 10,000,000. Shape products use checked
  multiplication; too large is a runtime error, never a crash.

## 3. Construction
    let a = Tensor([[1.0, 2.0], [3.0, 4.0]])   # shape [2, 2]
    let z = zeros([2, 3])                       # shape [2, 3], all 0.0
    let o = ones([3])                           # shape [3], all 1.0

- `Tensor(...)` accepts a List of Float, or nested Lists of Float.
- Integer elements are a compile-time type error (Int and Float are never mixed).
- Ragged input ([[1.0], [2.0, 3.0]]) is a runtime error.
- Shapes must be a non-empty List of non-negative Int.

## 4. Operations (all implemented, VM and tree interpreter give identical results)
| Operation | Result | Notes |
|---|---|---|
| `shape(a)` | List of Int | |
| `len(a)` | Int | first dimension |
| `a[i]` | Float (rank 1) or sub-Tensor (rank 2+) | bounds checked; `a[1][2]` works |
| `a + b`, `a - b`, `a * b`, `a / b` | Tensor | elementwise, with broadcasting |
| `a op x`, `x op a` (x is Float) | Tensor | scalar applied to every element |
| `matmul(a, b)` | Tensor | rank 2 x rank 2 only |
| `sum(a)`, `mean(a)`, `max(a)`, `min(a)` | Float | over all elements |
| `transpose(a)` | Tensor | rank 1 unchanged, rank 2 swaps axes |
| `reshape(a, [..])` | Tensor | element count must stay equal |

`Tensor op Int` is a type error. `sum`, `mean`, `max`, `min`, `transpose`,
`reshape`, `matmul`, `zeros`, `ones`, `shape` are built-in function names.

## 5. Rules
- Broadcasting (NumPy rule): shapes are right-aligned; each pair of dimensions
  must be equal or one of them must be 1. Otherwise: runtime error
  `cannot broadcast shapes [..] and [..]`.
- matmul: [m, k] x [k, n] -> [m, n]. Other shapes are a runtime error naming both shapes.
- Division follows IEEE 754 for floats (inf / NaN allowed), unlike Int division by zero.
- mean, max, min of an empty tensor is a runtime error.
- Printing shows shape and values: `Tensor(shape=[2], [1.0, 2.0])`.

## 6. Implementation notes
- All tensor logic lives in `src/runtime/values/mod.rs` (tensor_arith, tensor_matmul,
  tensor_index, tensor_reduce, tensor_transpose, tensor_reshape, ...).
- VM and interpreter both call these functions, so results cannot drift apart.
- Elementwise ops reuse `apply_binary`; no new IR instruction was needed for them.
- IR instructions: MakeTensor, FillTensor, TensorShape, MatMul, TensorSum,
  TensorMean, TensorMax, TensorMin, TensorTranspose, TensorReshape.

## 7. Beyond this document
Implemented since this document was first written:
- Automatic differentiation: `track` and `grad` (see `autodiff-design.md`).
- Neural network builtins: `linear`, `softmax`, `conv2d`, `mse`, `sgd`
  (see `nn.md`).
- Large `matmul` calls run in parallel and in blocks (set `RCA_NO_PARALLEL=1`
  to turn the parallel path off).

## 7a. Not implemented yet (planned)
- Method-call syntax (`a.matmul(b)`, `a.sum()`); today only functions exist.
- `a[i, j]` multi-index and `a[i] = x` assignment.
- matmul for rank 1 and rank 3+; axis-wise reductions; `Tensor` of Int.
- GPU abstraction, benchmarks against NumPy/PyTorch.

## 8. Rules for future changes
- Interpreter and VM must give identical results.
- Every operation needs tests with known expected values.
- Keep sections 2 and 5 true when adding features.

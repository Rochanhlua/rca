# Automatic differentiation: design proposal

Status: PROPOSAL (nothing in this document is implemented yet).
Scope: Phase 5, CPU only, reverse-mode first.

## 1. Goal

Make this program work, in the VM and in the tree interpreter, with identical results:

    let x = Tensor([2.0])
    let y = x * x
    let g = grad(y, x)
    print(g)

Expected output: `Tensor(shape=[1], [4.0])`.

The final spelling is `ai.grad(y, x)` (see ROADMAP section 6.2). How stdlib modules
are exposed is an open question (section 8), so milestone 1 uses a builtin `grad`.

## 2. Current state (facts)

- `Value::Tensor { shape, data }` where `data` is `Arc<Vec<f64>>`.
- All tensor operations are plain functions in `runtime/values/mod.rs`
  (`tensor_arith`, `tensor_matmul`, `tensor_reduce`, `tensor_transpose`,
  `tensor_reshape`, `tensor_map`). The VM and the tree interpreter both call them.
- Sum / mean / max / min return `Float`, not a Tensor.

Consequence: autodiff must live in the runtime values layer, not in either engine,
so both engines get it for free and cannot diverge.

## 3. Options considered

A. Add a graph field to `Value::Tensor`.
   Touches every construction and pattern site of the variant. Large, risky diff.

B. A global tape (thread-local list of operations).
   Simple, but hidden global state, hard to test, and wrong with several graphs.

C. A separate variant `Value::Tracked(Arc<Tracked>)` (RECOMMENDED).
   A tracked tensor holds its value (a normal tensor) plus a graph node.
   Existing code that handles plain tensors is untouched. Tensor operations check:
   if any operand is `Tracked`, take the autodiff path, otherwise the existing path.

## 4. Design (option C)

    struct Tracked {
        value: Tensor data + shape,   // the forward result
        node: Arc<Node>,
    }

    struct Node {
        id: u64,                      // monotonic creation order
        parents: Vec<Arc<Node>>,      // inputs of the operation
        op: Op,                       // Add, Sub, Mul, Div, MatMul, Sum, Mean,
                                      // Transpose, Reshape, Exp, Log, Sqrt,
                                      // Tanh, Relu, Sigmoid, ...
        // saved forward values needed by the backward rule
    }

- `track(x)` (name open) marks a plain tensor as a differentiable leaf.
- Every operation on a tracked operand produces a new tracked value.
- The graph is a DAG whose edges point to parents only, so `Arc` cannot form
  cycles and nothing leaks.
- `grad(y, x)` runs the backward pass:
  1. `y` must have exactly one element (v1). Seed its gradient with 1.0.
  2. Visit nodes from `y` down in decreasing `id` (creation order is already a
     valid reverse topological order).
  3. Accumulate gradients per node id (`+=`), which gives correct results when a
     value is used more than once.
  4. Return the gradient of leaf `x` as a plain Tensor with `x`'s shape.

## 5. Backward rules (v1)

- Add / Sub: pass the gradient through (negate for the right side of Sub).
- Mul: `da = g * b`, `db = g * a`.
- Div: `da = g / b`, `db = -g * a / (b * b)`.
- MatMul: `dA = G x B^T`, `dB = A^T x G`.
- Sum: broadcast `g` to the input shape. Mean: same, divided by the element count.
- Transpose / Reshape: apply the inverse shape change.
- Exp: `g * exp(x)`. Log: `g / x`. Sqrt: `g / (2 * sqrt(x))`.
- Tanh: `g * (1 - tanh(x)^2)`. Sigmoid: `g * s * (1 - s)`. Relu: `g` where `x > 0`, else 0.
- Broadcasting: when an operand was broadcast in the forward pass, its gradient
  is summed over the broadcast dimensions.

## 6. Correctness and testing

- Every rule is tested against a numerical gradient (central finite differences).
- VM and tree interpreter must print identical results (`same(...)` tests).
- Errors (non-scalar `y`, `x` not on the path to `y`, untracked `x`) are runtime
  errors with clear messages, identical in both engines.
- Non-finite gradients are runtime errors, consistent with `tensor_map`.

## 7. Milestones

1. `Tracked` value, Add and Mul, `grad` on a one-element tensor.
2. Sub, Div, and plain tensors / Float scalars mixed in as constants (implemented).
   Elementwise math, Sum, Mean (next).
3. Broadcasting backward, MatMul, Transpose, Reshape.
4. Gradient accumulation across several `grad` calls, `zero_grad`.
5. Forward-mode differentiation (separate design).

Each milestone is one pull request with tests.

## 8. Open questions

- Spelling: builtin `grad` / `track`, or `ai.grad` through a stdlib module? The
  module system currently resolves user files only.
- Should `track` be explicit, or should `Tensor(...)` take a `requires_grad` flag?
- Type system: add a `Type::Tracked`, or treat tracked values as `Tensor`?
- `max` / `min` gradients (ties) and `pow` are deferred until the basics work.

## 9. Non-goals (for now)

GPU, higher-order derivatives, graph optimization, in-place mutation of tracked
tensors, multi-threaded graphs.

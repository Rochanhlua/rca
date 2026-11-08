# Design: Shared Tensor Storage

Status: DECIDED, NOT IMPLEMENTED. Nothing in this document is in the code yet.

## 1. Problem
BENCHMARKS.md shows that reading a variable clones the whole value
(VM LoadGlobal / LoadLocal, interpreter variable read). For a Tensor this
copies the full `Vec<f64>`. In copy_large.rca (1,000,000 elements, 1000
reads) the run took about 8 seconds on the test phone.

## 2. Observation
Tensors cannot be modified today: `a[i] = x` is not implemented
(tensor.md, section 7), and every operation builds a new tensor. Two
variables that share the same storage therefore cannot see each other
change. Sharing would be invisible to RCA programs.

## 3. Decision
Change `Value::Tensor` so that `data` is `Arc<Vec<f64>>` instead of
`Vec<f64>`. `shape` stays `Vec<usize>`.
- Cloning a Tensor value copies a pointer and bumps a counter.
- Operations that produce a new tensor still allocate a new Vec.
- `Value` derives Debug, Clone and PartialEq. Arc<Vec<f64>> supports all
  three, so the derive line does not change.

## 4. Why Arc and not Rc
- The blueprint plans parallel tensor operations (section 6.5). Rc is not
  thread-safe and would make Value non-Send. Switching later would touch
  every site a second time.
- Arc is the standard thread-safe choice in Rust.
- Cost: atomic reference counting may be slower than Rc. This has NOT been
  measured. It must be measured after implementation (section 8).

## 5. Why not move semantics first
Move semantics needs last-use analysis in the compiler and is a larger
change. Shared storage is smaller and keeps memory-model.md sections 1-3
true. Move semantics stays in the Planned list.

## 6. Known impact
- `Value::Tensor {` appears 37 times in runtime/values/mod.rs, in both
  production code and test modules. The exact production count is not yet
  established. Many are patterns and may need no change.
- VM: Tensor patterns at runtime/vm/mod.rs lines 290 and 473.
- Interpreter: Tensor patterns at runtime/execution/mod.rs lines 426 and 655.
- Unknown: every place that builds Value::Tensor directly. The compiler
  will find them.

## 7. Risks
- If `a[i] = x` is added later, writes must copy first when the storage
  is shared (Arc::make_mut), so that value semantics still hold.
- List and Str values still copy on read. This change does not fix that.

## 8. Acceptance criteria
- All existing tests pass unchanged; VM and interpreter results stay identical.
- New tests show that using a shared tensor never changes another variable.
- benchmarks/copy_large.rca is re-run with the same method (4 runs) and
  before/after times are recorded in BENCHMARKS.md.
- The cost of Arc against a plain Vec is checked with a tensor-heavy
  benchmark (many small tensor operations), not only the copy benchmark.
- No performance claim beyond what is measured.

## 9. Out of scope
Move semantics, borrowing, sharing for List and Str.

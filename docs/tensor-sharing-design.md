# Design: Shared Tensor Storage

Status: PROPOSED. Nothing in this document is implemented.

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

## 3. Proposal
Store tensor data behind a reference-counted pointer instead of a plain
`Vec<f64>`. Cloning a Tensor value then copies a pointer and increments a
counter. Operations that produce a new tensor still allocate a new Vec.

## 4. Why not move semantics first
Move semantics needs last-use analysis in the compiler and is a larger
change. Shared storage is smaller and keeps memory-model.md sections 1-3
true. Move semantics stays in the Planned list.

## 5. Known impact
- VM: Tensor patterns at runtime/vm/mod.rs lines 290 and 473.
- Interpreter: Tensor patterns at runtime/execution/mod.rs lines 426 and 655.
- runtime/values/mod.rs: NOT yet inspected. Must be reviewed before any
  implementation.
- Unknown: whether any code requires Value to be Send, and every place
  that builds Value::Tensor directly.

## 6. Open question: Rc or Arc
- Rc: cheaper, but not thread-safe.
- Arc: thread-safe, needed if parallel tensor operations are added
  (blueprint section 6.5). Its extra cost has not been measured.
To be decided before implementation.

## 7. Risks
- If `a[i] = x` is added later, writes must copy first when the storage
  is shared, so that value semantics still hold.
- List and Str values still copy on read. This change does not fix that.

## 8. Acceptance criteria
- All existing tests pass unchanged; VM and interpreter results stay identical.
- New tests show that using a shared tensor never changes another variable.
- benchmarks/copy_large.rca is re-run with the same method (4 runs) and
  before/after times are recorded in BENCHMARKS.md.
- No performance claim beyond what is measured.

## 9. Out of scope
Move semantics, borrowing, sharing for List and Str.

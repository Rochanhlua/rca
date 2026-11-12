# RCA Memory Model (v0.5)

Status: Implemented = in the compiler/runtime today. Planned = not yet.

## 1. Value semantics (Implemented)
- Every value (Int, Float, Bool, Str, None, List, Tuple, Range, Tensor,
  Enum, Struct) is owned by exactly one variable.
- Assignment and function arguments COPY the value. Two variables never share memory.
- Reading a variable or a list element also copies the whole value,
  including tensor data. This is correct but not yet optimized, and its
  cost has not been measured.
- Result: no use-after-free, no double-free, no dangling references.

## 2. Runtime safety checks (Implemented)
Both the interpreter and the VM report these as runtime errors
(catchable with try/catch):
- List index out of bounds (read and write)
- Integer overflow (checked arithmetic)
- Division / modulo by zero
- Call depth exceeded (stack overflow protection)

## 3. No unsafe code (Implemented)
RCA programs have no raw pointers and no unsafe blocks.

## 3a. Native codegen memory (Implemented, leaks known)

The native (Cranelift) backend allocates on the heap but does not yet
free anything. This is a known gap between the VM and native paths.

Allocation sites in `src/compiler/codegen/mod.rs` (all `malloc`, no `free`):
- `MakeList` — `(n+1)*8` bytes for the length header and elements.
- `MakeTuple` — `(n+1)*8` bytes, same layout.
- `MakeStruct` — `n*8` bytes for the fields.
- String `+` — `strlen(a) + strlen(b) + 1` bytes.
- `ListPush` — allocates a new buffer and copies the old one; the
  previous buffer is not freed.

Consequences:
- Long-running native programs leak memory proportional to allocation
  count. Short scripts are unaffected in practice.
- Values passed to functions are copied, so the VM's value semantics
  still hold observably; the leak is invisible in the output.

Mitigation plan (Phase 4):
- Move semantics: consume the source value on the last use so `push`
  and string concatenation can reuse buffers instead of copying.
- A shared runtime header (length + refcount) so `free` can be called
  when the last reference dies, or a small region/arena strategy per
  function for temporaries.
- Leak regression tests once ownership lands.

## 4. Planned (NOT implemented)
- Move semantics: avoid copying when the old variable is never used again.
- Avoid copying on read (move semantics or shared references), once
  benchmarks show it matters.
- Borrowing: ref / mut ref with a borrow checker.
- Resource cleanup (files, handles) tied to scope.
- Memory limits for large values (tensors).

## 5. Rules for future changes
- Any new feature must keep sections 1-3 true.
- Interpreter and VM must give identical results (tests required).

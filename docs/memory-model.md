# RCA Memory Model

Status: Implemented = in the compiler/runtime today. Planned = not yet.

## 1. Value semantics (Implemented)
- Every value (Int, Float, Bool, Str, None, List, Tuple, Range, Tensor,
  Enum, Struct) is owned by exactly one variable.
- Assignment and function arguments COPY the value. Two variables never share
  memory that a program can observe.
- Reading a variable or a list element also copies the value. Lists, tuples,
  maps and strings are copied in full.
- Tensor data is the exception: `Value::Tensor` keeps its numbers in an
  `Arc<Vec<f64>>`, so copying a tensor value copies a pointer and bumps a
  counter. Tensors cannot be modified in place (`a[i] = x` is not
  implemented), so the sharing is invisible to programs. See
  `tensor-sharing-design.md`. The only copy-on-read measurement is `BENCHMARKS.md`
  (section 1, 2026-11-08); it has not been repeated since.
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

## 3a. Native codegen memory (Implemented in part, leaks remain)

The native (Cranelift) backend allocates heap objects with `malloc` and is
being moved to reference counting. This is still a known gap between the VM
and the native path.

What exists today (see `src/compiler/codegen/mod.rs` and `CHANGELOG.md`):
- Lists, tuples, structs and maps have a refcount word at offset 0 and, where
  applicable, the length at offset +8.
- Heap values are incref'd on `LoadLocal` and `LoadGlobal` and decref'd by
  `StoreLocal`, `StoreGlobal` and `Pop`. Locals start zeroed, so the first
  decref is a no-op. Aliasing (`let y = x`) and rebinding (`x = [2]`) are safe.
- `Print` decrefs `List`, `Tuple` and `Map` arguments after printing.
- A `Return` from a function other than `main` decrefs every heap local.
- `free` is called when a count reaches zero. `ListPush` frees the buffer it
  replaces.

What still leaks or is not finished:
- String concatenation results are never freed. Static string literals live in
  `.rodata`, so they cannot be decref'd, and there is no tag yet that tells
  heap strings from static ones.
- Ownership of arguments and return values across function calls is not
  wired up yet.
- There are no leak regression tests.

Consequences:
- Long-running native programs can still leak memory. Short scripts are
  unaffected in practice.
- Values passed to functions are copied, so the VM's value semantics still
  hold observably; the leaks are invisible in the output.

## 4. Planned (NOT implemented)
- A heap-versus-static tag for strings so concatenation results can be freed.
- Ownership across function boundaries in native code, then leak regression
  tests.
- Move semantics: avoid copying when the old variable is never used again.
- Avoid copying lists, tuples and strings on read (move semantics or shared
  references), once benchmarks show it matters.
- Borrowing: ref / mut ref with a borrow checker.
- Resource cleanup (files, handles) tied to scope.
- Memory limits for large values (tensors).

## 5. Rules for future changes
- Any new feature must keep sections 1-3 true.
- Interpreter and VM must give identical results (tests required).

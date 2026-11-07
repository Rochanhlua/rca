# RCA Memory Model (v0.4)

Status: Implemented = in the compiler/runtime today. Planned = not yet.

## 1. Value semantics (Implemented)
- Every value (Int, Float, Bool, Str, List, Struct) is owned by exactly one variable.
- Assignment and function arguments COPY the value. Two variables never share memory.
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

## 4. Planned (NOT implemented)
- Move semantics: avoid copying when the old variable is never used again.
- Borrowing: ref / mut ref with a borrow checker.
- Resource cleanup (files, handles) tied to scope.
- Memory limits for large values (tensors).

## 5. Rules for future changes
- Any new feature must keep sections 1-3 true.
- Interpreter and VM must give identical results (tests required).

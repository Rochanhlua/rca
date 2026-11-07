# RCA Syntax Specification v0.1

- Indentation (4 spaces) defines blocks
- Comments begin with #
- Variables: let (immutable), mut (mutable)
- Types: Int, Float, Bool, String, Char
- Functions: fn name(a: Int) -> Int:
- Keywords: let mut fn return if else elif for while in break
  continue struct enum type import from as pub private try catch
  throw and or not const true false none match

## Implemented in v0.1 (Phase 1)
- let / mut declarations with optional type annotation
- Assignment, expression statements
- fn with typed params, default values, return type
- struct declarations, named constructor arguments
- if / elif / else, while, for ... in a..b
- return, break, continue
- Operators: + - * / %  == != < <= > >=  and or not  ..
- Lists [1, 2, 3], indexing, field access, calls

## Planned (not yet parsed)
- import / from / as, enum, match, try / catch / throw
- const, pub / private, type aliases, generics

## Type checking (v0.2, Phase 2)
- Strict static typing: Int and Float cannot be mixed in arithmetic
- Type inference for let / mut without annotation
- Functions: parameter types, return types, default values, named arguments,
  forward calls, recursion, "must return on all paths" check
- Structs: constructors, field access, field assignment (requires mut)
- Scopes: block scopes, shadowing in inner scopes, no redeclaration in same scope
- Built-in: print(...)

## Deferred
- Modules (import / from / as): after execution engine and stdlib exist
- enum, match, try / catch / throw, const, pub / private, generics
- List type annotations (e.g. List[Int]); lists are inferred from literals only

## Execution (v0.3, Phase 3)
- `rca run file.rca` type-checks, then runs the program (tree-walking interpreter)
- Ranges are end-exclusive: `0..3` yields 0, 1, 2
- Values are copied on assignment (value semantics)
- Runtime errors: division by zero, index out of bounds, integer overflow, stack overflow

## Current status (v0.5) - supersedes "Planned" and "Deferred" above
Implemented since v0.1:
- try / catch / throw (throw takes a String; break/continue inside try is handled)
- import, import as, from ... import [as], pub fn, pub struct, module-qualified calls (utils.add(1, 2))
- Built-ins: print, len, str, int, float, list push
- Runtime checks: list bounds, integer overflow, division by zero, call depth

Still not implemented (per the lists above):
- enum, match, const, private, type aliases, generics
- List type annotations (lists are inferred from literals only)
- pub on let / const

## Current status (v0.9) - tensors
Implemented since v0.5 (details in tensor.md):
- Type `Tensor` (Float only)
- Built-ins: Tensor, zeros, ones, shape, matmul, sum, mean, max, min, transpose, reshape
- Tensor indexing `a[i]`, and `+ - * /` with broadcasting and Float scalars

Still not implemented:
- Method syntax on tensors (`a.matmul(b)`), `a[i, j]`, `a[i] = x`
- Automatic differentiation

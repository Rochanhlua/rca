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

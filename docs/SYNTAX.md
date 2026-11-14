# RCA Syntax Specification

Status: describes what the compiler accepts today (Nov 2026). Anything not
listed under "Implemented" is not available yet; see "Not implemented".

## Basics
- Indentation (4 spaces) defines blocks
- Comments begin with #
- Variables: `let` (immutable), `mut` (mutable)
- Types: Int, Float, Bool, String, Tensor, tuple types such as `(Int, String)`.
  `Char` is a reserved type name, but there are no character literals and no
  Char values yet.
- Functions: `fn name(a: Int) -> Int:`
- Keywords: let mut fn return if else elif for while in break
  continue struct enum type import from as pub private try catch
  throw and or not const true false none match

## Implemented

### Declarations and statements
- `let` / `mut` with an optional type annotation; types are inferred otherwise
- Tuple destructuring: `let (a, b) = p` and `mut (a, b) = p`
- `const NAME: Type = value` or `const NAME = value` (see Constants)
- Assignment, expression statements
- `if` / `elif` / `else`, `while`, `for ... in a..b`, `break`, `continue`, `return`

### Expressions
- Operators: `+ - * / %  == != < <= > >=  and or not  ..`
- Lists `[1, 2, 3]`, indexing `xs[i]`, index assignment `xs[i] = v`,
  `xs.push(v)`, field access, calls
- Tuples with 2 or more elements: `(1, "a")`, literal indexing `p[0]`,
  `len(p)`, `==` / `!=`. Tuples are immutable (see `tuple.md`).
- Tensors (Float only): `Tensor([[1.0, 2.0], [3.0, 4.0]])`, `a[i]`,
  `+ - * /` with broadcasting and Float scalars (see `tensor.md`)
- Ranges are end-exclusive: `0..3` yields 0, 1, 2

### Functions and structs
- `fn` with typed parameters, default values, named arguments, return type
- Forward calls, recursion, "must return on all paths" check
- `struct` declarations, named constructor arguments, field access,
  field assignment (requires `mut`)

### Enums and match (see `enum.md`)
- `enum Name:` with simple variants, `pub enum`, `Name.Variant`
- `match` statement over enum variants, Int/String/Bool literals and `_`
- Exhaustiveness, duplicate-arm and unreachable-arm checks

### Constants
- `const NAME: Type = value` or `const NAME = value`
- Top-level only; the value must be a literal or other constants
  (operators allowed)
- Assigning to a constant is an error: `cannot assign to constant 'NAME'`
- Constants can be read inside functions

### Errors
- `try` / `catch` / `throw` (`throw` takes a String; `break` and `continue`
  inside `try` are handled)
- Runtime errors are catchable: list bounds, integer overflow, division by
  zero, call depth

### Modules (see `modules.md`)
- `import`, `import as`, `from ... import [as]`, `pub fn`, `pub struct`,
  module-qualified calls: `utils.add(1, 2)`
- Standard library modules (14): `math`, `fs`, `time`, `string`, `json`,
  `tensor`, `matrix`, `ai`, `ml`, `nn`, `net`, `http`, `crypto`, `collections`

### Built-ins
- Core: `print`, `len`, `str`, `int`, `float`
- Math: `abs`, `pow`, `floor`, `ceil`, `round`
- String: `trim`, `split`, `replace`
- Files and time: `read_file`, `write_file`, `file_exists`, `now`, `sleep`
- Map: `map_new`, `map_set`, `map_get`, `map_has`, `map_len`
- JSON: `json_parse`, `json_stringify`
- Network: `tcp_connect`, `tcp_send`, `tcp_recv`, `tcp_close`,
  `udp_bind`, `udp_send`, `udp_recv`, `udp_close`, `http_get`, `http_post`
- Crypto: `sha256`, `sha512`, `hmac_sha256` (see `crypto.md`)
- Tensors and autodiff: `Tensor`, `zeros`, `ones`, `shape`, `matmul`, `sum`,
  `mean`, `max`, `min`, `transpose`, `reshape`, `track`, `grad`
  (see `tensor.md` and `autodiff-design.md`)
- Neural network: `linear`, `relu`, `sigmoid`, `tanh`, `softmax`, `conv2d`,
  `mse`, `sgd` (see `nn.md`)

## Execution
- `rca file.rca` type-checks, then runs the program on the bytecode VM
  (it calls `rcac run`)
- `rcac run-tree file.rca` uses the tree-walking interpreter instead.
  It does not support `import` yet.
- `rcac compile file.rca` writes a native object file (Cranelift);
  `rcac build file.rca` also links an executable (needs `clang`).
  Native code generation covers a subset of the language, see `README.md`
  and `memory-model.md`.
- `rcac pkg init|add|install` manages dependencies via `rca.json` and
  `$HOME/.rca/packages/` (see `packages.md`). `rcac pkg` is a tool
  command, not part of the language grammar.
- Values are copied on assignment (value semantics)
- The VM and the tree-walking interpreter must give identical results

## Not implemented
- Character literals and Char values
- Generics, type aliases, `private` as a modifier (the word is reserved, the
  parser does not use it; declarations are private by default)
- List and Map type annotations such as `List[Int]` (lists are inferred from
  literals; maps are used through the `map_*` functions)
- Enum payloads, match expressions, guards, nested patterns
- Empty and 1-element tuples; nested destructuring patterns
- `pub let` and `pub const`, importing constants from other modules
- Tensor method syntax (`a.matmul(b)`), `a[i, j]`, `a[i] = x`
- Autodiff for `max` and `min`
- Borrowing and references

# RCA Programming Language

Built for the Intelligence Era.

RCA is an independent, statically typed programming language with an
indentation-based syntax. The compiler is written in Rust. It has built-in
tensors and automatic differentiation, and it can compile to native code.
The long-term goal is an AI-native language with a native tensor engine.
See `docs/` for design documents.

**Status: v0.1.0 public beta (early development; not ready for production use).**

Creator: TH Lalrochanhlua

## Build

    cargo build --release
    cargo build
    cargo test

This produces two binaries:

- `rcac` is the compiler and tool driver.
- `rca` is the executable front end. `rca <file.rca>` runs a program
  (it delegates to `rcac run`).

## Usage

    rca  <file.rca>           run a program

    rcac lex <file.rca>       show tokens
    rcac parse <file.rca>     show the syntax tree
    rcac check <file.rca>     type check only
    rcac ir <file.rca>        show bytecode
    rcac ir-opt <file.rca>    show optimized bytecode
    rcac run <file.rca>       run on the bytecode VM
    rcac run-tree <file.rca>  run on the tree-walking interpreter
    rcac compile <file.rca>   compile to a native object file (<file>.rca.o)
    rcac build <file.rca>     compile and link a native executable (needs clang)

Developer tools:

    rcac lint <file.rca>      report warnings (unused vars, unreachable code)
    rcac test [dir]           run all .rca test files (default: tests/rca/)
    rcac fmt <file.rca>       print canonical formatting
    rcac repl                 interactive read-eval-print loop
    rcac lsp                  Language Server Protocol (JSON-RPC over stdio)
    rcac doc <file.rca>       print Markdown API documentation
    rcac bench <file.rca> [N] benchmark a file N times (default N=5)
    rcac watch <file.rca>     re-run a file whenever it changes
    rcac profile <file.rca>   run with instrumentation and print report
    rcac coverage <file.rca>  report executed vs missed lines
    rcac debugger <file.rca>  interactive step-through debugger
    rcac completion <shell>   print shell completion (bash, zsh, fish)

Package manager:

    rcac pkg init             create rca.json in the current directory
    rcac pkg add <name> <src> add or update a dependency (local, http://, https://)
    rcac pkg install          fetch dependencies to $HOME/.rca/packages/

See `docs/packages.md`.

## Example

    fn add(a: Int, b: Int) -> Int:
        return a + b

    let result = add(10, 20)
    print(result)

Tensor example:

    let a = Tensor([[1.0, 2.0], [3.0, 4.0]])
    let b = Tensor([[5.0, 6.0], [7.0, 8.0]])
    print(matmul(a, b))
    print(a + Tensor([10.0, 20.0]))

Training example (see `examples/mlp_train.rca` and `docs/nn.md`):
`track`, `linear`, `mse`, `grad` and `sgd` compose into a training loop.

More programs are in `examples/`. Files named `*_bad` and a few others demonstrate compile or runtime errors and are expected to fail.

## What works today

Language:

- Lexer, parser, diagnostics with line and column
- Static type checker with type inference
- Variables (`let`, `mut`), `const` and `pub const` (top-level, constant values only)
- Types: `Int`, `Float`, `Bool`, `String`, `Char`, fixed-size `Array[T, N]`
- Visibility: `pub` and `private` on functions, types, `let` and `const`
- Functions, generic functions (`fn name[T](...)`), structs, lists (`List[T]`), tuples (with destructuring), maps (`Map[K, V]`)
- `if`/`else`, `while`, `for`, `break`, `continue`
- `try` / `catch` / `throw`
- `enum` and `match` statement (see `docs/enum.md`)
- Modules: `import`, `import as`, `from`-import, `pub`, module-qualified
  calls (see `docs/modules.md`)
- Runtime checks: list bounds, integer overflow, division by zero, call depth

Execution:

- Bytecode compiler, optimizer, validator and VM
- Tree-walking interpreter
- Native code generation (Cranelift) via HIR for most of the language: arithmetic,
  control flow, loops, functions, floats, strings, structs, lists, tuples,
  enums and match, maps, math builtins, try/catch/throw, Tensor ops (`matmul`, `mse`, `softmax`, `sgd`, `reshape`, `shape`),
  reverse-mode autodiff (`track`, `grad`), and stdlib (`fs`, `time`, `string`, `crypto`, `net`, `http`).
  Heap objects carry a refcount header. Some native allocation gaps remain; see `docs/memory-model.md`.

Tensors and AI (CPU, Float only):

- Tensors: construction, indexing (`a[i]` and comma indexing `a[i, j]`), method syntax (`a.matmul(b)`), elementwise `+ - * /`, broadcasting,
  matmul (rank 2, parallel and blocked for large sizes), sum/mean/max/min,
  transpose, reshape. See `docs/tensor.md`.
- Automatic differentiation (reverse-mode): `track(t)` and `grad(y, x)`.
  Tracked tensors support `+ - * /`, `sum`, `mean`, rank 2 `matmul`,
  `transpose`, `reshape`, `exp`, `log`, `sqrt`, `tanh`, `relu` and `sigmoid`,
  also mixed with plain tensors or Floats (treated as constants). `sum` and
  `mean` of a tracked tensor give a one-element Tensor, not a Float.
  See `docs/autodiff-design.md`.
- Neural network builtins: `linear`, `softmax`, `conv2d` (valid padding,
  stride 1), `mse`, `sgd`. Multi-layer models are built by composition,
  and forward-only inference works with a plain `predict` function.
  See `docs/nn.md`.

Standard library (via `import <module>`; 14 modules):

`math`, `fs`, `time`, `string`, `json`, `tensor`, `matrix`, `ai`, `ml`, `nn`,
`net` (TCP and UDP), `http` (`get`/`post`, HTTPS via rustls), `crypto` (`sha256`, `sha512`, `hmac_sha256`),
`collections`. The same functions also work as plain builtins.

## Not implemented yet

- Borrowing and references (`&T`, `&mut T`)
- Generic structs/enums, enum payloads, match expressions
- Full native code generation coverage for remaining stdlib helpers and native refcount drop cleanup
- Autodiff for `max` and `min`; GPU support

## Documentation

- `docs/SYNTAX.md` syntax
- `docs/cli.md` command line
- `docs/ROADMAP.md` full project roadmap and phases
- `docs/crypto.md` hashing and HMAC
- `docs/net.md` TCP, UDP, HTTP
- `docs/packages.md` package manager
- `docs/memory-model.md` memory model
- `docs/modules.md` modules
- `docs/tensor.md` tensors
- `docs/autodiff-design.md` automatic differentiation
- `docs/nn.md` neural network builtins
- `docs/enum.md` enum and match
- `docs/tuple.md` tuples
- `docs/BENCHMARKS.md` benchmarks (no general performance claims are made)

## Development

Developed on Android (Termux) with Rust and Cargo.

## License

MIT. See `LICENSE`.

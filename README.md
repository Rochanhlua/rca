# RCA Programming Language

Built for the Intelligence Era.

RCA is an independent, statically typed programming language with a
Python-inspired, indentation-based syntax. The compiler (`rca`) is written
in Rust. The long-term goal is an AI-native language with a native tensor
engine. See `docs/` for design documents.

**Status: early development. Not ready for production use.**

Creator: TH Lalrochanhlua

## Build

    cargo build --release
    cargo test

## Usage

    rca lex <file.rca>        show tokens
    rca parse <file.rca>      show the syntax tree
    rca check <file.rca>      type check only
    rca ir <file.rca>         show bytecode
    rca ir-opt <file.rca>     show optimized bytecode
    rca run <file.rca>        run on the bytecode VM
    rca run-tree <file.rca>   run on the tree-walking interpreter

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

More programs are in `examples/`.

## What works today

- Lexer, parser, diagnostics with line and column
- Static type checker with type inference
- Variables (let, mut), functions, structs, lists
- if/else, while, for, break, continue
- try / catch / throw
- Built-ins: print, len, str, int, float, list push
- Modules: import, import as, from-import, pub, module-qualified calls
- Bytecode compiler, optimizer, validator and VM
- Runtime checks: list bounds, integer overflow, division by zero, call depth
- enum and match statement (see `docs/enum.md`)
- const declarations (top-level, constant values only)
- tuples: `(1, "a")`, literal indexing, `len`, `==`, destructuring `let (a, b) = p` (see `docs/tuple.md`)
- Tensors (CPU, Float only): construction, indexing, elementwise + - * /,
  broadcasting, matmul (rank 2), sum/mean/max/min, transpose, reshape.
  See `docs/tensor.md`.
- Automatic differentiation (reverse-mode, CPU): `track(t)` and `grad(y, x)`. Tracked tensors support `+ - * /`, `sum`, `mean`, rank 2 `matmul`, `transpose`, `reshape`, `exp`, `log`, `sqrt`, `tanh`, `relu` and `sigmoid`, also mixed with plain tensors or Floats (treated as constants). `sum` and `mean` of a tracked tensor give a one-element Tensor, not a Float. See `docs/autodiff-design.md`.

## Not implemented yet

- Native code generation
- Borrowing and references
- Map, generics, enum payloads, match expressions, pub const
- Autodiff for max and min; broadcasting in autodiff; tensor method syntax; GPU support
- Standard library modules
- Package manager
- Benchmarks (no performance claims are made)

## Documentation

- `docs/SYNTAX.md` syntax
- `docs/memory-model.md` memory model
- `docs/modules.md` modules
- `docs/tensor.md` tensors
- `docs/enum.md` enum and match
- `docs/tuple.md` tuples

## Development

Developed on Android (Termux) with Rust and Cargo.

## License

MIT. See `LICENSE`.

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

## Not implemented yet

- Native code generation
- Borrowing and references
- Tensors, automatic differentiation, GPU support
- Standard library modules
- Package manager

## Documentation

- `docs/SYNTAX.md` syntax
- `docs/memory-model.md` memory model
- `docs/modules.md` modules

## Development

Developed on Android (Termux) with Rust and Cargo.

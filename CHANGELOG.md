# Changelog

## Unreleased

- Lexer, parser, type checker, bytecode compiler, optimizer, validator, VM and tree interpreter.
- Modules, enum and match, const, tuples (with destructuring), CPU tensors.
- MIT license, CI (fmt, clippy, test).
- Reverse-mode autodiff (track, grad) for + and *.
- Autodiff: subtraction, division, and plain tensors or Floats mixed with tracked tensors.
- Autodiff: sum, mean and matmul (rank 2) for tracked tensors, with a linear regression example.
- Autodiff: transpose, reshape, exp, log, sqrt, tanh, relu and sigmoid for tracked tensors.
- Autodiff: broadcasting backward for + - * / (gradient summed over broadcast dimensions), finite-difference gradient checks.
- Integration tests: `tests/cli.rs` exercises every `rca` subcommand (`lex`, `parse`, `check`, `ir`, `ir-opt`, `run`, `run-tree`) end-to-end.
- Compiler regression tests: `tests/examples.rs` (check stage, 48 pass / 31 fail) and `tests/run_examples.rs` (run stage, 39 pass / 9 intentionally-fail-at-runtime) over all `examples/*.rca`.
- Snapshot tests: `tests/snapshots.rs` locks `rca lex`/`parse`/`ir`/`ir-opt` output; regenerate with `RCA_UPDATE_SNAPSHOTS=1 cargo test --test snapshots`.

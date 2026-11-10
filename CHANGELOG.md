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
- **Implemented:** file system builtins `read_file(path)`, `write_file(path, content)`, `file_exists(path)` — flat functions matching the existing `print`/`matmul`/`len` style. Both VM and tree-walking interpreter use `std::fs`.
- **Planned:** module-qualified `fs.read()`, `fs.write()`, `fs.exists()` via `import fs` (blueprint §8, §13). Requires a `src/stdlib/fs/` module system and module-method call syntax.
- **Implemented:** time builtins `now()` (Unix timestamp in ms) and `sleep(ms)` — flat functions matching the existing style. Both VM and tree-walking interpreter use `std::time`.
- **Planned:** module-qualified `time.now()`, `time.sleep()` via `import time` (blueprint §8, §13).
- **Implemented:** math builtins `abs()`, `pow()`, `floor()`, `ceil()`, `round()` — flat functions matching the existing style.
- **Planned:** module-qualified `math.abs()`, `math.pow()` etc. via `import math` (blueprint §8, §13).
- **Implemented:** string builtins `trim()`, `split()`, `replace()` — flat functions matching the existing style.
- **Planned:** module-qualified `string.trim()` etc. via `import string` (blueprint §8, §13).

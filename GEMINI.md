# STRICT RULES FOR AI AGENT (RCA PROJECT)
1. **100% PURE RUST ONLY:** Every component (compiler, VM, Cranelift codegen, tensor engine, autodiff, stdlib, tests) MUST be written natively in Rust (`src/*.rs`, `tests/*.rs`).
2. **ZERO EXTERNAL RUNTIMES:** Strictly forbidden to create `.py` files, FFI bindings to external interpreters, or wrapper scripts.
3. **BRANCH SAFETY:** Always work on the current feature branch (`ai-dev`). Never touch `master`.
4. **READ FIRST:** Before writing code, always read `docs/ROADMAP.md`, `CHANGELOG.md`, `README.md`, and `.github/workflows/`.
5. **INCREMENTAL WORK:** Implement ONLY ONE missing feature or sub-task at a time.
6. **VERIFY & TEST:** After every edit, run `cargo check` and `cargo test`. Never delete existing tests. Fix all compiler warnings and errors before proceeding.
7. **DOCS & CHANGELOG:** Once tests pass, update `CHANGELOG.md` (under `[Unreleased]`) and `docs/ROADMAP.md` / `README.md` to reflect the completed feature.

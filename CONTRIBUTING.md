# Contributing to RCA

RCA is in early development. Contributions are welcome, but the language and compiler change quickly, so please open an issue before starting large work.

## Build and test

    cargo build
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test

All three checks must pass before a pull request is merged. CI runs the same commands.

## Workflow

1. Fork the repository or create a branch from `master`.
2. Make one focused change per pull request.
3. Add tests with known expected output. The VM and the tree interpreter must give identical results.
4. Update `docs/` and `README.md` when a feature is added. Keep "implemented" and "planned" separate.
5. Open a pull request against `master`.

## Rules

- No new dependencies without discussion.
- No performance or security claims without evidence.
- Do not commit secrets, tokens or personal files.
- By contributing you agree your work is released under the MIT license.

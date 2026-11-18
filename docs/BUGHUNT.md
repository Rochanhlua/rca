# RCA Bug Hunt Log

Record of bug hunt rounds so the same checks are not repeated.

## Round 1 - 2026-11-18 (branch feat/roadmap-gap)

Method: ran every examples/*.rca with `rca <file>`; 119 files, 44 exited non-zero.
Command: for f in examples/*.rca; do ./target/release/rca "$f" >/dev/null 2>&1 || echo "FAIL: $f"; done

### Findings
- FIXED: `rca --help` / `rca --version` were forwarded to `rcac run` and read as file names (src/bin/rca.rs, commit c46f442).
- FIXED: examples/udp_loopback.rca sent to port 0; now uses fixed port 34567. Runtime was correct.
- EXPECTED: *_bad, *_mismatch, match_*, const_*, enum_clash, tuple_annot_*, tensor_*_bad, tensor_ragged fail on purpose with clear diagnostics.
- EXPECTED: examples/http_post_demo.rca fails with Connection refused when no server is listening.
- EXPECTED: examples/try_break2.rca tests that break inside try leaves no stale handler; final throw is uncaught on purpose.

### Not yet checked
- cargo test, cargo clippy
- unwrap()/expect() in src/runtime/vm and src/runtime/execution (34 occurrences)
- fuzz, memory safety, cross-platform (roadmap Phase 8)

# RCA Benchmarks

Status: first measurement only. No comparison with other languages yet.

## 1. Copy-on-read of tensors (2026-11-08)

Question: does reading a variable that holds a large tensor cost time
proportional to the tensor size? (See memory-model.md, section 1.)

Workload (benchmarks/copy_*.rca): create a tensor, then run
`let n = len(a)` 1000 times in a loop. `len` itself is cheap, so the time
is dominated by loading `a` (LoadGlobal), which clones the value in the VM.

The optimizer does not remove the load (checked with `rca ir-opt`:
27 -> 27 instructions, LoadGlobal and Len stay inside the loop).

### Setup
- Build: `cargo build --release` (default profile)
- Command: `time ./target/release/rca run benchmarks/copy_large.rca`
- rustc 1.99.1, aarch64, 8 cores, Android/Termux, phone model RMX3782
- Source: master at 4812cd3 plus the three benchmark files

### Results (real time)
| File | Tensor elements | Runs |
|---|---|---|
| copy_small.rca | 1,000 | 0.084s (1 run) |
| copy_medium.rca | 100,000 | 1.006s (1 run) |
| copy_large.rca | 1,000,000 | 7.495s, 8.980s, 8.187s, 8.035s |

For copy_large, `sys` time was about 75% of the total (5.5s to 6.9s) and
`user` time was about 1.1s to 1.3s.

### What this does and does not show
- Time grows with tensor size, which fits the whole tensor being copied on each read.
- High `sys` time suggests memory allocation is a large part of the cost.
  This is a hypothesis, not measured.
- This is a synthetic worst case. It says nothing about real AI workloads.
- copy_small and copy_medium were run once. copy_large varies by about 20%
  between runs on this phone.
- No comparison with Python, PyTorch, Julia, Rust or C++ has been made.

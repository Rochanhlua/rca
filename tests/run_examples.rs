//! Data-driven `run` tests over `examples/*.rca`.
//!
//! Complements `tests/examples.rs` (which tests `rca check`).

use std::path::PathBuf;
use std::process::{Command, Output};

fn rca() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rca"))
}

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
}

fn run(name: &str) -> Output {
    rca()
        .args(["run"])
        .arg(example(name))
        .output()
        .expect("spawn rca run")
}

const RUN_PASS: &[&str] = &[
    "autodiff_bias.rca",
    "autodiff_logistic.rca",
    "autodiff_mixed.rca",
    "autodiff_regression.rca",
    "autodiff_square.rca",
    "const_ok.rca",
    "convert_demo.rca",
    "full.rca",
    "hello.rca",
    "ir_demo.rca",
    "len_demo.rca",
    "opt_demo.rca",
    "push_demo.rca",
    "run_demo.rca",
    "destructure_fn.rca",
    "destructure_mut.rca",
    "destructure_ok.rca",
    "destructure_parse.rca",
    "destructure_scope.rca",
    "tuple_annot.rca",
    "tuple_ok.rca",
    "tuple_parse.rca",
    "tuple_run.rca",
    "enum_ok.rca",
    "enum_parse.rca",
    "enum_run.rca",
    "match_ok.rca",
    "match_parse.rca",
    "match_run.rca",
    "tensor_arith.rca",
    "tensor_broadcast.rca",
    "tensor_demo.rca",
    "tensor_index.rca",
    "tensor_math.rca",
    "tensor_matmul.rca",
    "tensor_numeric.rca",
    "try_break.rca",
    "try_demo.rca",
    "types_ok.rca",
    "native_if.rca",
    "native_compare.rca",
    "native_test.rca",
    "fs_demo.rca",
    "time_demo.rca",
    "string_demo.rca",
    "map_demo.rca",
    "json_demo.rca",
    "math_demo.rca",
];

const RUN_FAIL_AT_RUNTIME: &[&str] = &[
    "runtime_error.rca",
    "tensor_broadcast_bad.rca",
    "tensor_index_oob.rca",
    "tensor_math_bad.rca",
    "tensor_matmul_mismatch.rca",
    "tensor_ragged.rca",
    "tensor_reshape_bad.rca",
    "tensor_shape_mismatch.rca",
    "try_break2.rca",
];

#[test]
fn run_succeeds_on_runnable_examples() {
    let mut regressions = Vec::new();
    for name in RUN_PASS {
        let out = run(name);
        if !out.status.success() {
            regressions.push(format!(
                "  {}\n    exit: {}\n    stdout: {}\n    stderr: {}",
                name,
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stdout).trim(),
                String::from_utf8_lossy(&out.stderr).trim(),
            ));
        }
    }
    assert!(
        regressions.is_empty(),
        "{} of {} runnable example(s) regressed:\n{}",
        regressions.len(),
        RUN_PASS.len(),
        regressions.join("\n")
    );
}

#[test]
fn run_fails_at_runtime_on_intentionally_broken_examples() {
    let mut regressions = Vec::new();
    for name in RUN_FAIL_AT_RUNTIME {
        let out = run(name);
        if out.status.success() {
            regressions.push(format!(
                "  {} — run unexpectedly succeeded (expected runtime error)",
                name
            ));
            continue;
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        if !stderr.contains("runtime error:") {
            regressions.push(format!(
                "  {} — run failed but stderr lacks `runtime error:`\n    stderr: {}",
                name,
                stderr.trim(),
            ));
        }
    }
    assert!(
        regressions.is_empty(),
        "{} of {} intentionally-broken example(s) regressed:\n{}",
        regressions.len(),
        RUN_FAIL_AT_RUNTIME.len(),
        regressions.join("\n")
    );
}

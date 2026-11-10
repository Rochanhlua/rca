//! Data-driven `check` tests over every `.rca` file in `examples/`.
//!
//! Ground truth was established by running the real `rca check` binary on
//! each file in `examples/*.rca` (see project history). Do NOT reorder or
//! rename entries here without re-running that ground-truth sweep.
//!
//! - `SHOULD_PASS` (48): `check` MUST exit 0.
//! - `SHOULD_FAIL` (31): `check` MUST exit non-zero and MUST emit a
//!   diagnostic on stderr.
//!
//! Note: some files in `SHOULD_PASS` are *runtime*-bad by design
//! (`runtime_error.rca`, `tensor_index_oob.rca`, `tensor_broadcast_bad.rca`,
//! `tensor_math_bad.rca`, `tensor_matmul_mismatch.rca`, `tensor_ragged.rca`,
//! `tensor_reshape_bad.rca`, `tensor_shape_mismatch.rca`). `check` only runs
//! up to the type checker, not execution, so those correctly pass here.
//!
//! `examples/modules/` is covered separately by `tests/cli.rs`.
//!
//! Additive: this file only. No changes to `src/`, `examples/`, or `Cargo.toml`.
//!
//! Run: `cargo test --test examples`
//! All: `cargo test`

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

fn check(name: &str) -> Output {
    rca()
        .args(["check"])
        .arg(example(name))
        .output()
        .expect("spawn rca check")
}

/// Files whose `check` exits 0 (type-checks cleanly).
const SHOULD_PASS: &[&str] = &[
    // autodiff
    "autodiff_bias.rca",
    "autodiff_logistic.rca",
    "autodiff_mixed.rca",
    "autodiff_regression.rca",
    "autodiff_square.rca",
    // const
    "const_ok.rca",
    // core demos
    "convert_demo.rca",
    "full.rca",
    "hello.rca",
    "ir_demo.rca",
    "len_demo.rca",
    "opt_demo.rca",
    "push_demo.rca",
    "run_demo.rca",
    // destructure / tuple
    "destructure_fn.rca",
    "destructure_mut.rca",
    "destructure_ok.rca",
    "destructure_parse.rca",
    "destructure_scope.rca",
    "tuple_annot.rca",
    "tuple_ok.rca",
    "tuple_parse.rca",
    "tuple_run.rca",
    // enum / match
    "enum_ok.rca",
    "enum_parse.rca",
    "enum_run.rca",
    "match_ok.rca",
    "match_parse.rca",
    "match_run.rca",
    // types
    "types_ok.rca",
    "native_if.rca",
    "native_str.rca",
    "native_vars.rca",
    "native_while.rca",
    "native_for.rca",
    "native_fn.rca",
    "native_float.rca",
    "native_strings.rca",
    "native_compare.rca",
    "native_test.rca",
    // runtime-error examples (check passes, run would fail)
    "runtime_error.rca",
    // tensor (check passes; some are runtime-bad by design)
    "tensor_arith.rca",
    "tensor_broadcast.rca",
    "tensor_broadcast_bad.rca",
    "tensor_demo.rca",
    "tensor_index.rca",
    "tensor_index_oob.rca",
    "tensor_math.rca",
    "tensor_math_bad.rca",
    "tensor_matmul.rca",
    "tensor_matmul_mismatch.rca",
    "tensor_numeric.rca",
    "tensor_ragged.rca",
    "tensor_reshape_bad.rca",
    "tensor_shape_mismatch.rca",
    // try/catch
    "try_break.rca",
    "try_break2.rca",
    "try_demo.rca",
    "fs_demo.rca",
    "time_demo.rca",
    "string_demo.rca",
    "map_demo.rca",
    "json_demo.rca",
    "math_demo.rca",
];

/// Files whose `check` exits non-zero with a diagnostic on stderr.
const SHOULD_FAIL: &[&str] = &[
    "bad.rca",
    "const_assign.rca",
    "const_call.rca",
    "const_nested.rca",
    "const_var.rca",
    "destructure_dup_bad.rca",
    "destructure_nontuple_bad.rca",
    "destructure_size_bad.rca",
    "enum_bad_type.rca",
    "enum_bad_variant.rca",
    "enum_clash.rca",
    "enum_cmp_bad.rca",
    "enum_dup_variant.rca",
    "enum_nested_bad.rca",
    "functions_bad.rca",
    "match_dup.rca",
    "match_float.rca",
    "match_missing.rca",
    "match_no_wild.rca",
    "match_type.rca",
    "match_unreach.rca",
    "tensor_arith_bad_int.rca",
    "tensor_bad_int.rca",
    "tensor_index_bad.rca",
    "tensor_matmul_bad.rca",
    "tensor_sum_bad.rca",
    "tuple_annot_bad.rca",
    "tuple_annot_one.rca",
    "tuple_assign_bad.rca",
    "tuple_range_bad.rca",
    "types_bad.rca",
];

#[test]
fn check_passes_on_valid_examples() {
    let mut regressions = Vec::new();
    for name in SHOULD_PASS {
        let out = check(name);
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
        "{} of {} valid example(s) regressed:\n{}",
        regressions.len(),
        SHOULD_PASS.len(),
        regressions.join("\n")
    );
}

#[test]
fn check_rejects_invalid_examples() {
    let mut regressions = Vec::new();
    for name in SHOULD_FAIL {
        let out = check(name);
        if out.status.success() {
            regressions.push(format!("  {} — check unexpectedly passed", name));
            continue;
        }
        if out.stderr.is_empty() {
            regressions.push(format!("  {} — check failed but emitted no stderr", name));
        }
    }
    assert!(
        regressions.is_empty(),
        "{} of {} invalid example(s) regressed:\n{}",
        regressions.len(),
        SHOULD_FAIL.len(),
        regressions.join("\n")
    );
}

// tests/compile_fail.rs
//
// Trybuild harness for compile-fail tests.
//
// This file is the cargo test target entrypoint. It is invoked
// by `cargo test --test compile_fail`. Trybuild reads .rs files
// directly from disk via the glob pattern below; the test
// files are NOT imported as Rust modules and there must be NO
// `mod` declarations here — the files are intentionally
// non-compiling Rust (that is the property under test).
//
// Layout:
//   tests/
//     compile_fail.rs           <-- this file (the harness)
//     compile_fail/
//       clone_attempt.rs        <-- 7 test files; each must
//       double_spend.rs            be rejected by rustc with
//       spend_after_split.rs       the expected error code.
//       double_capture.rs
//       budget_in_rc.rs
//       borrow_after_split.rs
//       spend_through_shared_ref.rs
//
// Dev-dependencies in Cargo.toml:
//     [dev-dependencies]
//     trybuild = "1.0"
//
// First run (captures expected stderr per file):
//     TRYBUILD=overwrite cargo test --test compile_fail -- --nocapture
//
// Subsequent runs (verify stderr matches the captured baseline):
//     cargo test --test compile_fail -- --nocapture


#[test]
fn compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
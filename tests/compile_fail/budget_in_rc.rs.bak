// tests/compile_fail/budget_in_rc.rs
//
// Compile-fail test #5 (NEW, FIXED): demonstrates that attempting
// to share a Budget across OS threads via Rc (non-thread-safe
// reference counting) breaks the Send bound required for
// std::thread::spawn, because Rc is !Send by design.
//
// Expected rejection: E0277 (the trait `Send` is not implemented for `Rc<Budget<MAX>>`)
//
// This catches the common error of trying to "share" a Budget
// among multiple threads. The affine discipline requires that
// Budgets be transferred by move (split-then-spawn); shared
// non-Send references break the soundness story. The compiler
// enforces this via the Send bound on thread::spawn.
//
// NOTE: This test uses std::thread::spawn instead of
// tokio::spawn to avoid a tokio dev-dependency. The Send bound
// is the same in both runtimes (Tokio's spawn signature is
// `F: Future + Send + 'static`; std::thread::spawn's is
// `F: FnOnce() -> T + Send + 'static`); the rejection mechanism
// is identical.

use std::rc::Rc;
use std::thread;
use token_budgets::Budget;

fn main() {
    let budget = Budget::<10000>::new(500).unwrap();
    let shared = Rc::new(budget);

    let s1 = Rc::clone(&shared);
    // ERROR: Rc<Budget<10000>> does not implement Send, so it
    // cannot be moved into a std::thread::spawn'd closure.
    let handle = thread::spawn(move || {
        let _ = s1.micro_cents();
    });
    handle.join().unwrap();
}
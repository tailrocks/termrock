//! Cases that change process-global state (the panic hook, SIGTSTP) run in a
//! child process. Each `#[test]` is a parent that re-executes this test
//! binary, filtered to itself, with [`MARKER`] set to its own name. Only that
//! child touches the process-wide state, so no other test in the runner's
//! process sees it.

use std::process::{Command, Output, Stdio};

const MARKER: &str = "JUNIE_TUI_RUNTIME_CHILD";
const DONE: &str = "runtime-child-done";

/// Runs `body` in a child process. `module` is the caller's `module_path!()`.
pub(super) fn child_case(module: &str, name: &str, body: impl FnOnce()) {
    if std::env::var(MARKER).as_deref() == Ok(name) {
        body();
        println!("{DONE}");
        return;
    }
    let module = module.split_once("::").expect("crate prefix").1;
    let output = Command::new(std::env::current_exe().expect("test binary path"))
        .args([
            "--exact",
            &format!("{module}::{name}"),
            "--nocapture",
            "--test-threads=1",
        ])
        .env(MARKER, name)
        .stdin(Stdio::null())
        .output()
        .expect("spawn child");
    assert_child_passed(name, &output);
}

fn assert_child_passed(name: &str, output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stdout.contains(DONE),
        "child {name} failed: {}\n--- stdout\n{stdout}\n--- stderr\n{stderr}",
        output.status
    );
}

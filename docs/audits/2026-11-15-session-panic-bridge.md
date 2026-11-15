<!-- SPDX-License-Identifier: LicenseRef-Proprietary -->

# Panic restoration and session ownership in the runtime

Issue #23. Every successful `TerminalSession::enter_with` used to take the panic hook and wrap it in a new closure that always restored the terminal. After N sessions a later, unrelated caught panic restored the terminal N times, after the sessions had ended, and called the host hook once. Nested entry also entered raw mode and the alternate screen before failing on the SIGTSTP claim (#22), and its cleanup tore down the live session's modes.

## Contract

- One permanent bridge. The first entry in the process installs it with `std::sync::Once`. It captures the hook present at that moment, restores the terminal only if a session is active, and then calls that hook once. Later entries never touch the hook, so the chain length is constant.
- The hook is never changed from `Drop` or while panicking. `install_once` called from an unwinding thread returns an error before any hook mutation.
- Ownership comes first. `enter_with` claims the one session owner before it touches the terminal. A second entry fails with `ErrorKind::AlreadyExists`, runs no setup and tears nothing down. The claim lasts until the session drops, also after `try_leave`.
- Restoration is once per active interval. One generation-tagged atomic word (owner, interval, phase, release-pending bit) decides which of `try_leave`, `Drop` and the panic hook cleans up. The other two see the interval as finished. `suspend` ends the interval and the resume starts a new one.
- A hook on another thread never lets a new session start mid-cleanup. While a cleanup runs, a new claim fails with `AlreadyExists`, `try_leave`, `suspend` and re-entry fail with `ErrorKind::WouldBlock`, and a `Drop` defers its release to the cleanup winner. Nothing waits or spins in the hook or in `Drop`. A stale hook that observed an earlier interval cannot restore a later one.
- Setup is under the guard. `enable_raw_mode` runs inside the claimed interval. A failed setup, SIGTSTP install or resume restores once and releases the claim.
- `try_leave` restores the terminal first and SIGTSTP last. It keeps the signal guard when the restore fails or when another thread is still cleaning up.

## Limits a host must know

The bridge is process wide, so a panic on any thread while a session is active restores the terminal, even when that panic is later caught. That matches the old policy and differs from the fixed bug, which restored after the session ended.

Rust has no safe general way to ask which hook is installed. A host that calls `std::panic::set_hook` after the first entry replaces the bridge, and the library does not reinstall it. Unwinding still restores through `Drop`. With `panic = "abort"` the replacing hook must restore the terminal itself or wrap the hook it replaced. The library does not claim to see a replacement.

## Evidence

State-machine cases in `src/runtime/panic_bridge/tests.rs` use local gates with counting callbacks: once per interval, stale snapshot and stale token, exhaustion, a panicking cleanup, a cleanup paused at a barrier while `Drop`, a new claim and re-entry race it. Hook cases run in child processes: N sessions leave a single host call, an active panic restores once, a replacement hook is respected, and installing while panicking is refused. `src/runtime/tests.rs` covers nested entry, failed setup, failed SIGTSTP install, a failed `try_leave` keeping the guard, and `try_leave` during a hook cleanup. `tests/functional/lifecycle.rs` drives the same two behaviors, an active panic and a nested entry, through a tuiscotti PTY and reads the raw-mode state.

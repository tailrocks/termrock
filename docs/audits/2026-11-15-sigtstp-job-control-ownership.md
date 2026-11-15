<!-- SPDX-License-Identifier: LicenseRef-Proprietary -->

# SIGTSTP ownership in the runtime session

Issue #22. A terminal session used to install its SIGTSTP handler with `signal()`, discard the previous action, and never put it back. After `runtime::run` returned, the host's Ctrl+Z action, or a custom handler, was replaced by a library handler that only set a flag nothing read.

## v0.1 API migration

| Before | After |
| --- | --- |
| `job_control::install()` returned `()` and could not be undone | `job_control::install() -> io::Result<JobControlGuard>`, `#[must_use]`. Dropping the guard restores the saved action. |
| `job_control::stop_self()` was a free function | `JobControlGuard::stop_self(&mut self) -> io::Result<()>`. A caller can no longer reinstall a handler it does not own. |
| `runtime::run` called `install()` after entering | `TerminalSession::enter_with` installs the guard once terminal setup succeeds, so direct session users own SIGTSTP too. |
| `TerminalSession::leave()` only | `TerminalSession::try_leave() -> io::Result<()>` reports a failed SIGTSTP restore. `leave()` keeps its signature and discards the error. |

`job_control::take_request` is unchanged. No other caller of the removed forms existed in the library, the showcase, TablePro, Holla or Jackin.

## Contract

- The saved action is the whole `sigaction`: handler (including `SIG_IGN`), mask and flags. It is restored on normal exit, on an event-loop or `suspend` error, and on unwind, because the guard is a field of `TerminalSession` and a drop restores it.
- Ownership is exclusive. A second `install()` while a guard lives fails with `ErrorKind::AlreadyExists` before any handler or pending request changes. A host must not replace the SIGTSTP action while a session is active.
- Claiming and releasing ownership both clear the pending request, so a Ctrl+Z consumed or left over in one session never suspends the next.
- The installed action has an empty mask and no `SA_RESTART`, so a blocked terminal read returns `Interrupted`, which `poll_uninterrupted` already handles.
- `suspend` leaves terminal modes and keeps the SIGTSTP guard. `stop_self` swaps in `SIG_DFL`, raises, and reinstates the owned action. It never raises under an action it did not set, and a failed raise still reinstates before returning the error.
- `try_leave` restores terminal modes first and the SIGTSTP action last. A failed restore keeps the claim, so a later `try_leave` or the drop retries.

## Composition with panic restoration (#23)

Terminal restoration and SIGTSTP ownership are separate lifetimes. The panic hook restores terminal modes only and never touches signal state. The unwind that follows drops the session, which restores SIGTSTP after the terminal. A later panic-hook owner can wrap or replace the bridge without moving the guard, because the guard lives in the session and not in the hook. Unlike the hook, the SIGTSTP action is not replaceable by hosts while a session is active.

## Evidence

Subprocess cases in `src/runtime/job_control/tests.rs` cover the exact action roundtrip, error and unwind, pending requests across sessions, a second owner, `SIG_IGN`, and a fault at each of install, restore, default-install, raise and reinstate. A real stop and `SIGCONT` runs in a child in its own process group. `tests/functional/cases/x02_suspend.rs::suspend_resume_restores_host_handler` drives an active session through suspend, resume and exit in a tuiscotti PTY. Every case that changes process-wide signal state runs in a child process, so no test changes the runner's own disposition.

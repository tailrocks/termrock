//! The terminal session, the bounded external channel, the raw-input sources and
//! policies, the panic bridge and the clock.
//!
//! `Application` and the run loop that is generic over it are not here: they
//! stay in `junie-tui` until #448 moves them into `termrock-shell` beside `App`,
//! so that `impl<A: App> Application for A` is legal (see `docs/decisions.md`).
//! `termrock::runtime` is the `junie-tui` module that re-exports `runtime` here
//! next to them, and `termrock::clock` is `clock`.

pub mod clock;
pub mod runtime;

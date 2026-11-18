//! An independent host of the `jackin-console` library with defaults off.
//!
//! The package has its own `[workspace]` and lockfile. `jackin-console`
//! keeps `clap`, `jackin_console::fixtures` and `jackin_console::sim` behind
//! its `demo` feature, and a host must not get them by default. The
//! intentional `terminal` dependency of the library stays. The regression
//! `consumer_jackin_host_excludes_demo` in `tests/regressions` checks this
//! package's lockfile, and `scripts/check-consumers.sh` prints its normal tree.

use jackin_console::launch::LaunchTarget;

/// A launch request for `role`, with no workspace and no instances.
pub fn first_launch(role: &str) -> LaunchTarget {
    LaunchTarget {
        workspace: None,
        role: role.to_owned(),
        instances: Vec::new(),
        task: None,
    }
}

#[cfg(test)]
mod tests {
    use jackin_console::domain::agent::Agent;

    use super::*;

    #[test]
    fn consumer_jackin_host_resolves_the_host_api() {
        let target = first_launch("the-architect");
        assert_eq!(target.role, "the-architect");
        assert_eq!(target.agent(), Agent::Codex);
        assert!(target.account().is_none());
    }
}

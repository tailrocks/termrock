// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Motion policy and the one flash clock.
//!
//! junie has two motion states and no ambient vocabulary: loops, eases,
//! blends, and channel periods are gone. What remains is [`MotionPolicy`]
//! ({[`MotionPolicy::Full`], [`MotionPolicy::Off`]}), the braille spinner's
//! 80 ms tick, and the 140 ms pressed/acknowledged flash. Callers still supply
//! deterministic ticks; nothing here reads a clock, so every helper is
//! snapshot-testable.
/// Motion tier.
///
/// `Off` is never frozen into illegibility: status stays readable, spinners
/// park on their first frame, and state changes snap instead of animating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[non_exhaustive]
pub enum MotionPolicy {
    /// Everything: spinners advance and the pressed flash plays.
    #[default]
    Full,
    /// Instant state changes; status carried by non-motion channels.
    Off,
}

impl MotionPolicy {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Off => "off",
        }
    }

    /// Parse `full` / `off` (also `none`, `reduced`, `0`, `1`, `true`).
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "full" | "1" | "true" => Some(Self::Full),
            "off" | "none" | "reduced" | "0" | "false" => Some(Self::Off),
            _ => None,
        }
    }
    /// Whether indeterminate spinners should advance frames.
    #[must_use]
    pub const fn animate_spinners(self) -> bool {
        matches!(self, Self::Full)
    }

    /// Whether state changes may animate rather than snap.
    ///
    /// With the ambient vocabulary gone, the only transitions left are the
    /// pressed flash and the spinner cadence, and both answer this directly.
    #[must_use]
    pub const fn allows_transitions(self) -> bool {
        matches!(self, Self::Full)
    }
}

/// How long a fired action shows its acknowledgement.
///
/// junie's pressed flash: 140 ms, binary on/off. The old ease-out fade is
/// deleted with the rest of the easing vocabulary.
pub const ACTION_FLASH_MS: u64 = 140;

/// The acknowledgement a fired action owes the operator.
///
/// Copy is the case that needs it most: nothing on screen changes when text
/// reaches the clipboard, so without a mark the operator cannot tell a
/// successful copy from a swallowed keystroke and presses again. One shared
/// stamp means every site agrees on how long the mark stays and which tier
/// suppresses it, instead of nine widgets each picking a duration
/// (plans/021 Step 2, plans/014).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ActionFlash {
    fired_at_ms: Option<u64>,
}

impl ActionFlash {
    /// Never fired.
    #[must_use]
    pub const fn new() -> Self {
        Self { fired_at_ms: None }
    }

    /// Stamps the moment the action fired.
    pub const fn fire(&mut self, elapsed_ms: u64) {
        self.fired_at_ms = Some(elapsed_ms);
    }

    /// Clears the mark early (the surface closed, the selection moved on).
    pub const fn clear(&mut self) {
        self.fired_at_ms = None;
    }

    /// Whether the acknowledgement is still owed at `elapsed_ms`.
    ///
    /// Reduced motion keeps the mark: it is a statement of fact, not an
    /// animation, and an operator who suppressed motion still needs to know
    /// the copy happened. Only the *fade* is a transition.
    #[must_use]
    pub fn is_lit(self, elapsed_ms: u64) -> bool {
        self.fired_at_ms
            .is_some_and(|at| elapsed_ms.saturating_sub(at) < ACTION_FLASH_MS)
    }

    /// Brightness of the mark: `1.0` while lit, `0.0` once expired.
    ///
    /// The flash is a statement of fact, not an animation, so it is binary at
    /// every tier; `Off` suppresses the repaint churn but the lit state is
    /// still honest.
    #[must_use]
    pub const fn alpha(self, policy: MotionPolicy, elapsed_ms: u64) -> f32 {
        if !policy.allows_transitions() {
            return 0.0;
        }
        match self.fired_at_ms {
            Some(at) if elapsed_ms.saturating_sub(at) < ACTION_FLASH_MS => 1.0,
            _ => 0.0,
        }
    }

    /// When the mark next needs a repaint, for the host's frame scheduler.
    #[must_use]
    pub fn next_deadline_ms(self, elapsed_ms: u64) -> Option<u64> {
        self.fired_at_ms
            .map(|at| at.saturating_add(ACTION_FLASH_MS))
            .filter(|end| *end > elapsed_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_has_two_states_and_off_snaps() {
        assert_eq!(MotionPolicy::default(), MotionPolicy::Full);
        assert!(MotionPolicy::Full.animate_spinners());
        assert!(MotionPolicy::Full.allows_transitions());
        assert!(!MotionPolicy::Off.animate_spinners());
        assert!(!MotionPolicy::Off.allows_transitions());
        assert_ne!(MotionPolicy::Full.id(), MotionPolicy::Off.id());
    }

    #[test]
    fn parse_and_env_agree_on_names() {
        assert_eq!(MotionPolicy::parse("off"), Some(MotionPolicy::Off));
        assert_eq!(MotionPolicy::parse("reduced"), Some(MotionPolicy::Off));
        assert_eq!(MotionPolicy::parse("full"), Some(MotionPolicy::Full));
        assert_eq!(MotionPolicy::parse("nonsense"), None);
    }

    #[test]
    fn flash_is_a_140ms_binary_mark() {
        assert_eq!(ACTION_FLASH_MS, 140);
        let mut flash = ActionFlash::new();
        assert_eq!(flash.alpha(MotionPolicy::Full, 0), 0.0);
        flash.fire(500);
        assert!(flash.is_lit(600));
        assert_eq!(flash.alpha(MotionPolicy::Full, 600), 1.0);
        // The window closes hard at 140 ms; there is no fade tail.
        assert_eq!(flash.alpha(MotionPolicy::Full, 640), 0.0);
        assert!(!flash.is_lit(640));
        assert_eq!(flash.next_deadline_ms(600), Some(640));
        // `Off` suppresses the flash animation entirely.
        assert_eq!(flash.alpha(MotionPolicy::Off, 600), 0.0);
        flash.clear();
        assert!(!flash.is_lit(600));
    }
}

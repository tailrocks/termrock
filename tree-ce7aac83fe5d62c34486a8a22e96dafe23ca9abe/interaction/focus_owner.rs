// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! `FocusOwner` — per-screen single focus owner model.
//!
//! Every screen that uses tab bars + content blocks instantiates one
//! `FocusOwner<Tab>` where `Tab` is an enum naming the tabs. The current
//! owner is always one of: the tab bar, or a specific content block.
//!
//! All rendering decisions (green border, ▸ cursor, hint scope) derive from
//! this single value rather than from scattered bools.

use crate::widgets::PanelEmphasis;

/// Focus behavior shared by every button-row dialog: a closed ring of
/// semantic focus states with a stable button-strip index.
pub trait ButtonFocus: Copy + Eq + 'static {
    /// The `RING` constant.
    const RING: &'static [Self];

    /// Index into the dialog's button strip items.
    #[must_use]
    fn index(self) -> usize {
        Self::RING
            .iter()
            .position(|focus| focus == &self)
            .unwrap_or(0)
    }

    #[must_use]
    /// Performs the `next` operation.
    fn next(self) -> Self {
        let ring = Self::RING;
        if ring.is_empty() {
            return self;
        }
        let idx = self.index();
        ring[(idx + 1) % ring.len()]
    }

    #[must_use]
    /// Performs the `prev` operation.
    fn prev(self) -> Self {
        let ring = Self::RING;
        if ring.is_empty() {
            return self;
        }
        let idx = self.index();
        ring[(idx + ring.len() - 1) % ring.len()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Available `FocusOwner` choices.
pub enum FocusOwner<Tab> {
    #[default]
    /// The tab bar owns focus (green tab underline, dark content borders, no ▸).
    TabBar,
    /// A specific tab's content block owns focus (green border, ▸ cursor, white tab underline).
    Content(Tab),
}

impl<Tab: Copy> FocusOwner<Tab> {
    #[must_use]
    /// Returns whether `tab_bar`.
    pub const fn is_tab_bar(self) -> bool {
        matches!(self, Self::TabBar)
    }

    #[must_use]
    /// Returns whether `content`.
    pub const fn is_content(self) -> bool {
        matches!(self, Self::Content(_))
    }

    #[must_use]
    /// Performs the `content_tab` operation.
    pub fn content_tab(self) -> Option<Tab> {
        match self {
            Self::Content(tab) => Some(tab),
            Self::TabBar => None,
        }
    }

    /// Return the panel emphasis for the content block identified by `tab`.
    ///
    /// Returns `Focused` when this owner is `Content(t)` where `tab == t`,
    /// and `Normal` otherwise (including when the tab bar owns focus).
    #[must_use]
    pub fn panel_emphasis_for<F: PartialEq<Tab>>(self, tab: &F) -> PanelEmphasis {
        match self {
            Self::Content(owned) if tab == &owned => PanelEmphasis::Focused,
            _ => PanelEmphasis::Normal,
        }
    }

    /// Whether the ▸ cursor should appear on the content block for `tab`.
    #[must_use]
    pub fn show_cursor_for<F: PartialEq<Tab>>(self, tab: &F) -> bool {
        matches!(self, Self::Content(ref owned) if tab == owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Button {
        First,
        Second,
        Third,
    }

    impl ButtonFocus for Button {
        const RING: &'static [Self] = &[Self::First, Self::Second, Self::Third];
    }

    #[test]
    fn button_focus_cycles_and_wraps_in_both_directions() {
        assert_eq!(Button::First.next().next().next(), Button::First);
        assert_eq!(Button::First.prev(), Button::Third);
        assert_eq!(Button::Third.next(), Button::First);
    }

    #[test]
    fn panel_emphasis_and_cursor_follow_the_owned_tab() {
        let owner = FocusOwner::Content(Button::Second);
        assert_eq!(
            owner.panel_emphasis_for(&Button::Second),
            PanelEmphasis::Focused
        );
        assert_eq!(
            owner.panel_emphasis_for(&Button::First),
            PanelEmphasis::Normal
        );
        assert!(owner.show_cursor_for(&Button::Second));
        assert!(!owner.show_cursor_for(&Button::First));
        assert_eq!(
            FocusOwner::TabBar.panel_emphasis_for(&Button::Second),
            PanelEmphasis::Normal
        );
    }
}

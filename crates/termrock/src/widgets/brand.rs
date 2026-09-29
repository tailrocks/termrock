// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Product-neutral brand lockup.
//!
//! The host supplies the mark. The widget owns the single accent-filled
//! identity treatment used across Junie surfaces; it never embeds a product
//! name or glyph.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Widget,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState},
    style::{DesignSystem, Role},
    text::{display_cols, display_cols_slice},
};

/// Paint state for a clickable lockup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LockupState {
    /// Pointer is over the lockup.
    pub hovered: bool,
    /// Pointer is pressing the lockup.
    pub pressed: bool,
}

/// Painted lockup geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LockupParts {
    /// Allocation supplied by the host.
    pub root: Rect,
    /// Cells painted by the lockup.
    pub content: Rect,
}

/// Accent-filled application identity mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lockup<'a> {
    mark: &'a str,
    system: &'a DesignSystem,
    compact: bool,
}

impl<'a> Lockup<'a> {
    /// Creates a padded lockup from a host-supplied mark.
    #[must_use]
    pub const fn new(mark: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            mark,
            system,
            compact: false,
        }
    }

    /// Drops the one-cell outer padding for tight strips.
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Mark text supplied by the host.
    #[must_use]
    pub const fn mark(&self) -> &'a str {
        self.mark
    }

    /// Whether this lockup uses compact geometry.
    #[must_use]
    pub const fn is_compact(&self) -> bool {
        self.compact
    }

    fn label(&self) -> String {
        if self.compact {
            self.mark.to_owned()
        } else {
            format!(" {} ", self.mark)
        }
    }

    /// Display-cell width of the rendered lockup.
    #[must_use]
    pub fn width(&self) -> u16 {
        u16::try_from(display_cols(&self.label())).unwrap_or(u16::MAX)
    }

    fn style(&self, state: LockupState) -> Style {
        let theme = self.system.junie_theme();
        let background = if state.pressed {
            theme.accent_pressed
        } else if state.hovered {
            theme.accent_hover
        } else {
            theme.accent
        };
        let mut style = self.system.style(Role::TextOnAccent);
        style.bg = Some(background);
        style.add_modifier(Modifier::BOLD)
    }

    fn parts_for_label(area: Rect, label: &str) -> LockupParts {
        let max_width = u16::try_from(display_cols(label))
            .unwrap_or(u16::MAX)
            .min(area.width);
        let clipped = display_cols_slice(label, 0, usize::from(max_width));
        let content_width = u16::try_from(display_cols(clipped.as_ref()))
            .unwrap_or(u16::MAX)
            .min(max_width);
        LockupParts {
            root: area,
            content: Rect::new(area.x, area.y, content_width, 1.min(area.height)),
        }
    }

    /// Paints a resting lockup into the supplied row.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) -> LockupParts {
        self.paint_with_state(area, buffer, LockupState::default())
    }

    /// Paints a lockup with explicit hover/press state.
    pub fn paint_with_state(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: LockupState,
    ) -> LockupParts {
        let label = self.label();
        let parts = Self::parts_for_label(area, &label);
        let content = parts.content.intersection(*buffer.area());
        if content.is_empty() {
            return LockupParts { content, ..parts };
        }
        let skip = usize::from(content.x.saturating_sub(parts.content.x));
        let (leading_gap, clipped) = display_window(&label, skip, usize::from(content.width));
        let paint_x = content
            .x
            .saturating_add(u16::try_from(leading_gap).unwrap_or(content.width));
        let paint_width = content.width.saturating_sub(leading_gap as u16);
        let painted_width = u16::try_from(display_cols(&clipped))
            .unwrap_or(u16::MAX)
            .min(paint_width);
        if clipped.is_empty() || painted_width == 0 {
            return LockupParts {
                content: Rect::new(content.x, content.y, 0, 0),
                ..parts
            };
        }
        let painted_content = Rect::new(paint_x, content.y, painted_width, content.height);
        buffer.set_stringn(
            painted_content.x,
            painted_content.y,
            &clipped,
            usize::from(painted_content.width),
            self.style(state),
        );
        LockupParts {
            content: painted_content,
            ..parts
        }
    }

    /// Registers the painted parts returned by [`Self::paint`] in the
    /// frame-local semantic scene.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        parts: LockupParts,
        interactive: bool,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        self.register_semantic_with_state(scene, id, parts, interactive, LockupState::default());
    }

    /// Registers the painted region with explicit visual state.
    ///
    /// The semantic schema exposes `pressed` but not `hovered`; hover remains
    /// a paint-only state while the pressed flag is projected for interactive
    /// lockups. Use [`Self::register_semantic`] for resting state.
    pub fn register_semantic_with_state<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        parts: LockupParts,
        interactive: bool,
        state: LockupState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if parts.content.is_empty() {
            return;
        }
        let node = if interactive {
            SemanticNode::control(id, parts.content)
                .role(SemanticRole::Control)
                .label(self.mark)
                .description("application brand lockup")
                .focusable(true)
                .state(SemanticState {
                    pressed: state.pressed,
                    ..Default::default()
                })
        } else {
            SemanticNode::content(id, parts.content)
                .role(SemanticRole::Chrome)
                .label(self.mark)
                .description("application brand lockup")
                .focusable(false)
        };
        let _ = scene.register(node);
    }
}

fn display_window(label: &str, skip: usize, width: usize) -> (usize, String) {
    let end = skip.saturating_add(width);
    let mut column: usize = 0;
    let mut first_visible = None;
    for grapheme in label.graphemes(true) {
        let grapheme_width = display_cols(grapheme);
        let start = column;
        column = column.saturating_add(grapheme_width);
        if grapheme_width > 0 && start >= skip && column <= end {
            first_visible = Some(start);
            break;
        }
        if column >= end {
            break;
        }
    }
    let clipped = display_cols_slice(label, skip, width);
    let leading_gap = first_visible
        .map(|start| start.saturating_sub(skip))
        .unwrap_or(width);
    (leading_gap.min(width), clipped)
}

impl Widget for &Lockup<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer);
    }
}

impl Widget for Lockup<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::SemanticRole;

    #[test]
    fn padded_and_compact_lockups_preserve_mark_and_width() {
        let system = DesignSystem::default();
        let lockup = Lockup::new("mark❯", &system);
        assert_eq!(lockup.width(), 7);
        assert_eq!(lockup.mark(), "mark❯");
        assert!(!lockup.is_compact());

        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 1));
        let parts = lockup.paint(Rect::new(0, 0, 20, 1), &mut buffer);
        assert_eq!(parts.content.width, 7);
        assert_eq!(buffer[(0, 0)].symbol(), " ");
        assert_eq!(buffer[(1, 0)].symbol(), "m");
        assert_eq!(buffer[(1, 0)].fg, system.junie_theme().text_on_accent);
        assert_eq!(buffer[(1, 0)].bg, system.junie_theme().accent);
        assert!(buffer[(1, 0)].modifier.contains(Modifier::BOLD));

        let compact = lockup.compact();
        assert!(compact.is_compact());
        assert_eq!(compact.width(), 5);
    }

    #[test]
    fn clickable_state_uses_accent_ladder_and_semantic_role() {
        let system = DesignSystem::default();
        let lockup = Lockup::new("x", &system);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 1));
        let _ = lockup.paint_with_state(
            Rect::new(0, 0, 8, 1),
            &mut buffer,
            LockupState {
                hovered: true,
                pressed: false,
            },
        );
        assert_eq!(buffer[(1, 0)].bg, system.junie_theme().accent_hover);
        let _ = lockup.paint_with_state(
            Rect::new(0, 0, 8, 1),
            &mut buffer,
            LockupState {
                hovered: true,
                pressed: true,
            },
        );
        assert_eq!(buffer[(1, 0)].bg, system.junie_theme().accent_pressed);

        let mut scene = SemanticScene::<&str>::new();
        let parts = lockup.paint(Rect::new(2, 0, 8, 1), &mut buffer);
        lockup.register_semantic(&mut scene, "brand", parts, true);
        assert_eq!(scene.nodes().len(), 1);
        assert_eq!(scene.nodes()[0].role, SemanticRole::Control);
        assert_eq!(scene.nodes()[0].area.width, 3);
    }

    #[test]
    fn wide_glyphs_are_not_partially_painted_or_hit_targeted() {
        let system = DesignSystem::default();
        let compact = Lockup::new("界", &system).compact();
        let area = Rect::new(0, 0, 1, 1);
        let mut buffer = Buffer::empty(area);

        let parts = compact.paint(area, &mut buffer);
        assert!(parts.content.is_empty());
        assert_eq!(buffer[(0, 0)].symbol(), " ");

        let mut scene = SemanticScene::<&str>::new();
        compact.register_semantic(&mut scene, "wide", parts, true);
        assert!(scene.nodes().is_empty());

        let padded = Lockup::new("界", &system);
        let short = Rect::new(0, 0, 2, 1);
        let mut buffer = Buffer::empty(short);
        let parts = padded.paint(short, &mut buffer);
        assert_eq!(parts.content.width, 1);
        assert_eq!(buffer[(0, 0)].symbol(), " ");
        assert_eq!(buffer[(1, 0)].symbol(), " ");
    }

    #[test]
    fn ascii_truncation_keeps_painted_and_semantic_width_aligned() {
        let system = DesignSystem::default();
        let lockup = Lockup::new("abcd", &system).compact();
        let area = Rect::new(0, 0, 2, 1);
        let mut buffer = Buffer::empty(area);

        let parts = lockup.paint(area, &mut buffer);
        assert_eq!(parts.content.width, 2);
        assert_eq!(buffer[(0, 0)].symbol(), "a");
        assert_eq!(buffer[(1, 0)].symbol(), "b");

        let mut scene = SemanticScene::<&str>::new();
        lockup.register_semantic(&mut scene, "ascii", parts, true);
        assert_eq!(scene.nodes()[0].area, parts.content);
    }

    #[test]
    fn zero_and_short_areas_are_safe() {
        let system = DesignSystem::default();
        let lockup = Lockup::new("mark", &system);
        let buffer_area = Rect::new(0, 0, 8, 1);
        let mut buffer = Buffer::empty(buffer_area);

        let zero_width = Rect::new(0, 0, 0, 1);
        assert!(lockup.paint(zero_width, &mut buffer).content.is_empty());
        let zero_height = Rect::new(0, 0, 8, 0);
        assert!(lockup.paint(zero_height, &mut buffer).content.is_empty());

        let mut scene = SemanticScene::<&str>::new();
        let zero_width_parts = lockup.paint(zero_width, &mut buffer);
        let zero_height_parts = lockup.paint(zero_height, &mut buffer);
        lockup.register_semantic(&mut scene, "zero", zero_width_parts, true);
        lockup.register_semantic(&mut scene, "short", zero_height_parts, false);
        assert!(scene.nodes().is_empty());
    }

    #[test]
    fn paint_clips_to_offset_and_partial_buffer_area() {
        let system = DesignSystem::default();
        let lockup = Lockup::new("mark", &system).compact();
        let buffer_area = Rect::new(10, 4, 5, 1);
        let mut buffer = Buffer::empty(buffer_area);

        let parts = lockup.paint(Rect::new(8, 4, 8, 1), &mut buffer);
        assert_eq!(parts.content, Rect::new(10, 4, 2, 1));
        assert_eq!(buffer[(10, 4)].symbol(), "r");
        assert_eq!(buffer[(11, 4)].symbol(), "k");
        assert_eq!(buffer[(12, 4)].symbol(), " ");

        let mut scene = SemanticScene::<&str>::new();
        lockup.register_semantic(&mut scene, "brand", parts, true);
        assert_eq!(scene.nodes()[0].area, parts.content);
    }

    #[test]
    fn disjoint_and_zero_height_areas_paint_no_content() {
        let system = DesignSystem::default();
        let lockup = Lockup::new("mark", &system);
        let buffer_area = Rect::new(10, 4, 5, 1);
        let mut buffer = Buffer::empty(buffer_area);

        let disjoint = lockup.paint(Rect::new(0, 0, 8, 1), &mut buffer);
        assert!(disjoint.content.is_empty());
        let zero_height = lockup.paint(Rect::new(10, 4, 5, 0), &mut buffer);
        assert!(zero_height.content.is_empty());

        let mut scene = SemanticScene::<&str>::new();
        lockup.register_semantic(&mut scene, "disjoint", disjoint, true);
        lockup.register_semantic(&mut scene, "zero-height", zero_height, true);
        assert!(scene.nodes().is_empty());
    }

    #[test]
    fn left_clipped_wide_graphemes_preserve_later_columns() {
        let system = DesignSystem::default();
        for mark in ["界X", "👩‍💻X"] {
            let lockup = Lockup::new(mark, &system).compact();
            let mut buffer = Buffer::empty(Rect::new(9, 0, 3, 1));

            let parts = lockup.paint(Rect::new(8, 0, 5, 1), &mut buffer);

            assert_eq!(buffer[(9, 0)].symbol(), " ", "{mark:?}");
            assert_eq!(buffer[(10, 0)].symbol(), "X", "{mark:?}");
            assert_eq!(parts.content, Rect::new(10, 0, 1, 1), "{mark:?}");
        }
    }

    #[test]
    fn partial_flag_and_zwj_graphemes_have_no_false_content_width() {
        let system = DesignSystem::default();
        for mark in ["🇺🇸", "👩‍💻"] {
            let lockup = Lockup::new(mark, &system).compact();
            let area = Rect::new(0, 0, 1, 1);
            let mut buffer = Buffer::empty(area);

            let parts = lockup.paint(area, &mut buffer);
            assert!(parts.content.is_empty(), "{mark:?}");
            assert_eq!(buffer[(0, 0)].symbol(), " ", "{mark:?}");

            let mut scene = SemanticScene::<&str>::new();
            lockup.register_semantic(&mut scene, "wide", parts, true);
            assert!(scene.nodes().is_empty(), "{mark:?}");
        }
    }

    #[test]
    fn static_registration_uses_chrome_semantics() {
        let system = DesignSystem::default();
        let lockup = Lockup::new("mark", &system);
        let mut scene = SemanticScene::<&str>::new();

        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 1));
        let parts = lockup.paint(Rect::new(0, 0, 8, 1), &mut buffer);
        lockup.register_semantic(&mut scene, "brand", parts, false);

        let node = &scene.nodes()[0];
        assert_eq!(node.role, SemanticRole::Chrome);
        assert!(!node.focusable);
        assert!(!node.state.pressed);
    }

    #[test]
    fn explicit_pressed_state_projects_to_interactive_semantics() {
        let system = DesignSystem::default();
        let lockup = Lockup::new("mark", &system);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 1));
        let parts = lockup.paint(Rect::new(0, 0, 8, 1), &mut buffer);
        let mut scene = SemanticScene::<&str>::new();

        lockup.register_semantic_with_state(
            &mut scene,
            "brand",
            parts,
            true,
            LockupState {
                hovered: true,
                pressed: true,
            },
        );

        let node = &scene.nodes()[0];
        assert_eq!(node.role, SemanticRole::Control);
        assert!(node.state.pressed);
        assert!(!node.state.selected);
    }
}

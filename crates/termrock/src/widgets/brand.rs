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

use crate::{
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState},
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
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
        let clipped = take_display_cols(label, usize::from(max_width));
        let content_width = u16::try_from(display_cols(clipped.as_ref()))
            .unwrap_or(u16::MAX)
            .min(max_width);
        LockupParts {
            root: area,
            content: Rect::new(area.x, area.y, content_width, 1.min(area.height)),
        }
    }

    fn parts(&self, area: Rect) -> LockupParts {
        let label = self.label();
        Self::parts_for_label(area, &label)
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
        if parts.content.is_empty() {
            return parts;
        }
        let clipped = take_display_cols(&label, usize::from(parts.content.width));
        buffer.set_stringn(
            parts.content.x,
            parts.content.y,
            &clipped,
            usize::from(parts.content.width),
            self.style(state),
        );
        parts
    }

    /// Registers the painted region in the frame-local semantic scene.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        interactive: bool,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        self.register_semantic_with_state(scene, id, area, interactive, LockupState::default());
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
        area: Rect,
        interactive: bool,
        state: LockupState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        let parts = self.parts(area);
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
        lockup.register_semantic(&mut scene, "brand", Rect::new(2, 0, 8, 1), true);
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
        compact.register_semantic(&mut scene, "wide", area, true);
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
        lockup.register_semantic(&mut scene, "ascii", area, true);
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
        lockup.register_semantic(&mut scene, "zero", zero_width, true);
        lockup.register_semantic(&mut scene, "short", zero_height, false);
        assert!(scene.nodes().is_empty());
    }

    #[test]
    fn static_registration_uses_chrome_semantics() {
        let system = DesignSystem::default();
        let lockup = Lockup::new("mark", &system);
        let mut scene = SemanticScene::<&str>::new();

        lockup.register_semantic(&mut scene, "brand", Rect::new(0, 0, 8, 1), false);

        let node = &scene.nodes()[0];
        assert_eq!(node.role, SemanticRole::Chrome);
        assert!(!node.focusable);
        assert!(!node.state.pressed);
    }

    #[test]
    fn explicit_pressed_state_projects_to_interactive_semantics() {
        let system = DesignSystem::default();
        let lockup = Lockup::new("mark", &system);
        let mut scene = SemanticScene::<&str>::new();

        lockup.register_semantic_with_state(
            &mut scene,
            "brand",
            Rect::new(0, 0, 8, 1),
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

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::Text,
    widgets::{StatefulWidget, Widget},
};
use ratatui_widgets::{clear::Clear, paragraph::Paragraph};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    interaction::{
        HitRegion, NavigationMove, Outcome, OverlayId, OverlayKind, OverlayOutcome, OverlayPolicy,
        OverlaySize, OverlaySpec, OverlayStack, UiIntent, place_overlay,
    },
    style::{Density, DesignSystem, Role, RolePalette},
};

use super::{
    Action, ActionBar, ActionBarState, DetailRow, DetailTable, DetailTableState, Panel, PanelChrome,
};

/// Default overlay id for a modal dialog on an [`OverlayStack`].
pub const DIALOG_OVERLAY_ID: &str = "termrock.dialog";

/// Preferred dialog size before clamp / narrow promotion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogSize {
    /// Preferred width in cells.
    pub width: u16,
    /// Preferred height in rows.
    pub height: u16,
}

impl Default for DialogSize {
    fn default() -> Self {
        Self::for_density(Density::Comfortable)
    }
}

impl DialogSize {
    /// Preferred size for a density mode (cells).
    #[must_use]
    pub const fn for_density(density: Density) -> Self {
        match density {
            Density::Comfortable => Self {
                width: 48,
                height: 12,
            },
            Density::Compact => Self {
                width: 40,
                height: 10,
            },
            Density::Dashboard => Self {
                width: 36,
                height: 8,
            },
        }
    }

    /// Minimum usable width before fullscreen promotion (policy elsewhere).
    #[must_use]
    pub const fn min_width(density: Density) -> u16 {
        match density {
            Density::Comfortable => 40,
            Density::Compact => 36,
            Density::Dashboard => 32,
        }
    }
}

/// Visual / semantic dialog chrome variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DialogVariant {
    /// Neutral elevated dialog.
    #[default]
    Default,
    /// Destructive / risk surface (`PanelChrome::Danger`).
    Danger,
    /// Informational emphasis (focused border, info title tone).
    Info,
}

impl From<DialogSize> for OverlaySize {
    fn from(value: DialogSize) -> Self {
        OverlaySize::dialog(value.width, value.height)
    }
}

/// Centered dialog rectangle using [`OverlayKind::Dialog`] policy.
#[must_use]
pub fn place_dialog(bounds: Rect, preferred: DialogSize) -> Rect {
    if bounds.is_empty() || preferred.width == 0 || preferred.height == 0 {
        return Rect::default();
    }
    place_overlay(
        bounds,
        None,
        OverlaySize::from(preferred),
        OverlayPolicy::for_kind(OverlayKind::Dialog),
    )
}

/// Opens (or replaces) a dismissible dialog overlay.
pub fn open_dialog_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    preferred: DialogSize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::dialog(
            DIALOG_OVERLAY_ID,
            OverlaySize::from(preferred),
            opener_focus,
        ),
    )
}

/// Opens an alert dialog that traps Esc until an explicit action.
pub fn open_alert_dialog_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    preferred: DialogSize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::alert_dialog(
            DIALOG_OVERLAY_ID,
            OverlaySize::from(preferred),
            opener_focus,
        ),
    )
}

/// Dismisses the default dialog overlay when present.
pub fn dismiss_dialog_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(DIALOG_OVERLAY_ID))
}

#[derive(Debug, Clone, Copy)]
/// A themed fill painted behind modal content.
pub struct Backdrop {
    symbol: char,
    style: Style,
}

impl Default for Backdrop {
    fn default() -> Self {
        Self {
            symbol: ' ',
            style: Style::new()
                .fg(Color::Reset)
                .bg(crate::style::DIALOG_BACKDROP),
        }
    }
}

impl Backdrop {
    #[must_use]
    /// Creates a fully opaque backdrop from a semantic theme.
    pub fn new() -> Self {
        Self::default()
    }

    /// Terminal-default background (Reset) — preferred modal scrim.
    #[must_use]
    pub fn reset() -> Self {
        Self::default()
    }

    /// Optional dim wash using a glyph field (ASCII-safe `.` fallback).
    #[must_use]
    pub fn dim_wash(ascii: bool) -> Self {
        Self {
            symbol: if ascii { '.' } else { '░' },
            style: Style::new()
                .fg(Color::DarkGray)
                .bg(crate::style::DIALOG_BACKDROP)
                .add_modifier(ratatui_core::style::Modifier::DIM),
        }
    }

    /// Resolves backdrop from design tokens (Reset by default; no hard black).
    #[must_use]
    pub fn from_tokens(tokens: &DesignSystem) -> Self {
        let _ = tokens;
        Self::reset()
    }

    #[must_use]
    /// Sets the fill symbol used across the backdrop.
    pub const fn symbol(mut self, symbol: char) -> Self {
        self.symbol = symbol;
        self
    }

    #[must_use]
    /// Sets the style used to fill the backdrop.
    pub const fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl Widget for &Backdrop {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buffer[(x, y)].set_char(self.symbol).set_style(self.style);
            }
        }
    }
}

impl Widget for Backdrop {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod backdrop_tests {
    use super::*;
    use ratatui_core::{layout::Position, widgets::StatefulWidget};

    use crate::{
        input::KeyModifiers,
        interaction::{OverlayId, OverlayKind, OverlayOutcome, OverlayStack},
    };

    #[test]
    fn default_backdrop_uses_terminal_background() {
        let backdrop = Backdrop::default();
        assert_eq!(backdrop.symbol, ' ');
        assert_eq!(backdrop.style.fg, Some(Color::Reset));
        assert_eq!(backdrop.style.bg, Some(Color::Reset));
    }

    #[test]
    fn dialog_opens_on_overlay_stack_with_opener_restore() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        let out = open_dialog_overlay(
            &mut stack,
            bounds,
            DialogSize {
                width: 40,
                height: 10,
            },
            Some("trigger"),
        );
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Dialog);
        let placed = place_dialog(
            bounds,
            DialogSize {
                width: 40,
                height: 10,
            },
        );
        assert_eq!(stack.top().unwrap().rect, placed);
        // Outside click is trapped for dialogs
        assert_eq!(
            stack.handle_outside_click(Position::new(0, 0)),
            OverlayOutcome::Ignored
        );
        assert!(matches!(
            stack.handle_escape(),
            OverlayOutcome::Dismissed {
                focus: Some("trigger"),
                ..
            }
        ));
    }

    #[test]
    fn alert_dialog_traps_escape() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<()>::new();
        let _ = open_alert_dialog_overlay(&mut stack, bounds, DialogSize::default(), None);
        assert_eq!(stack.handle_escape(), OverlayOutcome::Ignored);
        assert!(stack.contains(&OverlayId::from_static(DIALOG_OVERLAY_ID)));
        let _ = dismiss_dialog_overlay(&mut stack);
        assert!(stack.is_empty());
    }

    #[test]
    fn dialog_narrow_promotes_fullscreen() {
        let bounds = Rect::new(0, 0, 32, 10);
        let mut stack = OverlayStack::<()>::new();
        let _ = open_dialog_overlay(&mut stack, bounds, DialogSize::default(), None);
        assert!(stack.top().unwrap().fullscreen_promoted);
        assert_eq!(stack.top().unwrap().rect, bounds);
    }

    #[test]
    fn choice_dialog_skips_disabled_actions_and_returns_semantic_outcomes() {
        let actions = [
            Action {
                id: "accept",
                label: "Accept",
                enabled: true,
                style: None,
            },
            Action {
                id: "blocked",
                label: "Blocked",
                enabled: false,
                style: None,
            },
            Action {
                id: "cancel",
                label: "Cancel",
                enabled: true,
                style: None,
            },
        ];
        let mut state = ChoiceDialogState::new(Some("accept"));
        assert_eq!(
            state.handle_intent(&actions, UiIntent::Move(NavigationMove::Next)),
            Outcome::Changed
        );
        assert_eq!(state.cursor, Some("cancel"));
        assert_eq!(
            state.handle_intent(&actions, UiIntent::Activate),
            Outcome::Activated("cancel")
        );
        assert_eq!(
            state.handle_intent(&actions, UiIntent::Cancel),
            Outcome::Cancelled
        );
        assert_eq!(
            state.handle_key(&actions, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Outcome::Cancelled
        );
        // Tab is host/scene-owned — not local cursor.
        assert_eq!(
            state.handle_key(&actions, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            Outcome::Ignored
        );
    }

    #[test]
    fn choice_dialog_accepts_input_gate() {
        let actions = [Action {
            id: "ok",
            label: "OK",
            enabled: true,
            style: None,
        }];
        let mut state = ChoiceDialogState::new(Some("ok"));
        state.set_accepts_input(false);
        assert_eq!(
            state.handle_intent(&actions, UiIntent::Activate),
            Outcome::Ignored
        );
    }

    #[test]
    fn choice_dialog_narrow_stacks_actions() {
        let actions = [
            Action {
                id: "a",
                label: "Accept",
                enabled: true,
                style: None,
            },
            Action {
                id: "c",
                label: "Cancel",
                enabled: true,
                style: None,
            },
        ];
        let system = DesignSystem::default();
        let dialog = ChoiceDialog::new(
            Dialog::new("Choose", Text::from("?"), &system).emphasis(PanelChrome::Focused),
            &actions,
        )
        .ascii(true);
        let area = Rect::new(0, 0, 22, 8);
        let mut buffer = Buffer::empty(area);
        let mut state = ChoiceDialogState::new(Some("a"));
        (&dialog).render(area, &mut buffer, &mut state);
        assert!(state.regions.len() >= 1);
        let text: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(
            text.contains("Accept") || text.contains("[Accept]"),
            "{text:?}"
        );
    }

    #[test]
    fn choice_dialog_mouse_outcomes_follow_enabled_painted_regions() {
        let actions = [Action {
            id: "accept",
            label: "Accept",
            enabled: true,
            style: None,
        }];
        let tokens = DesignSystem::default();
        let dialog = ChoiceDialog::new(
            Dialog::new("Choose", Text::from("Continue?"), &tokens).emphasis(PanelChrome::Focused),
            &actions,
        );
        let area = Rect::new(3, 2, 30, 6);
        let mut buffer = Buffer::empty(area);
        let mut state = ChoiceDialogState::default();
        (&dialog).render(area, &mut buffer, &mut state);
        assert_eq!(state.regions.len(), 1);
        let region = state.regions[0].area;
        assert_eq!(
            state.click(Position::new(region.x, region.y)),
            Outcome::Activated("accept")
        );
    }

    #[test]
    fn message_details_start_after_wrapped_body() {
        let details = [DetailRow {
            id: "stage",
            label: "Stage",
            value: "Build",
            href: None,
            capability: super::super::DetailCapability::None,
            emphasis: false,
            style: None,
        }];
        let tokens = DesignSystem::default();
        let dialog = MessageDialog::new(
            Dialog::new("Failure", Text::from("a message that wraps"), &tokens)
                .emphasis(PanelChrome::Focused),
            &details,
            &tokens,
        )
        .wrap(true);
        let area = Rect::new(0, 0, 12, 8);
        let mut buffer = Buffer::empty(area);
        let mut state = DetailTableState::default();
        (&dialog).render(area, &mut buffer, &mut state);
        assert_eq!(state.viewport.y, 3);
    }

    #[test]
    fn dialog_uses_semantic_focused_panel_chrome() {
        let tokens = DesignSystem::default();
        let dialog =
            Dialog::new(" Notice ", Text::from("Done"), &tokens).emphasis(PanelChrome::Focused);
        let area = Rect::new(0, 0, 18, 4);
        let mut buffer = Buffer::empty(area);
        (&dialog).render(area, &mut buffer);

        assert_eq!(
            buffer[(0, 0)].fg,
            tokens.style(crate::style::Role::BorderFocused).fg.unwrap()
        );
        assert!(
            buffer
                .content()
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
                .contains(" Notice ")
        );
    }

    #[test]
    fn danger_variant_uses_danger_border_and_title_cue() {
        let tokens = DesignSystem::default();
        let dialog = Dialog::new("Delete", Text::from("Irreversible"), &tokens)
            .variant(DialogVariant::Danger);
        let area = Rect::new(0, 0, 24, 5);
        let mut buffer = Buffer::empty(area);
        (&dialog).render(area, &mut buffer);
        assert_eq!(buffer[(0, 0)].fg, tokens.style(Role::Danger).fg.unwrap());
        let text: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains('!') || text.contains("Delete"), "{text:?}");
    }

    #[test]
    fn loading_disables_choice_activation() {
        let actions = [Action {
            id: "ok",
            label: "OK",
            enabled: true,
            style: None,
        }];
        let mut state = ChoiceDialogState::new(Some("ok"));
        state.set_loading(true);
        assert_eq!(
            state.handle_intent(&actions, UiIntent::Activate),
            Outcome::Ignored
        );
        state.set_loading(false);
        assert_eq!(
            state.handle_intent(&actions, UiIntent::Activate),
            Outcome::Activated("ok")
        );
    }

    #[test]
    fn empty_body_and_from_system() {
        let system = DesignSystem::phosphor();
        let dialog =
            Dialog::from_system("Empty", Text::default(), &system).footer_hint("esc dismiss");
        let area = Rect::new(0, 0, 28, 6);
        let mut buffer = Buffer::empty(area);
        (&dialog).render(area, &mut buffer);
        let text: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains("Empty"), "{text:?}");
        assert!(text.contains("esc") || text.contains("dismiss"), "{text:?}");
    }

    #[test]
    fn dim_wash_backdrop_is_not_hard_black() {
        let wash = Backdrop::dim_wash(false);
        assert_ne!(wash.symbol, '\0');
        assert_eq!(wash.style.bg, Some(Color::Reset));
    }

    #[test]
    fn dialog_size_tracks_density() {
        assert!(
            DialogSize::for_density(Density::Comfortable).width
                >= DialogSize::for_density(Density::Dashboard).width
        );
        assert!(DialogSize::min_width(Density::Comfortable) >= 32);
    }
}

#[derive(Debug, Clone)]
/// A framed modal surface with resolved geometry.
///
/// Anatomy: frame (Panel) · title · body · optional footer hint · loading cue.
/// Open/close and Esc trap live on [`OverlayStack`]; this widget is pure paint +
/// geometry for the modal rect.
pub struct Dialog<'a> {
    title: &'a str,
    body: Text<'a>,
    style: Style,
    tokens: &'a DesignSystem,
    emphasis: PanelChrome,
    variant: DialogVariant,
    footer_hint: Option<&'a str>,
    loading: bool,
}

impl<'a> Dialog<'a> {
    #[must_use]
    /// Creates a dialog painted from design tokens / recipes.
    pub const fn new(title: &'a str, body: Text<'a>, tokens: &'a DesignSystem) -> Self {
        Self {
            title,
            body,
            style: Style::new(),
            tokens,
            emphasis: PanelChrome::Normal,
            variant: DialogVariant::Default,
            footer_hint: None,
            loading: false,
        }
    }

    /// Preferred constructor from [`DesignSystem`].
    #[must_use]
    pub const fn from_system(title: &'a str, body: Text<'a>, system: &'a DesignSystem) -> Self {
        Self::new(title, body, system)
    }

    /// Theme borrow for child widgets that still take `&RolePalette`.
    #[must_use]
    pub const fn theme(&self) -> &RolePalette {
        self.tokens.palette()
    }

    /// Design tokens used for panel recipes and density.
    #[must_use]
    pub const fn tokens(&self) -> &DesignSystem {
        self.tokens
    }

    #[must_use]
    /// Overrides the theme-derived dialog body style.
    pub const fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    /// Sets the semantic panel emphasis (overridden by danger variant).
    pub const fn emphasis(mut self, emphasis: PanelChrome) -> Self {
        self.emphasis = emphasis;
        self
    }

    /// Sets chrome variant (default / danger / info).
    #[must_use]
    pub const fn variant(mut self, variant: DialogVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Optional footer hint row (keymap help); dropped on tiny heights.
    #[must_use]
    pub const fn footer_hint(mut self, hint: &'a str) -> Self {
        self.footer_hint = Some(hint);
        self
    }

    /// Loading chrome (title busy glyph); consumers disable actions separately.
    #[must_use]
    pub const fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    fn resolved_emphasis(&self) -> PanelChrome {
        match self.variant {
            DialogVariant::Danger => PanelChrome::Danger,
            DialogVariant::Default | DialogVariant::Info => self.emphasis,
        }
    }

    fn title_for_paint(&self) -> String {
        let mut title = self.title.to_string();
        if matches!(self.variant, DialogVariant::Danger) && !title.contains('!') {
            title = format!("! {title}");
        }
        if self.loading {
            let glyph = self.tokens.glyphs.loading();
            title = format!("{title} {glyph}");
        }
        title
    }
}
impl Widget for &Dialog<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        Clear.render(area, buffer);
        let emphasis = self.resolved_emphasis();
        let title = self.title_for_paint();
        let panel = Panel::new(self.tokens)
            .title(title.as_str())
            .emphasis(emphasis);
        let mut body_style = self.style;
        if body_style.fg.is_none() {
            body_style = body_style.patch(self.tokens.style(Role::Text));
        }
        // Tiny: border + title only.
        if area.height < 3 {
            panel.block().render(area, buffer);
            return;
        }
        let footer_rows = u16::from(self.footer_hint.is_some() && area.height >= 5);
        let body_area = if footer_rows > 0 {
            Rect::new(
                area.x,
                area.y,
                area.width,
                area.height.saturating_sub(footer_rows),
            )
        } else {
            area
        };
        Paragraph::new(self.body.clone())
            .block(panel.block())
            .style(body_style)
            .render(body_area, buffer);
        if let Some(hint) = self.footer_hint
            && footer_rows > 0
        {
            let y = area.bottom().saturating_sub(2);
            let x = area.x.saturating_add(1);
            let w = area.width.saturating_sub(2);
            let style = self.tokens.style(Role::TextMuted);
            buffer.set_stringn(x, y, hint, usize::from(w), style);
        }
    }
}

impl Widget for Dialog<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Runtime state for `ChoiceDialog`.
///
/// **Action cursor** is bar-local. Scene/overlay surface focus is host-owned;
/// hosts may project scene focus into [`Self::cursor`] each frame (lookbook trap).
pub struct ChoiceDialogState<Id> {
    /// Action cursor (not scene surface focus).
    pub cursor: Option<Id>,
    /// Hit regions produced by the most recent render.
    pub regions: Vec<HitRegion<Id>>,
    /// When true, activation is ignored (async confirm in progress).
    loading: bool,
    /// Host grants keyboard/pointer into this dialog.
    accepts_input: bool,
}

impl<Id> Default for ChoiceDialogState<Id> {
    fn default() -> Self {
        Self {
            cursor: None,
            regions: Vec::new(),
            loading: false,
            accepts_input: true,
        }
    }
}

impl<Id: Clone + PartialEq> ChoiceDialogState<Id> {
    #[must_use]
    /// Creates choice-dialog state with optional initial action cursor.
    pub const fn new(cursor: Option<Id>) -> Self {
        Self {
            cursor,
            regions: Vec::new(),
            loading: false,
            accepts_input: true,
        }
    }

    /// Action cursor id.
    #[must_use]
    pub fn cursor(&self) -> Option<&Id> {
        self.cursor.as_ref()
    }

    /// Deprecated name for [`Self::cursor`].
    #[deprecated(note = "use cursor")]
    #[must_use]
    pub fn focused(&self) -> Option<&Id> {
        self.cursor.as_ref()
    }

    /// Host input gate (overlay top / scene ownership).
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Whether host granted input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Marks the dialog as waiting on an async action (blocks activation).
    pub const fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    #[must_use]
    /// Returns whether activation is suppressed.
    pub const fn is_loading(&self) -> bool {
        self.loading
    }

    /// Routes cancellation, cyclic action cursor, and activation keys.
    ///
    /// Prefer [`Self::handle_intent`] / [`crate::interaction::default_choice_dialog_intent`].
    /// **Tab is host/scene owned** when actions are registered on InteractionScene —
    /// default intent maps Left/Right (not Tab) for local cursor.
    pub fn handle_key(&mut self, actions: &[Action<'_, Id>], key: KeyEvent) -> Outcome<Id> {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return Outcome::Ignored;
        }
        if let Some(intent) = crate::interaction::default_choice_dialog_intent(key) {
            return self.handle_intent(actions, intent);
        }
        Outcome::Ignored
    }

    /// Semantic intent routing for footer actions.
    pub fn handle_intent(&mut self, actions: &[Action<'_, Id>], intent: UiIntent) -> Outcome<Id> {
        if !self.accepts_input {
            return Outcome::Ignored;
        }
        if self.loading
            && matches!(
                intent,
                UiIntent::Activate | UiIntent::Submit | UiIntent::Open
            )
        {
            return Outcome::Ignored;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close => Outcome::Cancelled,
            UiIntent::Activate | UiIntent::Submit | UiIntent::Open => {
                self.activate_selected(actions)
            }
            UiIntent::Move(NavigationMove::Previous) => self.select_relative(actions, -1),
            UiIntent::Move(NavigationMove::Next) => self.select_relative(actions, 1),
            UiIntent::Move(NavigationMove::First) => {
                let enabled: Vec<_> = actions.iter().filter(|a| a.enabled).collect();
                enabled.first().map_or(Outcome::Ignored, |a| {
                    if self.cursor.as_ref() == Some(&a.id) {
                        return Outcome::Ignored;
                    }
                    self.cursor = Some(a.id.clone());
                    Outcome::Changed
                })
            }
            UiIntent::Move(NavigationMove::Last) => {
                let enabled: Vec<_> = actions.iter().filter(|a| a.enabled).collect();
                enabled.last().map_or(Outcome::Ignored, |a| {
                    if self.cursor.as_ref() == Some(&a.id) {
                        return Outcome::Ignored;
                    }
                    self.cursor = Some(a.id.clone());
                    Outcome::Changed
                })
            }
            _ => Outcome::Ignored,
        }
    }

    /// Moves selection to the next enabled item, wrapping at the end.
    pub fn select_next(&mut self, actions: &[Action<'_, Id>]) -> Outcome<Id> {
        self.select_relative(actions, 1)
    }

    /// Moves selection to the previous enabled item, wrapping at the start.
    pub fn select_previous(&mut self, actions: &[Action<'_, Id>]) -> Outcome<Id> {
        self.select_relative(actions, -1)
    }

    fn select_relative(&mut self, actions: &[Action<'_, Id>], direction: isize) -> Outcome<Id> {
        let enabled: Vec<&Action<'_, Id>> =
            actions.iter().filter(|action| action.enabled).collect();
        if enabled.is_empty() {
            self.cursor = None;
            return Outcome::Ignored;
        }
        let current = self
            .cursor
            .as_ref()
            .and_then(|cur| enabled.iter().position(|action| &action.id == cur));
        let next = match (current, direction.is_negative()) {
            (Some(0), true) | (None, true) => enabled.len() - 1,
            (Some(index), true) => index - 1,
            (Some(index), false) => (index + 1) % enabled.len(),
            (None, false) => 0,
        };
        let id = enabled[next].id.clone();
        if self.cursor.as_ref() == Some(&id) {
            return Outcome::Ignored;
        }
        self.cursor = Some(id);
        Outcome::Changed
    }

    #[must_use]
    /// Returns the semantic outcome for the currently selected item.
    pub fn activate_selected(&self, actions: &[Action<'_, Id>]) -> Outcome<Id> {
        if self.loading || !self.accepts_input {
            return Outcome::Ignored;
        }
        self.cursor
            .as_ref()
            .and_then(|cur| {
                actions
                    .iter()
                    .find(|action| action.enabled && &action.id == cur)
            })
            .map_or(Outcome::Ignored, |action| {
                Outcome::Activated(action.id.clone())
            })
    }

    #[must_use]
    /// Maps a pointer position to the semantic outcome of the painted hit region.
    ///
    /// Click always activates the hit action (dialog one-shot). Hosts that want
    /// scene focus first should route pointer through InteractionScene.
    pub fn click(&mut self, position: ratatui_core::layout::Position) -> Outcome<Id> {
        if self.loading || !self.accepts_input {
            return Outcome::Ignored;
        }
        let Some(region) = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
        else {
            return Outcome::Ignored;
        };
        self.cursor = Some(region.id.clone());
        Outcome::Activated(region.id.clone())
    }
}

#[derive(Debug, Clone)]
/// A modal choice prompt with stable action identities.
pub struct ChoiceDialog<'a, Id> {
    dialog: Dialog<'a>,
    actions: &'a [Action<'a, Id>],
    gap: &'a str,
    ascii: bool,
    colorless: bool,
}

impl<'a, Id> ChoiceDialog<'a, Id> {
    #[must_use]
    /// Creates a choice dialog over borrowed actions and mutable state.
    pub const fn new(dialog: Dialog<'a>, actions: &'a [Action<'a, Id>]) -> Self {
        Self {
            dialog,
            actions,
            gap: " ",
            ascii: false,
            colorless: false,
        }
    }

    #[must_use]
    /// Sets spacing between adjacent items in terminal cells.
    pub const fn gap(mut self, gap: &'a str) -> Self {
        self.gap = gap;
        self
    }

    /// ASCII action cursor marks.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Reduced-color action paint.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &ChoiceDialog<'_, Id> {
    type State = ChoiceDialogState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        (&self.dialog).render(area, buffer);
        if area.height < 3 {
            state.regions.clear();
            return;
        }
        let narrow = area.width < 28;
        let action_rows = if narrow {
            (self.actions.len() as u16)
                .min(area.height.saturating_sub(3))
                .max(1)
        } else {
            1
        };
        let action_area = Rect::new(
            area.x.saturating_add(1),
            area.bottom().saturating_sub(action_rows.saturating_add(1)),
            area.width.saturating_sub(2),
            action_rows,
        );
        let mut action_state = ActionBarState {
            cursor: state.cursor.clone(),
            regions: Vec::new(),
        };
        (&ActionBar::new(self.actions, self.dialog.tokens())
            .gap(self.gap)
            .ascii(self.ascii)
            .colorless(self.colorless)
            .vertical(narrow && action_rows > 1))
            .render(action_area, buffer, &mut action_state);
        // Do not clobber host-projected cursor from paint; only take regions.
        // If paint discovers empty cursor, keep host value.
        if state.cursor.is_none() {
            state.cursor = action_state.cursor;
        }
        state.regions = action_state.regions;
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for ChoiceDialog<'_, Id> {
    type State = ChoiceDialogState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[derive(Debug, Clone)]
/// A message dialog with optional scrollable details.
pub struct MessageDialog<'a, Id> {
    dialog: Dialog<'a>,
    details: &'a [DetailRow<'a, Id>],
    label_width: u16,
    wrap: bool,
    system: &'a DesignSystem,
}

impl<'a, Id> MessageDialog<'a, Id> {
    #[must_use]
    /// Creates a message dialog with no details and zero scroll offset.
    pub const fn new(
        dialog: Dialog<'a>,
        details: &'a [DetailRow<'a, Id>],
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            dialog,
            details,
            label_width: 0,
            wrap: false,
            system,
        }
    }

    #[must_use]
    /// Reserves a fixed label width in terminal display columns.
    pub const fn label_width(mut self, label_width: u16) -> Self {
        self.label_width = label_width;
        self
    }

    #[must_use]
    /// Sets whether long content wraps instead of scrolling horizontally.
    pub const fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &MessageDialog<'_, Id> {
    type State = DetailTableState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        (&self.dialog).render(area, buffer);
        if area.width < 3 || area.height < 3 {
            state.regions.clear();
            return;
        }
        let content_width = usize::from(area.width.saturating_sub(2)).max(1);
        let body_height = self
            .dialog
            .body
            .lines
            .iter()
            .map(|line| line.width().div_ceil(content_width).max(1))
            .sum::<usize>()
            .min(usize::from(area.height.saturating_sub(2)));
        let body_height = u16::try_from(body_height).unwrap_or(u16::MAX);
        let inner = Rect::new(
            area.x + 1,
            area.y.saturating_add(1).saturating_add(body_height),
            area.width - 2,
            area.height.saturating_sub(body_height).saturating_sub(2),
        );
        (&DetailTable::new(self.details, self.system)
            .label_width(self.label_width)
            .wrap(self.wrap))
            .render(inner, buffer, state);
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for MessageDialog<'_, Id> {
    type State = DetailTableState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

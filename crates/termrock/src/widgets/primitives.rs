// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Terminal-native activation and pure chrome primitives (Plan 050).
//!
//! **Law:** Enter/Space or one pointer gesture activates once; disabled and
//! loading never activate. Press confirms; Release never activates. Effects
//! remain consumer-owned outcomes.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::Modifier,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{HitRegion, SemanticNode, SemanticRole, SemanticScene, SemanticState},
    runtime::FrameTick,
    style::{
        ButtonRecipeVariant, ControlState, DesignSystem, Glyph, GlyphSet, MotionPolicy, Role,
        VisualState,
    },
    text::{display_cols, take_display_cols},
};

// ── Shared activation ───────────────────────────────────────────────────────

/// Typed result of an activation gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ActivationOutcome {
    /// Gesture did not activate.
    #[default]
    Ignored,
    /// Control activated once (or confirm after pending).
    Activated,
    /// First press while pending-confirmation required (await second Activate).
    ConfirmRequired,
    /// Visual press/arm state changed without activation.
    Pressed,
}

impl ActivationOutcome {
    /// Wraps in the standard [`crate::interaction::EventResult`] envelope.
    #[must_use]
    pub fn into_event_result(self) -> crate::interaction::EventResult<Self> {
        match self {
            Self::Ignored => crate::interaction::EventResult::ignored(),
            other => crate::interaction::EventResult::emit(other),
        }
    }
}

/// Armed/loading/disabled activation model shared by Button and IconButton.
///
/// **Input gate** is [`Self::set_accepts_input`] (host/scene ownership). Do not
/// treat it as a second scene focus authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ActivationState {
    enabled: bool,
    loading: bool,
    /// Host grants keyboard/pointer (overlay/scene ownership).
    accepts_input: bool,
    /// Pointer or Space is armed (await Up / release).
    armed: bool,
    /// Requires two Activate intents (destructive confirm).
    pending_confirmation: bool,
    /// First activate received; next Activate fires.
    confirm_armed: bool,
    /// Whether this terminal reports key releases.
    release_reporting: crate::input::KeyReleaseReporting,
}

impl ActivationState {
    /// Enabled, not loading, does not accept input until host grants.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            enabled: true,
            loading: false,
            accepts_input: false,
            armed: false,
            pending_confirmation: false,
            confirm_armed: false,
            release_reporting: crate::input::KeyReleaseReporting::PressOnly,
        }
    }

    /// Whether the control can activate.
    #[must_use]
    pub const fn can_activate(&self) -> bool {
        self.enabled && !self.loading && self.accepts_input
    }

    /// Host input gate without side effects (draft/state preserved elsewhere).
    pub const fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
        if !accepts {
            self.armed = false;
            self.confirm_armed = false;
        }
    }

    /// Enabled flag.
    pub const fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.armed = false;
            self.confirm_armed = false;
        }
    }

    /// Loading flag (blocks activation; distinct from disabled paint).
    pub const fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
        if loading {
            self.armed = false;
            self.confirm_armed = false;
        }
    }

    /// States whether this terminal reports key releases.
    ///
    /// Hosts that negotiate the Kitty keyboard protocol pass `Reported`, which
    /// restores press-to-arm / release-to-fire for Space.
    pub const fn set_release_reporting(&mut self, reporting: crate::input::KeyReleaseReporting) {
        self.release_reporting = reporting;
    }

    /// Whether this control waits for a key release before activating.
    #[must_use]
    pub const fn release_reporting(&self) -> crate::input::KeyReleaseReporting {
        self.release_reporting
    }

    /// Require two Activate intents (e.g. destructive).
    pub const fn set_pending_confirmation(&mut self, pending: bool) {
        self.pending_confirmation = pending;
        if !pending {
            self.confirm_armed = false;
        }
    }

    /// Whether host granted input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    #[must_use]
    /// Enabled.
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    /// Loading (not the same as disabled).
    pub const fn is_loading(&self) -> bool {
        self.loading
    }

    /// Whether pointer/Space is armed.
    #[must_use]
    pub const fn is_armed(&self) -> bool {
        self.armed
    }

    /// Awaiting second confirm.
    #[must_use]
    pub const fn is_confirm_armed(&self) -> bool {
        self.confirm_armed
    }

    fn fire_or_confirm(&mut self) -> ActivationOutcome {
        if self.pending_confirmation && !self.confirm_armed {
            self.confirm_armed = true;
            return ActivationOutcome::ConfirmRequired;
        }
        self.confirm_armed = false;
        self.armed = false;
        ActivationOutcome::Activated
    }

    /// Semantic intent routing (prefer over raw Enter/Space matching).
    pub fn handle_intent(&mut self, intent: crate::interaction::UiIntent) -> ActivationOutcome {
        if !self.can_activate() {
            return ActivationOutcome::Ignored;
        }
        match intent {
            crate::interaction::UiIntent::Activate
            | crate::interaction::UiIntent::Submit
            | crate::interaction::UiIntent::Toggle => self.fire_or_confirm(),
            _ => ActivationOutcome::Ignored,
        }
    }

    /// Keyboard activation via [`crate::interaction::default_button_intent`].
    ///
    /// Enter activates on Press. Space arms on Press and activates on Release
    /// (hold-repeat does not multi-fire).
    pub fn handle_key(&mut self, key: KeyEvent) -> ActivationOutcome {
        if !self.can_activate() {
            return ActivationOutcome::Ignored;
        }
        // Space: arm on press, fire on release — but only where releases are
        // actually reported. On an ordinary terminal that half of the
        // handshake never arrives, so Space armed the control forever and
        // never activated it (plans/021 Step 2).
        if matches!(key.code, KeyCode::Char(' ')) {
            if !self.release_reporting.can_wait_for_release() {
                return match key.kind {
                    KeyEventKind::Press => {
                        self.armed = false;
                        self.fire_or_confirm()
                    }
                    KeyEventKind::Repeat | KeyEventKind::Release => ActivationOutcome::Ignored,
                };
            }
            match key.kind {
                KeyEventKind::Press => {
                    self.armed = true;
                    return ActivationOutcome::Pressed;
                }
                KeyEventKind::Release if self.armed => {
                    self.armed = false;
                    return self.fire_or_confirm();
                }
                KeyEventKind::Repeat | KeyEventKind::Release => {
                    return ActivationOutcome::Ignored;
                }
            }
        }
        if key.kind == KeyEventKind::Release || key.kind == KeyEventKind::Repeat {
            return ActivationOutcome::Ignored;
        }
        if let Some(intent) = crate::interaction::default_button_intent(key) {
            return self.handle_intent(intent);
        }
        ActivationOutcome::Ignored
    }

    /// Key path with [`crate::interaction::EventResult`].
    pub fn handle_key_result(
        &mut self,
        key: KeyEvent,
    ) -> crate::interaction::EventResult<ActivationOutcome> {
        self.handle_key(key).into_event_result()
    }

    /// Intent path with [`crate::interaction::EventResult`].
    pub fn handle_intent_result(
        &mut self,
        intent: crate::interaction::UiIntent,
    ) -> crate::interaction::EventResult<ActivationOutcome> {
        self.handle_intent(intent).into_event_result()
    }

    /// Pointer activation: Down arms; Up inside region activates once.
    pub fn handle_mouse(&mut self, event: MouseEvent, region: Option<Rect>) -> ActivationOutcome {
        if !self.can_activate() {
            return ActivationOutcome::Ignored;
        }
        let Some(area) = region else {
            self.armed = false;
            return ActivationOutcome::Ignored;
        };
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) if area.contains(event.position) => {
                self.armed = true;
                ActivationOutcome::Pressed
            }
            MouseEventKind::Up(MouseButton::Left)
                if self.armed && area.contains(event.position) =>
            {
                self.armed = false;
                self.fire_or_confirm()
            }
            MouseEventKind::Up(_) | MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                if !area.contains(event.position) {
                    self.armed = false;
                }
                ActivationOutcome::Ignored
            }
            _ => ActivationOutcome::Ignored,
        }
    }
}

// ── Button / IconButton ─────────────────────────────────────────────────────

/// Visual / semantic button chrome.
///
/// Junie kinds: primary, secondary, subtle ([`Self::Quiet`]), danger
/// ([`Self::Destructive`]). Toggle is [`crate::widgets::Toggle`], not a
/// button variant. Idle anatomy is `▎label` (gutter reserved, fg=bg when
/// unfocused); width is label+2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ButtonVariant {
    /// Strong recommended action (Accent + weight).
    Primary,
    /// Default secondary action (pad + border role when focused).
    #[default]
    Secondary,
    /// Quiet / ghost / subtle (minimal chrome; focus = bold label + gutter).
    Quiet,
    /// Destructive (Danger); must not be unsafe default focus.
    Destructive,
}

impl ButtonVariant {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Quiet => "quiet",
            Self::Destructive => "destructive",
        }
    }

    const fn recipe(self) -> ButtonRecipeVariant {
        match self {
            Self::Primary => ButtonRecipeVariant::Primary,
            Self::Destructive => ButtonRecipeVariant::Destructive,
            Self::Quiet => ButtonRecipeVariant::Quiet,
            Self::Secondary => ButtonRecipeVariant::Secondary,
        }
    }
}

/// Horizontal density of button chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ButtonSize {
    /// Tighter pad (toolbar / dense dialogs).
    Compact,
    /// Default pad.
    #[default]
    Normal,
}

impl ButtonSize {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Normal => "normal",
        }
    }

    /// junie has exactly one density: the leading pad cell **is** the `▎`
    /// gutter column and one air cell trails, so a button is `label + 2`
    /// wide. Both names resolve to it; the knob survives only because
    /// consumers outside this batch still name it.
    const fn pad_cols(self) -> usize {
        match self {
            Self::Compact | Self::Normal => 1,
        }
    }
}

/// Painted button anatomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ButtonParts {
    /// Full hit target.
    pub root: Rect,
    /// Label / body band.
    pub label: Rect,
}

/// Canonical primary action control.
///
/// **Activation law** (shared [`ActivationState`]):
/// - Enter/Submit/Toggle → activate on **Press** (Repeat ignored).
/// - Space → arm on Press, activate on **Release** (hold-repeat does not multi-fire).
/// - Pointer Left → arm on Down, activate on **Up** inside region.
/// - Disabled / loading / no `accepts_input` → never activate.
/// - Pending confirmation → first Activate yields `ConfirmRequired`.
///
/// Outcomes are pure ([`ActivationOutcome`]); effects stay consumer-owned.
/// Affordance is **role + weight + border/fill cues**, not brackets alone.
#[derive(Debug, Clone, Copy)]
pub struct Button<'a> {
    label: &'a str,
    system: &'a DesignSystem,
    variant: ButtonVariant,
    size: ButtonSize,
    leading: Option<&'a str>,
    trailing: Option<&'a str>,
    full_width: bool,
    /// Required when label is empty (icon-only).
    accessible_label: Option<&'a str>,
    colorless: bool,
    /// Plane the button sits on (dialog body, panel, popover). `None` = the
    /// chrome surface.
    container: Option<ratatui_core::style::Color>,
}

impl<'a> Button<'a> {
    /// Label + design system.
    #[must_use]
    pub const fn new(label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            label,
            system,
            variant: ButtonVariant::Secondary,
            size: ButtonSize::Normal,
            leading: None,
            trailing: None,
            full_width: false,
            accessible_label: None,
            colorless: false,
            container: None,
        }
    }

    /// Names the plane this button sits on, so a dialog button does not pin
    /// the chrome surface as its ground (junie resolvers take the container).
    #[must_use]
    pub const fn container(mut self, container: ratatui_core::style::Color) -> Self {
        self.container = Some(container);
        self
    }

    /// Variant chrome.
    #[must_use]
    pub const fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Fluent Primary.
    #[must_use]
    pub const fn as_primary(mut self) -> Self {
        self.variant = ButtonVariant::Primary;
        self
    }

    /// Fluent Secondary.
    #[must_use]
    pub const fn as_secondary(mut self) -> Self {
        self.variant = ButtonVariant::Secondary;
        self
    }

    /// Fluent Quiet.
    #[must_use]
    pub const fn as_quiet(mut self) -> Self {
        self.variant = ButtonVariant::Quiet;
        self
    }

    /// Fluent Destructive.
    #[must_use]
    pub const fn as_destructive(mut self) -> Self {
        self.variant = ButtonVariant::Destructive;
        self
    }

    /// Size / pad.
    #[must_use]
    pub const fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Compact density.
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.size = ButtonSize::Compact;
        self
    }

    /// Leading glyph string (dropped before label on narrow).
    #[must_use]
    pub const fn leading(mut self, glyph: &'a str) -> Self {
        self.leading = Some(glyph);
        self
    }

    /// Trailing glyph string (dropped first on narrow).
    #[must_use]
    pub const fn trailing(mut self, glyph: &'a str) -> Self {
        self.trailing = Some(glyph);
        self
    }

    /// Fill paint width.
    #[must_use]
    pub const fn full_width(mut self, full: bool) -> Self {
        self.full_width = full;
        self
    }

    /// Accessible name (required for empty / icon-only labels).
    #[must_use]
    pub const fn accessible_label(mut self, label: &'a str) -> Self {
        self.accessible_label = Some(label);
        self
    }

    /// ASCII chrome / loading fallback.
    #[must_use]
    /// Reduced-color paint (force non-color cues).
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Whether this variant is a safe dialog default focus candidate.
    ///
    /// **Destructive returns false** — host must not default-focus it.
    #[must_use]
    pub const fn is_safe_default_focus(self) -> bool {
        !matches!(self.variant, ButtonVariant::Destructive)
    }

    /// Semantic name for a11y / HintBar (label or accessible_label).
    #[must_use]
    pub fn a11y_name(&self) -> &str {
        if !self.label.is_empty() {
            self.label
        } else {
            self.accessible_label.unwrap_or("")
        }
    }

    /// Preferred width in cells (label + chrome + glyphs).
    ///
    /// Idle width: a busy button reserves two more cells for its spinner
    /// prefix, which [`Self::paint`] accounts for at paint time.
    #[must_use]
    pub fn preferred_width(&self) -> u16 {
        let pad = self.size.pad_cols().saturating_mul(2);
        let mut w = display_cols(self.label).saturating_add(pad);
        if let Some(g) = self.leading {
            w = w.saturating_add(display_cols(g).saturating_add(1));
        }
        if let Some(g) = self.trailing {
            w = w.saturating_add(display_cols(g).saturating_add(1));
        }
        u16::try_from(w.min(usize::from(u16::MAX))).unwrap_or(u16::MAX)
    }

    fn mono(&self) -> bool {
        self.colorless
            || matches!(
                self.system.capability,
                crate::style::ColorCapability::Monochrome
            )
    }
}

/// Stateful paint for [`Button`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ButtonState {
    /// Activation model.
    pub activation: ActivationState,
    /// Last painted hit region.
    pub region: Option<Rect>,
    /// Pointer hover (host updates via [`Self::handle_mouse`]).
    pub hovered: bool,
    /// The control owns the keyboard (host sets it; focus is never inferred
    /// from being enabled — exactly one bar in a form at a time).
    pub focused: bool,
}

impl ButtonState {
    /// Fresh state (does **not** accept input until host grants).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            activation: ActivationState::new(),
            region: None,
            hovered: false,
            focused: false,
        }
    }

    /// Key routing (intent-backed).
    pub fn handle_key(&mut self, key: KeyEvent) -> ActivationOutcome {
        self.activation.handle_key(key)
    }

    /// Intent routing.
    pub fn handle_intent(&mut self, intent: crate::interaction::UiIntent) -> ActivationOutcome {
        self.activation.handle_intent(intent)
    }

    /// Mouse: hover tracking + arm/activate against last paint.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> ActivationOutcome {
        if let Some(area) = self.region {
            match event.kind {
                MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                    self.hovered = area.contains(event.position);
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    self.hovered = area.contains(event.position);
                }
                MouseEventKind::Up(_) => {}
                _ => {}
            }
        }
        self.activation.handle_mouse(event, self.region)
    }

    /// EventResult key path.
    pub fn handle_key_result(
        &mut self,
        key: KeyEvent,
    ) -> crate::interaction::EventResult<ActivationOutcome> {
        self.activation.handle_key_result(key)
    }
}

impl Button<'_> {
    /// Paint button and update hit region on state. Returns anatomy.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut ButtonState) -> ButtonParts {
        state.region = None;
        if area.is_empty() {
            return ButtonParts::default();
        }
        // Icon-only without a11y name: refuse silent activation paint (dim + !).
        let a11y_ok = !self.label.is_empty() || self.accessible_label.is_some();
        let theme = self.system;
        let loading = state.activation.is_loading();
        let disabled = !state.activation.is_enabled();
        let surface = state.activation.accepts_input() && !disabled && !loading;
        let armed = state.activation.is_armed() || state.activation.is_confirm_armed();
        let hovered = state.hovered && surface;
        let _ = self.mono();

        // Public kinds map onto the recipe vocabulary; paint adds no extra glyphs.
        let recipe_variant = self.variant.recipe();
        // One focus owner: focus is a fact the host supplies, never something
        // a button infers from being enabled. An enabled button is idle, not
        // focused — idle weight belonged to every button in the form.
        let focused = state.focused && surface;
        let control_state = if disabled {
            ControlState::Disabled
        } else if loading {
            ControlState::Loading
        } else if armed {
            ControlState::Pressed
        } else if hovered {
            ControlState::Hovered
        } else if focused {
            ControlState::Focused
        } else {
            ControlState::Default
        };
        // N2: the caller names the plane; a dialog button must not pin the
        // chrome surface as its ground.
        let ground = self
            .container
            .unwrap_or_else(|| theme.junie_theme().surface);
        let recipe = theme.button_recipe(recipe_variant, control_state, ground);
        // M2: the recipe is the whole face. Nothing is added on top — a press
        // is the recipe's own explicit reversal, not a modifier stacked on an
        // already-resolved pair.
        let style = if !a11y_ok {
            theme.style(Role::Danger)
        } else {
            recipe.fill.patch(recipe.label)
        };

        let narrow = area.width < 12;
        let tiny = area.width < 6;
        let show_trailing = self.trailing.is_some() && !narrow && !tiny;
        let show_leading =
            self.leading.is_some() && !tiny && (area.width >= 10 || self.label.is_empty());

        // Catalog on both paths: the ASCII profile already carries the
        // degraded form, so no literal is needed to spell it out.
        let load_g = theme.glyphs.resolve(Glyph::Loading).text;
        let mut body = String::new();
        if loading {
            body.push_str(load_g);
            if !self.label.is_empty() {
                body.push(' ');
            }
        } else if show_leading {
            if let Some(g) = self.leading {
                body.push_str(g);
                body.push(' ');
            }
        }
        if tiny && !self.label.is_empty() {
            body.push_str(self.label);
        } else if self.label.is_empty() {
            if let Some(g) = self.leading {
                body.push_str(g);
            } else if !a11y_ok {
                // The refusal mark is `!`, the vocabulary's only error glyph.
                body.push('!');
            }
        } else {
            body.push_str(self.label);
        }
        if show_trailing {
            if let Some(g) = self.trailing {
                body.push(' ');
                body.push_str(g);
            }
        }
        if state.activation.is_confirm_armed() {
            body.push_str(" ?");
        }

        // Anatomy: the `▎` gutter cell leads, the body follows, pad trails.
        // The bar is the focus indicator — accent while this button owns the
        // keyboard, invisible (fg = its own ground) when it does not. The
        // column is reserved either way, so focus never shifts the label.
        let pad = self.size.pad_cols();
        let pad_s = " ".repeat(pad);
        let label = format!("{pad_s}{body}{pad_s}");
        // A busy button reserves two more cells for the spinner prefix.
        let busy_pad = u16::from(loading).saturating_mul(2);
        let paint_w = if self.full_width {
            area.width
        } else {
            area.width
                .min(self.preferred_width().saturating_add(busy_pad).max(3))
        };
        let text = take_display_cols(&label, usize::from(paint_w));
        // Full-width: paint the resolved recipe across the complete hit band
        // before writing the shorter label. This avoids an unstyled tail and
        // keeps the recipe—not a local role shortcut—as the sole owner.
        if self.full_width {
            buffer.set_style(Rect::new(area.x, area.y, area.width, 1), style);
        }
        let fill_bg = recipe.fill.bg.or(style.bg).unwrap_or(ground);
        let face = Rect::new(area.x, area.y, paint_w, 1.min(area.height));
        buffer.set_style(face, style.bg(fill_bg));
        buffer.set_stringn(
            area.x,
            area.y,
            &text,
            usize::from(paint_w),
            style.bg(fill_bg),
        );
        let theme_t = theme.junie_theme();
        let on_accent =
            matches!(recipe_variant, ButtonRecipeVariant::Primary) && !disabled && !loading;
        if paint_w > 0 {
            // Junie: col 0 is always `▎`. Unfocused, fg equals the fill so the
            // glyph is present but invisible. Use the resolver, not a guessed
            // plane — Quiet has no fill of its own and sits on `ground`.
            let visual = VisualState {
                focused,
                hovered,
                pressed: armed,
                disabled,
                busy: loading,
                ..VisualState::default()
            };
            let gutter = theme_t.gutter(visual, fill_bg, on_accent);
            let cell = &mut buffer[(area.x, area.y)];
            // junie `set_string("▎", gutter)` replaces the cell; a prior
            // bold label fill must not leak onto the bar.
            cell.reset();
            cell.set_symbol(theme.glyphs.selection_gutter());
            cell.set_style(gutter);
        }
        // Busy states activity with the accent spinner cell, never with a
        // second fill or a hue on the label.
        if loading
            && let Some((glyph, glyph_style)) = recipe.busy_glyph
            && let Some(x) = area.x.checked_add(u16::try_from(pad).unwrap_or(u16::MAX))
            && x < area.right()
        {
            buffer.set_stringn(x, area.y, glyph, 1, glyph_style);
        }
        let root = Rect {
            x: area.x,
            y: area.y,
            width: if self.full_width { area.width } else { paint_w },
            height: 1.min(area.height),
        };
        state.region = Some(root);
        ButtonParts { root, label: root }
    }

    /// Semantic registration.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &ButtonState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let name = self.a11y_name();
        let _ = scene.register(
            SemanticNode::control(id, state.region.unwrap_or(area))
                .role(SemanticRole::Button)
                .label(if name.is_empty() { "button" } else { name })
                .description(self.variant.id())
                .focusable(state.activation.accepts_input() && state.activation.is_enabled())
                .state(SemanticState {
                    selected: state.activation.accepts_input(),
                    pressed: state.activation.is_armed(),
                    busy: state.activation.is_loading(),
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for Button<'_> {
    type State = ButtonState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let _ = self.paint(area, buffer, state);
    }
}

impl StatefulWidget for &Button<'_> {
    type State = ButtonState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let _ = self.paint(area, buffer, state);
    }
}

/// Minimum pointer hit width (cells) — does **not** expand painted glyph width.
pub const ICON_BUTTON_MIN_HIT: u16 = 3;

/// Icon-button visual density / toolbar recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum IconButtonSize {
    /// Dense gutters / data rows (visual 1–2; hit still ≥ [`ICON_BUTTON_MIN_HIT`]).
    Compact,
    /// Toolbar / panel header recipe (default).
    #[default]
    Toolbar,
}

impl IconButtonSize {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Toolbar => "toolbar",
        }
    }
}

/// Painted vs hit geometry for [`IconButton`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IconButtonParts {
    /// Full hit target (includes slop; may exceed visual).
    pub root: Rect,
    /// Glyph / text face only.
    pub visual: Rect,
    /// Optional badge corner.
    pub badge: Rect,
}

/// Compact glyph action with **mandatory** accessible labeling.
///
/// Visual width stays 1–2 cells (plus optional badge); pointer hit is expanded
/// to at least [`ICON_BUTTON_MIN_HIT`] **without** stretching the painted glyph
/// (slop pads empty cells). Hosts wire [`Self::help`] into [`crate::widgets::Tooltip`]
/// / HintBar; this widget does not steal focus for tooltips.
///
/// Activation law is shared with [`Button`] via [`ActivationState`].
#[derive(Debug, Clone, Copy)]
pub struct IconButton<'a> {
    glyph: &'a str,
    /// When set and mono/ASCII profile, prefer this over `glyph`.
    ascii_glyph: Option<&'a str>,
    /// When visual width cannot fit glyph, paint this text (1–2 cells ideal).
    text_fallback: Option<&'a str>,
    accessible_label: &'a str,
    /// Tooltip / HintBar copy (defaults to accessible_label).
    help: Option<&'a str>,
    system: &'a DesignSystem,
    variant: ButtonVariant,
    size: IconButtonSize,
    /// Single-cell badge (count / status); clipped to 1–2 cols.
    badge: Option<&'a str>,
    /// Toggle affordance (pressed chrome when state.pressed).
    toggle: bool,
    colorless: bool,
    /// Plane the button sits on; `None` = the chrome surface.
    container: Option<ratatui_core::style::Color>,
}

impl<'a> IconButton<'a> {
    /// Glyph + **required** non-empty accessible name.
    ///
    /// Empty `accessible_label` is a contract violation: paint refuses silent
    /// activation and shows a danger mark.
    #[must_use]
    pub const fn new(glyph: &'a str, accessible_label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            glyph,
            ascii_glyph: None,
            text_fallback: None,
            accessible_label,
            help: None,
            system,
            variant: ButtonVariant::Quiet,
            size: IconButtonSize::Toolbar,
            badge: None,
            toggle: false,
            colorless: false,
            container: None,
        }
    }

    /// Names the plane this icon button sits on (junie resolvers take the
    /// container, so a toolbar on a panel is not pinned to the surface).
    #[must_use]
    pub const fn container(mut self, container: ratatui_core::style::Color) -> Self {
        self.container = Some(container);
        self
    }

    /// Variant (quiet / primary / destructive common).
    #[must_use]
    pub const fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Destructive recipe (never safe default focus for dialogs).
    #[must_use]
    pub const fn destructive(mut self) -> Self {
        self.variant = ButtonVariant::Destructive;
        self
    }

    /// Quiet toolbar recipe (default).
    #[must_use]
    pub const fn quiet(mut self) -> Self {
        self.variant = ButtonVariant::Quiet;
        self
    }

    /// Size / density.
    #[must_use]
    pub const fn size(mut self, size: IconButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Compact data-row recipe.
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.size = IconButtonSize::Compact;
        self
    }

    /// ASCII glyph override (also forced when design system is ASCII).
    #[must_use]
    pub const fn ascii_glyph(mut self, glyph: &'a str) -> Self {
        self.ascii_glyph = Some(glyph);
        self
    }

    /// Force ASCII path.
    #[must_use]
    /// Text fallback when glyph cannot fit (low capability / 1-col squeeze).
    pub const fn text_fallback(mut self, text: &'a str) -> Self {
        self.text_fallback = Some(text);
        self
    }

    /// Tooltip / help string (HintBar / [`crate::widgets::Tooltip`] host content).
    #[must_use]
    pub const fn help(mut self, help: &'a str) -> Self {
        self.help = Some(help);
        self
    }

    /// Corner badge (`"3"`, `"!"`, …).
    #[must_use]
    pub const fn badge(mut self, badge: &'a str) -> Self {
        self.badge = Some(badge);
        self
    }

    /// Enable toggle chrome (host sets [`IconButtonState::set_pressed`]).
    #[must_use]
    pub const fn toggle(mut self, on: bool) -> Self {
        self.toggle = on;
        self
    }

    /// Reduced-color paint.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Accessible name (always the constructor label).
    #[must_use]
    pub const fn a11y_name(&self) -> &'a str {
        self.accessible_label
    }

    /// Help / tooltip text (help or a11y name).
    #[must_use]
    pub fn help_text(&self) -> &'a str {
        self.help.unwrap_or(self.accessible_label)
    }

    /// Whether label contract is satisfied.
    #[must_use]
    pub const fn has_accessible_label(&self) -> bool {
        !self.accessible_label.is_empty()
    }

    /// Safe default focus (destructive → false).
    #[must_use]
    pub const fn is_safe_default_focus(self) -> bool {
        !matches!(self.variant, ButtonVariant::Destructive)
    }

    /// Preferred **visual** width (glyph + optional badge), not hit slop.
    #[must_use]
    pub fn preferred_visual_width(&self) -> u16 {
        let face = display_cols(self.face_glyph());
        let badge = self.badge.map(|b| display_cols(b).min(2)).unwrap_or(0);
        // Badge overlays corner; visual footprint stays max(face, 2) when badge
        let w = if badge > 0 {
            face.max(2).max(badge)
        } else {
            face.max(1)
        };
        u16::try_from(w.min(4)).unwrap_or(2)
    }

    /// Minimum hit width for pointer (slop).
    #[must_use]
    pub const fn min_hit_width(&self) -> u16 {
        ICON_BUTTON_MIN_HIT
    }

    fn mono(&self) -> bool {
        self.colorless
            || matches!(
                self.system.capability,
                crate::style::ColorCapability::Monochrome
            )
    }

    fn face_glyph(&self) -> &str {
        if self.mono() {
            if let Some(a) = self.ascii_glyph {
                return a;
            }
        }
        self.glyph
    }

    fn paint_face(&self, max_cols: usize) -> String {
        let g = self.face_glyph();
        let gw = display_cols(g);
        if gw <= max_cols && gw > 0 {
            return take_display_cols(g, max_cols);
        }
        if let Some(fb) = self.text_fallback {
            return take_display_cols(fb, max_cols.max(1));
        }
        // First char of a11y name as last-resort fallback
        let ch = self
            .accessible_label
            .chars()
            .next()
            .unwrap_or('?')
            .to_ascii_uppercase();
        take_display_cols(&ch.to_string(), max_cols.max(1))
    }
}

/// State for [`IconButton`] (activation + toggle + hit slop).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IconButtonState {
    /// Shared activation law with [`Button`].
    pub activation: ActivationState,
    /// Visual paint region (glyph).
    pub visual: Option<Rect>,
    /// Pointer hit region (≥ visual; includes slop).
    pub hit: Option<Rect>,
    /// Hover.
    pub hovered: bool,
    /// Toggle pressed (only meaningful when button is toggle).
    pub pressed: bool,
    /// The control owns the keyboard (host sets it).
    pub focused: bool,
}

impl IconButtonState {
    /// Fresh (no input until host grants).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            activation: ActivationState::new(),
            visual: None,
            hit: None,
            hovered: false,
            pressed: false,
            focused: false,
        }
    }

    /// Toggle pressed visual.
    pub const fn set_pressed(&mut self, on: bool) {
        self.pressed = on;
    }

    /// Compat: region = hit (for [`button_hit`] / older hosts).
    #[must_use]
    pub const fn region(&self) -> Option<Rect> {
        self.hit
    }

    /// Key routing.
    pub fn handle_key(&mut self, key: KeyEvent) -> ActivationOutcome {
        let out = self.activation.handle_key(key);
        if matches!(out, ActivationOutcome::Activated) {
            // Host may flip toggle; we do not auto-toggle (caller owns domain).
        }
        out
    }

    /// Intent routing.
    pub fn handle_intent(&mut self, intent: crate::interaction::UiIntent) -> ActivationOutcome {
        self.activation.handle_intent(intent)
    }

    /// Mouse against **hit** region (slop-aware).
    pub fn handle_mouse(&mut self, event: MouseEvent) -> ActivationOutcome {
        if let Some(area) = self.hit {
            match event.kind {
                MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                    self.hovered = area.contains(event.position);
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    self.hovered = area.contains(event.position);
                }
                _ => {}
            }
        }
        self.activation.handle_mouse(event, self.hit)
    }

    /// EventResult key path.
    pub fn handle_key_result(
        &mut self,
        key: KeyEvent,
    ) -> crate::interaction::EventResult<ActivationOutcome> {
        self.activation.handle_key_result(key)
    }
}

impl<'a> IconButton<'a> {
    /// Paint; visual centered in area, hit expanded with slop.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut IconButtonState,
    ) -> IconButtonParts {
        state.visual = None;
        state.hit = None;
        if area.is_empty() {
            return IconButtonParts::default();
        }

        let a11y_ok = self.has_accessible_label();
        let mono = self.mono();
        let loading = state.activation.is_loading();
        let disabled = !state.activation.is_enabled();
        let surface = state.activation.accepts_input() && !disabled && !loading;
        let armed = state.activation.is_armed() || state.activation.is_confirm_armed();
        let toggled = self.toggle && state.pressed;

        // Visual size: prefer 1–2 cells for face
        let face_budget = match self.size {
            IconButtonSize::Compact => 1usize.max(display_cols(self.face_glyph()).min(2)),
            IconButtonSize::Toolbar => display_cols(self.face_glyph()).clamp(1, 2),
        };
        let face = if loading {
            if mono {
                Glyph::Loading.resolve().text.to_string()
            } else {
                self.system.glyphs.resolve(Glyph::Loading).text.to_string()
            }
        } else if !a11y_ok {
            "!".into()
        } else {
            self.paint_face(face_budget)
        };
        let face_w = u16::try_from(display_cols(&face).max(1)).unwrap_or(1);

        // Hit width: max(area, min hit, visual) but paint only face_w centered
        let hit_w = area.width.min(
            ICON_BUTTON_MIN_HIT
                .max(face_w)
                .max(if self.badge.is_some() {
                    face_w.saturating_add(1)
                } else {
                    face_w
                }),
        );
        let hit = Rect {
            x: area.x,
            y: area.y,
            width: hit_w.min(area.width),
            height: 1.min(area.height),
        };
        // Center visual in hit
        let vx = hit.x.saturating_add(hit.width.saturating_sub(face_w) / 2);
        let visual = Rect {
            x: vx,
            y: hit.y,
            width: face_w.min(hit.width),
            height: 1,
        };

        let icon_state = if disabled {
            ControlState::Disabled
        } else if loading {
            ControlState::Loading
        } else if armed {
            ControlState::Pressed
        } else if state.hovered {
            ControlState::Hovered
        } else if state.focused && surface {
            ControlState::Focused
        } else {
            ControlState::Default
        };
        let ground = self
            .container
            .unwrap_or_else(|| self.system.junie_theme().surface);
        let recipe = self
            .system
            .button_recipe(self.variant.recipe(), icon_state, ground);
        // The recipe is the whole face: a press is the recipe's own explicit
        // reversal, a latch speaks through weight, and nothing is stacked on.
        let mut style = if !a11y_ok {
            self.system.style(Role::Danger)
        } else {
            recipe.fill.patch(recipe.label)
        };
        if toggled {
            style = style.add_modifier(Modifier::BOLD);
        }

        buffer.set_stringn(visual.x, visual.y, &face, usize::from(visual.width), style);

        // Badge: top-right of hit (overlay last cell)
        let mut badge_rect = Rect::default();
        if let Some(b) = self.badge {
            if hit.width >= 2 && !b.is_empty() {
                let bt = take_display_cols(b, 1);
                let bx = hit.x.saturating_add(hit.width.saturating_sub(1));
                let mut bs = self.system.style(Role::Danger);
                if mono {
                    bs = self
                        .system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD);
                }
                bs = ratatui_core::style::Style { bg: None, ..bs };
                buffer.set_stringn(bx, hit.y, &bt, 1, bs);
                badge_rect = Rect {
                    x: bx,
                    y: hit.y,
                    width: 1,
                    height: 1,
                };
            }
        }

        state.visual = Some(visual);
        state.hit = Some(hit);
        IconButtonParts {
            root: hit,
            visual,
            badge: badge_rect,
        }
    }

    /// Paint and update hit geometry.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut IconButtonState) {
        let _ = self.paint(area, buffer, state);
    }

    /// Semantic registration (label = accessible name).
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &IconButtonState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        let hit = state.hit.unwrap_or(area);
        if hit.is_empty() {
            return;
        }
        let _ = scene.register(
            SemanticNode::control(id, hit)
                .role(SemanticRole::Button)
                .label(self.accessible_label)
                .description(self.help_text())
                .focusable(state.activation.accepts_input() && state.activation.is_enabled())
                .state(SemanticState {
                    selected: state.pressed || state.activation.accepts_input(),
                    pressed: state.activation.is_armed(),
                    checked: state.pressed,
                    busy: state.activation.is_loading(),
                    ..Default::default()
                }),
        );
    }

    /// Accessible label for toolbar overflow / overflow menus (same as a11y).
    #[must_use]
    pub const fn to_toolbar_label(&self) -> &'a str {
        self.accessible_label
    }
}

/// Build a toolbar action that prefers icon + keeps label for a11y/overflow.
///
/// Defined on [`crate::widgets::ToolbarItem`] via this free function to keep
/// IconButton free of a hard module cycle with toolbar.
#[must_use]
pub fn toolbar_icon_action<'a, Id>(
    id: Id,
    icon: &'a str,
    accessible_label: &'a str,
) -> super::ToolbarItem<'a, Id> {
    super::ToolbarItem::action(id, accessible_label).icon(icon)
}

// ── Badge / Tag / Chip ──────────────────────────────────────────────────────
// Badge: `widgets/badge.rs`. Tag / Chip / TokenStrip: `widgets/tag_chip.rs`.

// ── Kbd / Separator / Spinner ───────────────────────────────────────────────
// Kbd + ShortcutHint live in `widgets/kbd.rs`.

// Separator lives in `widgets/separator.rs` (variants, labels, ASCII glyphs).

// Spinner: `widgets/spinner.rs`.

/// Hit helper for button registration.
#[must_use]
pub fn button_hit<Id: Clone>(id: Id, state: &ButtonState) -> Option<HitRegion<Id>> {
    state.region.map(|area| HitRegion { id, area })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;
    use ratatui_core::layout::Position;
    use std::time::{Duration, Instant};

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn space_activates_on_a_terminal_without_release_reporting() {
        use crate::input::{KeyCode, KeyEvent, KeyModifiers, KeyReleaseReporting};

        let mut state = ActivationState::new();
        state.set_accepts_input(true);
        // The default terminal reports presses only.
        assert_eq!(state.release_reporting(), KeyReleaseReporting::PressOnly);
        let out = state.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(
            matches!(out, ActivationOutcome::Activated),
            "Space must activate where releases are never reported: {out:?}"
        );
        assert!(!state.is_armed(), "and it must not stay armed");

        // Where releases are reported, the dialog discipline stands.
        let mut kitty = ActivationState::new();
        kitty.set_accepts_input(true);
        kitty.set_release_reporting(KeyReleaseReporting::Reported);
        let armed = kitty.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(matches!(armed, ActivationOutcome::Pressed));
        let fired = kitty.handle_key(KeyEvent {
            kind: KeyEventKind::Release,
            ..KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)
        });
        assert!(matches!(fired, ActivationOutcome::Activated));
    }

    #[test]
    fn primary_button_is_accent_chip() {
        let system = DesignSystem::default();
        let mut state = ButtonState::new();
        state.activation.set_enabled(true);
        let area = Rect::new(0, 0, 16, 1);
        let mut buffer = Buffer::empty(area);
        Button::new("Run", &system)
            .as_primary()
            .paint(area, &mut buffer, &mut state);
        let cell = &buffer[(2, 0)];
        // junie: the primary button is the accent chip — accent fill, canvas
        // label — and focus speaks through the gutter/weight, never a second
        // fill.
        assert_eq!(cell.bg, system.style(Role::Accent).fg.unwrap());
        assert_eq!(cell.fg, system.style(Role::TextOnAccent).fg.unwrap());
        // Unfocused primary: the bar character is reserved, painted in the fill.
        let gutter = &buffer[(0, 0)];
        assert_eq!(gutter.symbol(), "▎");
        assert_eq!(gutter.fg, gutter.bg);
    }

    #[test]
    fn idle_secondary_is_gutter_then_label() {
        let system = DesignSystem::default();
        let mut state = ButtonState::new();
        let area = Rect::new(0, 0, 16, 1);
        let mut buffer = Buffer::empty(area);
        Button::new("Run", &system).paint(area, &mut buffer, &mut state);
        assert_eq!(buffer[(0, 0)].symbol(), "▎");
        assert_eq!(buffer[(0, 0)].fg, buffer[(0, 0)].bg);
        assert_eq!(buffer[(1, 0)].symbol(), "R");
        assert_ne!(buffer[(1, 0)].symbol(), "›");
        assert_eq!(Button::new("Run", &system).preferred_width(), 5);
    }

    #[test]
    fn focused_button_paints_visible_gutter() {
        let system = DesignSystem::junie();
        let mut state = ButtonState::new();
        state.activation.set_enabled(true);
        state.activation.set_accepts_input(true);
        state.focused = true;
        let area = Rect::new(0, 0, 16, 1);
        let mut buffer = Buffer::empty(area);
        Button::new("Preview", &system)
            .as_secondary()
            .paint(area, &mut buffer, &mut state);
        let gutter = &buffer[(0, 0)];
        assert_eq!(gutter.symbol(), "▎");
        assert_eq!(gutter.fg, system.style(Role::Focus).fg.unwrap());
        assert_ne!(gutter.fg, gutter.bg);
    }

    #[test]
    fn button_enter_activates_once_when_accepts_input() {
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        assert_eq!(
            state.handle_key(press(KeyCode::Enter)),
            ActivationOutcome::Activated
        );
        // Repeat must not multi-fire
        let mut rep = press(KeyCode::Enter);
        rep.kind = KeyEventKind::Repeat;
        assert_eq!(state.handle_key(rep), ActivationOutcome::Ignored);
    }

    #[test]
    fn button_disabled_never_activates() {
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        state.activation.set_enabled(false);
        assert_eq!(
            state.handle_key(press(KeyCode::Enter)),
            ActivationOutcome::Ignored
        );
        assert_eq!(
            state.handle_key(press(KeyCode::Char(' '))),
            ActivationOutcome::Ignored
        );
    }

    #[test]
    fn button_loading_never_activates_and_differs_from_disabled() {
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        state.activation.set_loading(true);
        assert!(state.activation.is_enabled());
        assert!(state.activation.is_loading());
        assert_eq!(
            state.handle_key(press(KeyCode::Char(' '))),
            ActivationOutcome::Ignored
        );
    }

    #[test]
    fn button_space_arms_then_release_activates() {
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        // Press-to-arm needs a terminal that reports the release (plans/021).
        state
            .activation
            .set_release_reporting(crate::input::KeyReleaseReporting::Reported);
        assert_eq!(
            state.handle_key(press(KeyCode::Char(' '))),
            ActivationOutcome::Pressed
        );
        let mut rel = press(KeyCode::Char(' '));
        rel.kind = KeyEventKind::Release;
        assert_eq!(state.handle_key(rel), ActivationOutcome::Activated);
    }

    #[test]
    fn button_pending_confirmation_requires_two_activates() {
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        state.activation.set_pending_confirmation(true);
        assert_eq!(
            state.handle_intent(crate::interaction::UiIntent::Activate),
            ActivationOutcome::ConfirmRequired
        );
        assert_eq!(
            state.handle_intent(crate::interaction::UiIntent::Activate),
            ActivationOutcome::Activated
        );
    }

    #[test]
    fn destructive_is_not_safe_default_focus() {
        let system = DesignSystem::default();
        assert!(
            Button::new("Save", &system)
                .variant(ButtonVariant::Primary)
                .is_safe_default_focus()
        );
        assert!(
            !Button::new("Delete", &system)
                .variant(ButtonVariant::Destructive)
                .is_safe_default_focus()
        );
    }

    #[test]
    fn button_mouse_down_up_activates() {
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        state.activation.set_enabled(true);
        state.region = Some(Rect::new(0, 0, 8, 1));
        let down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(1, 0),
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(state.handle_mouse(down), ActivationOutcome::Pressed);
        let up = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            position: Position::new(1, 0),
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(state.handle_mouse(up), ActivationOutcome::Activated);
    }

    #[test]
    fn button_paint_variants_and_narrow() {
        let system = DesignSystem::default();
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        let area = Rect::new(0, 0, 20, 1);
        let mut buffer = Buffer::empty(area);
        Button::new("Save", &system)
            .as_primary()
            .leading("✓")
            .render(area, &mut buffer, &mut state);
        assert!(state.region.is_some());
        let text: String = (0..20)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(text.contains("Save"), "{text:?}");

        let tiny = Rect::new(0, 0, 5, 1);
        let mut tbuf = Buffer::empty(tiny);
        Button::new("SaveChanges", &system).render(tiny, &mut tbuf, &mut state);
        assert!(state.region.is_some());
    }

    #[test]
    fn all_variants_paint_without_panic() {
        let system = DesignSystem::default();
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        let area = Rect::new(0, 0, 16, 1);
        for v in [
            ButtonVariant::Primary,
            ButtonVariant::Secondary,
            ButtonVariant::Quiet,
            ButtonVariant::Destructive,
        ] {
            let mut buf = Buffer::empty(area);
            Button::new("Act", &system)
                .variant(v)
                .paint(area, &mut buf, &mut state);
            assert!(state.region.is_some(), "{}", v.id());
        }
    }

    #[test]
    fn semantic_bold_focus_survives_no_color_paint() {
        let system = DesignSystem::default().no_color();
        let area = Rect::new(0, 0, 12, 1);
        for variant in [ButtonVariant::Secondary, ButtonVariant::Quiet] {
            let mut idle = Buffer::empty(area);
            let mut idle_state = ButtonState::new();
            Button::new("Run", &system)
                .variant(variant)
                .paint(area, &mut idle, &mut idle_state);

            let mut focused = Buffer::empty(area);
            let mut focused_state = ButtonState::new();
            focused_state.activation.set_accepts_input(true);
            focused_state.focused = true;
            Button::new("Run", &system).variant(variant).paint(
                area,
                &mut focused,
                &mut focused_state,
            );

            assert_ne!(idle.content(), focused.content(), "{variant:?}");
        }
    }

    #[test]
    fn mouse_up_outside_does_not_activate() {
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        state.region = Some(Rect::new(0, 0, 8, 1));
        let down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(1, 0),
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(state.handle_mouse(down), ActivationOutcome::Pressed);
        let up = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            position: Position::new(20, 0),
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(state.handle_mouse(up), ActivationOutcome::Ignored);
    }

    #[test]
    fn full_width_expands_hit() {
        let system = DesignSystem::default();
        let mut state = ButtonState::new();
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        let parts = Button::new("OK", &system)
            .full_width(true)
            .paint(area, &mut buf, &mut state);
        assert_eq!(parts.root.width, 40);
        assert_eq!(state.region.map(|r| r.width), Some(40));
    }

    #[test]
    fn icon_button_requires_accessible_label() {
        let system = DesignSystem::default();
        let mut state = IconButtonState::new();
        state.activation.set_accepts_input(true);
        let area = Rect::new(0, 0, 4, 1);
        let mut buffer = Buffer::empty(area);
        IconButton::new("×", "Close", &system).render(area, &mut buffer, &mut state);
        assert_eq!(IconButton::new("×", "Close", &system).a11y_name(), "Close");
        assert!(state.hit.map(|r| r.width >= 3).unwrap_or(false));
    }

    #[test]
    fn icon_button_hit_slop_exceeds_visual() {
        let system = DesignSystem::default();
        let mut state = IconButtonState::new();
        state.activation.set_accepts_input(true);
        let area = Rect::new(0, 0, 5, 1);
        let mut buf = Buffer::empty(area);
        let parts = IconButton::new("×", "Close", &system)
            .ascii_glyph("x")
            .paint(area, &mut buf, &mut state);
        assert!(parts.root.width >= ICON_BUTTON_MIN_HIT);
        assert!(parts.visual.width <= parts.root.width);
        // Glyph not stretched across full hit
        assert!(parts.visual.width <= 2);
    }

    #[test]
    fn icon_button_empty_label_is_unsafe() {
        let system = DesignSystem::default();
        assert!(!IconButton::new("×", "", &system).has_accessible_label());
    }

    #[test]
    fn icon_button_toggle_and_badge() {
        let system = DesignSystem::default();
        let mut state = IconButtonState::new();
        state.activation.set_accepts_input(true);
        state.set_pressed(true);
        let area = Rect::new(0, 0, 4, 1);
        let mut buf = Buffer::empty(area);
        let parts = IconButton::new("*", "Star", &system)
            .toggle(true)
            .badge("3")
            .paint(area, &mut buf, &mut state);
        assert_eq!(parts.badge.width, 1);
    }

    #[test]
    fn icon_button_mouse_uses_hit_slop() {
        let system = DesignSystem::default();
        let mut state = IconButtonState::new();
        state.activation.set_accepts_input(true);
        let area = Rect::new(0, 0, 5, 1);
        let mut buf = Buffer::empty(area);
        let _ = IconButton::new("×", "Close", &system).paint(area, &mut buf, &mut state);
        // Click on slop cell (right of visual if centered)
        let hit = state.hit.unwrap();
        let pos = Position::new(hit.x.saturating_add(hit.width.saturating_sub(1)), hit.y);
        let down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: pos,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(state.handle_mouse(down), ActivationOutcome::Pressed);
        let up = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            position: pos,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(state.handle_mouse(up), ActivationOutcome::Activated);
    }

    #[test]
    fn toolbar_icon_action_keeps_label() {
        let item = toolbar_icon_action("close", "×", "Close");
        assert_eq!(item.label, "Close");
        assert_eq!(item.icon, Some("×"));
    }

    #[test]
    fn chip_toggle_space() {
        use crate::widgets::{Chip, ChipOutcome, ChipState};
        let system = DesignSystem::default();
        let chip = Chip::new("f1", "rust", &system);
        let mut state = ChipState::new(false);
        state.set_focused(true);
        assert!(matches!(
            chip.handle_key(&mut state, press(KeyCode::Char(' '))),
            ChipOutcome::Selected("f1")
        ));
        assert!(state.is_selected());
        assert!(matches!(
            chip.handle_key(&mut state, press(KeyCode::Char(' '))),
            ChipOutcome::Unselected("f1")
        ));
    }

    #[test]
    fn tag_remove_on_delete() {
        use crate::widgets::{Tag, TagOutcome, TagState};
        let system = DesignSystem::default();
        let tag = Tag::removable_tag("t", "file", &system);
        let mut state = TagState::new();
        state.set_focused(true);
        assert!(matches!(
            tag.handle_key(&mut state, press(KeyCode::Delete)),
            TagOutcome::Remove("t")
        ));
    }

    #[test]
    fn badge_and_separator_paint() {
        let tokens = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
        Widget::render(
            &crate::widgets::Badge::new("NEW", &tokens),
            Rect::new(0, 0, 10, 1),
            &mut buf,
        );
        Widget::render(
            &crate::widgets::Separator::new(&tokens),
            Rect::new(0, 1, 20, 1),
            &mut buf,
        );
        assert!(!buf[(0, 0)].symbol().is_empty());
    }
}

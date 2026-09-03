// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Tag and Chip — removable / selectable compact tokens.
//!
//! **Tag** — static or removable entity/attachment token.  
//! **Chip** — selectable filter token; may also be removable, errored, or loading.
//!
//! **Internal focus.** Removable tokens use [`TokenPart`] (`Body` | `Remove`) so
//! Left/Right moves between label and remove **without Tab**. Host Tab / roving
//! moves between tokens (see [`TokenStrip`]).
//!
//! **Strip layout.** [`TokenStrip`] supports wrap, horizontal scroll, and
//! `+N` overflow summaries for filters, paste chips, and attachments.
//!
//! Glyphs resolve via [`crate::style::GlyphSet`]. Removal always has an explicit
//! semantic label (`remove {name}`) for inspection / help.
//!
//! References: token inputs, Grok paste/file chips, desktop filter chips.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::input::{KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use crate::interaction::{
    EventResult, RovingEntry, RovingFocusGroup, RovingOrientation, RovingOutcome, SemanticNode,
    SemanticRole, SemanticScene, SemanticState, UiIntent, default_button_intent,
};
use crate::style::{ButtonRecipeVariant, ControlState, DesignSystem, Glyph, Role};
use crate::text::{display_cols, take_display_cols};

// ── Shared token chrome ─────────────────────────────────────────────────────

/// Focus inside one removable token (not host Tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TokenPart {
    /// Label / body (default).
    #[default]
    Body,
    /// Remove affordance.
    Remove,
}

impl TokenPart {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Remove => "remove",
        }
    }
}

/// Visual / semantic status for chips and tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TokenStatus {
    /// Normal.
    #[default]
    Default,
    /// Error / invalid token.
    Error,
    /// Loading / pending.
    Loading,
}

impl TokenStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Error => "error",
            Self::Loading => "loading",
        }
    }
}

/// Geometry for one painted token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TokenParts {
    /// Full token band.
    pub root: Rect,
    /// Label / body hit region.
    pub body: Rect,
    /// Remove hit region (empty when not removable).
    pub remove: Rect,
}

impl TokenParts {
    /// True when remove has area.
    #[must_use]
    pub const fn has_remove(self) -> bool {
        self.remove.width > 0 && self.remove.height > 0
    }
}

/// Explicit removal text for semantic inspection / a11y.
#[must_use]
pub fn remove_label(token_label: &str) -> String {
    format!("remove {token_label}")
}

fn remove_glyph(system: &DesignSystem) -> &'static str {
    system.glyphs.resolve(Glyph::Close).text
}

fn loading_glyph(system: &DesignSystem) -> &'static str {
    system.glyphs.loading()
}

// ── Tag ─────────────────────────────────────────────────────────────────────

/// Tag outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TagOutcome<Id> {
    /// No change.
    Ignored,
    /// Removal requested.
    Remove(Id),
    /// Internal part focus moved (Body ↔ Remove).
    PartChanged(TokenPart),
    /// Body activated (Enter on non-removable or body when not remove-focused).
    Activated(Id),
    /// Pointer entered or left the remove glyph.
    HoverChanged,
}

/// Removable or static compact tag (entity / attachment).
#[derive(Debug, Clone, Copy)]
pub struct Tag<'a, Id> {
    /// Stable identity.
    pub id: Id,
    label: &'a str,
    system: &'a DesignSystem,
    removable: bool,
    status: TokenStatus,
    disabled: bool,
}

impl<'a, Id> Tag<'a, Id> {
    /// Static tag (not removable by default).
    #[must_use]
    pub const fn new(id: Id, label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            id,
            label,
            system,
            removable: false,
            status: TokenStatus::Default,
            disabled: false,
        }
    }

    /// Removable tag (attachments / paste chips).
    #[must_use]
    pub const fn removable_tag(id: Id, label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            id,
            label,
            system,
            removable: true,
            status: TokenStatus::Default,
            disabled: false,
        }
    }

    /// Removable flag.
    #[must_use]
    pub const fn removable(mut self, on: bool) -> Self {
        self.removable = on;
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, status: TokenStatus) -> Self {
        self.status = status;
        self
    }

    /// Error tag.
    #[must_use]
    pub const fn error(mut self) -> Self {
        self.status = TokenStatus::Error;
        self
    }

    /// Loading tag.
    #[must_use]
    pub const fn loading(mut self) -> Self {
        self.status = TokenStatus::Loading;
        self
    }

    /// Disabled.
    #[must_use]
    pub const fn disabled(mut self, on: bool) -> Self {
        self.disabled = on;
        self
    }

    /// Label text.
    #[must_use]
    pub const fn label(&self) -> &'a str {
        self.label
    }

    /// Whether removable.
    #[must_use]
    pub const fn is_removable(&self) -> bool {
        self.removable && !self.disabled
    }

    /// Semantic remove action label.
    #[must_use]
    pub fn remove_action_label(&self) -> String {
        remove_label(self.label)
    }

    /// Measure natural width.
    #[must_use]
    pub fn measure_width(&self) -> u16 {
        u16::try_from(display_cols(&self.decorated_body()))
            .unwrap_or(1)
            .saturating_add(1) // gutter ▎
            .saturating_add(if self.removable {
                1 + u16::try_from(display_cols(remove_glyph(self.system))).unwrap_or(1)
            } else {
                0
            })
            .saturating_add(1) // trailing pad
            .max(1)
    }

    fn decorated_body(&self) -> String {
        let mut s = String::new();
        match self.status {
            TokenStatus::Loading => {
                s.push_str(loading_glyph(self.system));
                s.push(' ');
            }
            TokenStatus::Error => {
                s.push_str(self.system.glyphs.resolve(Glyph::Error).text);
                s.push(' ');
            }
            TokenStatus::Default => {}
        }
        s.push_str(self.label);
        s
    }
}

/// Tag interaction + geometry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TagState {
    /// Host gave focus to this token.
    pub focused: bool,
    /// Body vs remove (when removable).
    pub part: TokenPart,
    /// Pointer is over the remove glyph.
    pub hovered_remove: bool,
    /// Cached parts.
    pub parts: Option<TokenParts>,
}

impl TagState {
    /// Default unfocused.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            focused: false,
            part: TokenPart::Body,
            hovered_remove: false,
            parts: None,
        }
    }

    /// Focus flag.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.part = TokenPart::Body;
        }
    }

    /// Active part.
    pub const fn set_part(&mut self, part: TokenPart) {
        self.part = part;
    }
}

impl<'a, Id: Clone> Tag<'a, Id> {
    /// Paint; updates state geometry.
    /// Paint; updates state geometry.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut TagState) -> TokenParts {
        state.parts = None;
        if area.is_empty() {
            return TokenParts::default();
        }
        // A tag is a neutral label: angle brackets, no selection pip.
        let parts = TokenPaint {
            system: self.system,
            mark: None,
            prefix: token_prefix(self.system, self.status),
            label: self.label,
            removable: self.is_removable(),
            status: self.status,
            focused: state.focused,
            selected: false,
            disabled: self.disabled,
            part: state.part,
            hovered_remove: state.hovered_remove,
            overlay: false,
        }
        .paint(area, buffer);
        state.parts = Some(parts);
        parts
    }

    /// Key path: Left/Right part, Delete remove, Enter activate/remove.
    pub fn handle_key(&self, state: &mut TagState, key: KeyEvent) -> TagOutcome<Id> {
        if self.disabled || !state.focused || !key.is_press() {
            return TagOutcome::Ignored;
        }
        if let Some(intent) = default_button_intent(key) {
            if matches!(intent, UiIntent::Activate) {
                return if self.is_removable() && matches!(state.part, TokenPart::Remove) {
                    TagOutcome::Remove(self.id.clone())
                } else {
                    TagOutcome::Activated(self.id.clone())
                };
            }
            if matches!(intent, UiIntent::Activate | UiIntent::Toggle)
                && self.is_removable()
                && matches!(state.part, TokenPart::Remove)
            {
                return TagOutcome::Remove(self.id.clone());
            }
        }
        match key.code {
            KeyCode::Left | KeyCode::Char('h') if self.is_removable() => {
                if matches!(state.part, TokenPart::Remove) {
                    state.part = TokenPart::Body;
                    return TagOutcome::PartChanged(TokenPart::Body);
                }
                TagOutcome::Ignored
            }
            KeyCode::Right | KeyCode::Char('l') if self.is_removable() => {
                if matches!(state.part, TokenPart::Body) {
                    state.part = TokenPart::Remove;
                    return TagOutcome::PartChanged(TokenPart::Remove);
                }
                TagOutcome::Ignored
            }
            KeyCode::Delete | KeyCode::Backspace if self.is_removable() => {
                TagOutcome::Remove(self.id.clone())
            }
            _ => TagOutcome::Ignored,
        }
    }

    /// Mouse: body activate, remove hits remove.
    pub fn handle_mouse(&self, state: &mut TagState, event: MouseEvent) -> TagOutcome<Id> {
        if self.disabled {
            return TagOutcome::Ignored;
        }
        if matches!(event.kind, MouseEventKind::Moved) {
            let Some(parts) = state.parts else {
                return TagOutcome::Ignored;
            };
            let over = parts.has_remove() && parts.remove.contains(event.position);
            if over == state.hovered_remove {
                return TagOutcome::Ignored;
            }
            state.hovered_remove = over;
            return TagOutcome::HoverChanged;
        }
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return TagOutcome::Ignored;
        }
        let Some(parts) = state.parts else {
            return TagOutcome::Ignored;
        };
        if parts.has_remove() && parts.remove.contains(event.position) {
            state.focused = true;
            state.part = TokenPart::Remove;
            return TagOutcome::Remove(self.id.clone());
        }
        if parts.root.contains(event.position) || parts.body.contains(event.position) {
            state.focused = true;
            state.part = TokenPart::Body;
            return TagOutcome::Activated(self.id.clone());
        }
        TagOutcome::Ignored
    }

    /// EventResult wrapper.
    pub fn handle_key_result(
        &self,
        state: &mut TagState,
        key: KeyEvent,
    ) -> EventResult<TagOutcome<Id>> {
        match self.handle_key(state, key) {
            TagOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Semantic registration (body + remove when present).
    pub fn register_semantic<Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        area: Rect,
        state: &TagState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        let mut st = TagState {
            focused: state.focused,
            part: state.part,
            hovered_remove: state.hovered_remove,
            parts: None,
        };
        // layout without paint
        let parts = {
            let mut buf = Buffer::empty(Rect::new(0, 0, area.width.max(1), 1));
            self.paint(
                Rect {
                    x: 0,
                    y: 0,
                    width: area.width,
                    height: 1,
                },
                &mut buf,
                &mut st,
            )
        };
        let body = offset_rect(parts.body, area.x, area.y);
        let rem = offset_rect(parts.remove, area.x, area.y);
        let _ = scene.register(
            SemanticNode::content(self.id.clone(), body)
                .role(SemanticRole::Content)
                .label(self.label)
                .description(format!("tag; {}", self.status.id()))
                .focusable(false),
        );
        if parts.has_remove() {
            let _ = scene.register(
                SemanticNode::control(self.id.clone(), rem)
                    .role(SemanticRole::Button)
                    .label(self.remove_action_label())
                    .description("remove action")
                    .focusable(self.is_removable())
                    .state(SemanticState {
                        selected: matches!(state.part, TokenPart::Remove) && state.focused,
                        ..Default::default()
                    }),
            );
        }
    }
}

fn offset_rect(r: Rect, ox: u16, oy: u16) -> Rect {
    if r.width == 0 || r.height == 0 {
        return r;
    }
    Rect {
        x: ox.saturating_add(r.x),
        y: oy.saturating_add(r.y),
        width: r.width,
        height: r.height,
    }
}

/// Which brackets a token wears.
///
/// The shape says what kind of token it is before any colour does: a neutral
/// label is angled, an interactive one is squared (law P13, audit F4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BracketStyle {
    /// `⟨ label ⟩` — a neutral tag (ASCII `< >`).
    Angle,
    /// `[ label ]` — an interactive chip or keycap.
    #[default]
    Square,
}

impl BracketStyle {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Angle => "angle",
            Self::Square => "square",
        }
    }

    /// Opening and closing brackets under the active glyph profile.
    #[must_use]
    pub const fn pair(self, ascii: bool) -> (&'static str, &'static str) {
        match (self, ascii) {
            (Self::Angle, false) => ("⟨", "⟩"),
            (Self::Angle, true) => ("<", ">"),
            (Self::Square, _) => ("[", "]"),
        }
    }
}

/// One token's paint plan: gutter, an optional mark, a label, a remove slot.
///
/// Filter chips are a row: `▎label ×`. The `×` stays faint until hovered.
/// No boxed-pill chrome.
struct TokenPaint<'a> {
    system: &'a DesignSystem,
    /// Selection pip for interactive tokens (shape before colour).
    mark: Option<&'a str>,
    /// Status glyph painted before the label.
    prefix: Option<&'a str>,
    label: &'a str,
    removable: bool,
    status: TokenStatus,
    focused: bool,
    selected: bool,
    disabled: bool,
    part: TokenPart,
    hovered_remove: bool,
    /// Chip wells sit on overlay (junie Toggle/Secondary). Tags stay quiet.
    overlay: bool,
}

impl TokenPaint<'_> {
    fn paint(&self, area: Rect, buffer: &mut Buffer) -> TokenParts {
        if area.is_empty() {
            return TokenParts::default();
        }
        let remove = if self.removable {
            remove_glyph(self.system)
        } else {
            ""
        };
        let gutter = self.system.glyphs.selection_gutter();

        let mut inner = String::new();
        if let Some(prefix) = self.prefix {
            inner.push_str(prefix);
            inner.push(' ');
        }
        if let Some(mark) = self.mark {
            inner.push_str(mark);
            inner.push(' ');
        }
        inner.push_str(self.label);

        let mut style = token_style(
            self.system,
            self.status,
            self.focused,
            self.selected,
            self.disabled,
            self.overlay,
        );
        if self.focused {
            style = style.add_modifier(Modifier::BOLD);
        }
        buffer.set_style(
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1.min(area.height),
            },
            style,
        );

        let fill_bg = style.bg.unwrap_or(self.system.junie_theme().surface);
        let gutter_style = self.system.gutter(
            crate::style::VisualState {
                focused: self.focused,
                selected: self.selected,
                disabled: self.disabled,
                ..crate::style::VisualState::default()
            },
            fill_bg,
            false,
        );
        buffer.set_stringn(area.x, area.y, gutter, 1, gutter_style);

        let inner_w = u16::try_from(display_cols(&inner)).unwrap_or(0);
        let body_x = area.x.saturating_add(1);
        let body_w = inner_w.min(area.right().saturating_sub(body_x));
        if body_w > 0 {
            buffer.set_stringn(
                body_x,
                area.y,
                &take_display_cols(&inner, usize::from(body_w)),
                usize::from(body_w),
                style,
            );
        }

        let remove_w = if self.removable {
            u16::try_from(display_cols(remove)).unwrap_or(1)
        } else {
            0
        };
        let remove_x = body_x
            .saturating_add(body_w)
            .saturating_add(1)
            .min(area.right().saturating_sub(remove_w.max(1)));
        let remove_rect = if self.removable && remove_x < area.right() && remove_w > 0 {
            let hot =
                self.hovered_remove || (self.focused && matches!(self.part, TokenPart::Remove));
            let xs = if hot {
                style
                    .patch(self.system.style(Role::TextStrong))
                    .add_modifier(Modifier::BOLD)
            } else {
                style.patch(self.system.style(Role::TextMuted))
            };
            buffer.set_stringn(remove_x, area.y, remove, 1, xs);
            Rect {
                x: remove_x,
                y: area.y,
                width: remove_w.max(1),
                height: 1.min(area.height),
            }
        } else {
            Rect::default()
        };

        let used = if self.removable {
            remove_x
                .saturating_add(remove_w)
                .saturating_sub(area.x)
                .saturating_add(1)
                .min(area.width)
        } else {
            1u16.saturating_add(body_w).min(area.width)
        };

        TokenParts {
            root: Rect {
                x: area.x,
                y: area.y,
                width: used,
                height: 1.min(area.height),
            },
            body: Rect {
                x: body_x,
                y: area.y,
                width: body_w,
                height: 1.min(area.height),
            },
            remove: remove_rect,
        }
    }
}

/// The status glyph a token wears before its label, if any.
fn token_prefix(system: &DesignSystem, status: TokenStatus) -> Option<&'static str> {
    match status {
        TokenStatus::Loading => Some(loading_glyph(system)),
        TokenStatus::Error => Some(system.glyphs.resolve(Glyph::Error).text),
        TokenStatus::Default => None,
    }
}

fn token_style(
    system: &DesignSystem,
    status: TokenStatus,
    focused: bool,
    selected: bool,
    disabled: bool,
    overlay: bool,
) -> ratatui_core::style::Style {
    // Junie chips always use Toggle/Secondary overlay fill. Enabled vs not
    // is foreground weight, not a second well. Tags stay Quiet on surface.
    let recipe = system.button_recipe(
        if overlay || selected {
            ButtonRecipeVariant::Secondary
        } else {
            ButtonRecipeVariant::Quiet
        },
        if disabled {
            ControlState::Disabled
        } else if matches!(status, TokenStatus::Loading) {
            ControlState::Loading
        } else if focused {
            ControlState::Focused
        } else {
            ControlState::Default
        },
        system.junie_theme().surface,
    );
    // Status and membership are different facts and compose: an errored chip
    // that is also selected used to lose its selection entirely, because the
    // status arm matched first and returned (plans/021 Step 4).
    let mut style = recipe.fill.patch(recipe.label);
    if matches!(status, TokenStatus::Error) {
        style = style.patch(system.style(Role::Danger));
    }
    // Source ChipBar: disabled/off chips keep the overlay well but `text_faint`.
    if overlay && !selected && !disabled && !matches!(status, TokenStatus::Error) {
        style = style.fg(system.junie_theme().text_faint);
    }
    if focused {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}

// ── Chip ────────────────────────────────────────────────────────────────────

/// Chip outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChipOutcome<Id> {
    /// No change.
    Ignored,
    /// Selected.
    Selected(Id),
    /// Unselected.
    Unselected(Id),
    /// Removal requested.
    Remove(Id),
    /// Internal part focus changed.
    PartChanged(TokenPart),
    /// Body activated without toggle (e.g. loading chip expand).
    Activated(Id),
    /// Pointer entered or left the remove glyph.
    HoverChanged,
}

/// Selectable filter / token chip.
#[derive(Debug, Clone, Copy)]
pub struct Chip<'a, Id> {
    /// Stable identity.
    pub id: Id,
    label: &'a str,
    system: &'a DesignSystem,
    removable: bool,
    status: TokenStatus,
    disabled: bool,
    /// When false, paint only (no toggle) — static chip chrome.
    interactive: bool,
}

impl<'a, Id> Chip<'a, Id> {
    /// Interactive toggle chip.
    #[must_use]
    pub const fn new(id: Id, label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            id,
            label,
            system,
            removable: false,
            status: TokenStatus::Default,
            disabled: false,
            interactive: true,
        }
    }

    /// Static display chip (no toggle).
    #[must_use]
    pub const fn static_chip(id: Id, label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            id,
            label,
            system,
            removable: false,
            status: TokenStatus::Default,
            disabled: false,
            interactive: false,
        }
    }

    /// Removable.
    #[must_use]
    pub const fn removable(mut self, on: bool) -> Self {
        self.removable = on;
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, status: TokenStatus) -> Self {
        self.status = status;
        self
    }

    /// Error chip.
    #[must_use]
    pub const fn error(mut self) -> Self {
        self.status = TokenStatus::Error;
        self
    }

    /// Loading chip.
    #[must_use]
    pub const fn loading(mut self) -> Self {
        self.status = TokenStatus::Loading;
        self
    }

    /// Disabled.
    #[must_use]
    pub const fn disabled(mut self, on: bool) -> Self {
        self.disabled = on;
        self
    }

    /// Interactive toggle.
    #[must_use]
    pub const fn interactive(mut self, on: bool) -> Self {
        self.interactive = on;
        self
    }

    /// Label.
    #[must_use]
    pub const fn label(&self) -> &'a str {
        self.label
    }

    /// Removable and enabled.
    #[must_use]
    pub const fn is_removable(&self) -> bool {
        self.removable && !self.disabled
    }

    /// Semantic remove label.
    #[must_use]
    pub fn remove_action_label(&self) -> String {
        remove_label(self.label)
    }

    /// Measure width for selected state.
    ///
    /// Source ChipBar: `1 + label + 1 + removable 2 + 1`, then a 1-cell gap
    /// after the chip. The extra removable cell is trailing wash, not a
    /// second glyph.
    #[must_use]
    pub fn measure_width(&self, _selected: bool) -> u16 {
        let mut body = self.label.to_string();
        if matches!(self.status, TokenStatus::Loading) {
            body = format!("{} {body}", loading_glyph(self.system));
        }
        if matches!(self.status, TokenStatus::Error) {
            body = format!("{} {body}", self.system.glyphs.resolve(Glyph::Error).text);
        }
        let mut w = 1 + display_cols(&body) + 1;
        if self.removable {
            w += 2;
        }
        w += 1;
        u16::try_from(w).unwrap_or(1).max(1)
    }
}

/// Chip interaction state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChipState {
    /// Selected (controlled store; host may override).
    selected: bool,
    /// Host focus.
    focused: bool,
    /// Body vs remove.
    part: TokenPart,
    /// Pointer is over the remove glyph.
    hovered_remove: bool,
    /// Geometry.
    parts: Option<TokenParts>,
}

impl ChipState {
    /// Selection seed.
    #[must_use]
    pub const fn new(selected: bool) -> Self {
        Self {
            selected,
            focused: false,
            part: TokenPart::Body,
            hovered_remove: false,
            parts: None,
        }
    }

    /// Selected.
    #[must_use]
    pub const fn is_selected(&self) -> bool {
        self.selected
    }

    /// Set selected.
    pub const fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    /// Focus.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.part = TokenPart::Body;
        }
    }

    /// Focused.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Part.
    #[must_use]
    pub const fn part(&self) -> TokenPart {
        self.part
    }

    /// Set part.
    pub const fn set_part(&mut self, part: TokenPart) {
        self.part = part;
    }

    /// Parts.
    #[must_use]
    pub const fn parts(&self) -> Option<TokenParts> {
        self.parts
    }

    /// Pointer over the remove glyph.
    pub const fn set_hovered_remove(&mut self, on: bool) {
        self.hovered_remove = on;
    }
}

impl<'a, Id: Clone> Chip<'a, Id> {
    /// Paint chip.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut ChipState) -> TokenParts {
        state.parts = None;
        if area.is_empty() {
            return TokenParts::default();
        }
        let parts = TokenPaint {
            system: self.system,
            mark: None,
            prefix: token_prefix(self.system, self.status),
            label: self.label,
            removable: self.is_removable(),
            status: self.status,
            focused: state.focused,
            selected: state.selected,
            disabled: self.disabled,
            part: state.part,
            hovered_remove: state.hovered_remove,
            overlay: true,
        }
        .paint(area, buffer);
        state.parts = Some(parts);
        parts
    }

    /// Keys: part nav, toggle, remove.
    pub fn handle_key(&self, state: &mut ChipState, key: KeyEvent) -> ChipOutcome<Id> {
        if self.disabled || !state.focused || !key.is_press() {
            return ChipOutcome::Ignored;
        }
        if let Some(intent) = default_button_intent(key) {
            if matches!(intent, UiIntent::Activate) {
                return if self.is_removable() && matches!(state.part, TokenPart::Remove) {
                    ChipOutcome::Remove(self.id.clone())
                } else if self.interactive {
                    self.toggle(state)
                } else {
                    ChipOutcome::Activated(self.id.clone())
                };
            }
            if matches!(intent, UiIntent::Activate | UiIntent::Toggle)
                && self.interactive
                && matches!(state.part, TokenPart::Body)
            {
                return self.toggle(state);
            }
        }
        match key.code {
            KeyCode::Left | KeyCode::Char('h') if self.is_removable() => {
                if matches!(state.part, TokenPart::Remove) {
                    state.part = TokenPart::Body;
                    return ChipOutcome::PartChanged(TokenPart::Body);
                }
                ChipOutcome::Ignored
            }
            KeyCode::Right | KeyCode::Char('l') if self.is_removable() => {
                if matches!(state.part, TokenPart::Body) {
                    state.part = TokenPart::Remove;
                    return ChipOutcome::PartChanged(TokenPart::Remove);
                }
                ChipOutcome::Ignored
            }
            KeyCode::Delete | KeyCode::Backspace if self.is_removable() => {
                ChipOutcome::Remove(self.id.clone())
            }
            _ => ChipOutcome::Ignored,
        }
    }

    fn toggle(&self, state: &mut ChipState) -> ChipOutcome<Id> {
        state.selected = !state.selected;
        if state.selected {
            ChipOutcome::Selected(self.id.clone())
        } else {
            ChipOutcome::Unselected(self.id.clone())
        }
    }

    /// Mouse.
    pub fn handle_mouse(&self, state: &mut ChipState, event: MouseEvent) -> ChipOutcome<Id> {
        if self.disabled {
            return ChipOutcome::Ignored;
        }
        if matches!(event.kind, MouseEventKind::Moved) {
            let Some(parts) = state.parts else {
                return ChipOutcome::Ignored;
            };
            let over = parts.has_remove() && parts.remove.contains(event.position);
            if over == state.hovered_remove {
                return ChipOutcome::Ignored;
            }
            state.hovered_remove = over;
            return ChipOutcome::HoverChanged;
        }
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return ChipOutcome::Ignored;
        }
        let Some(parts) = state.parts else {
            return ChipOutcome::Ignored;
        };
        if parts.has_remove() && parts.remove.contains(event.position) {
            state.focused = true;
            state.part = TokenPart::Remove;
            return ChipOutcome::Remove(self.id.clone());
        }
        if parts.root.contains(event.position) {
            state.focused = true;
            state.part = TokenPart::Body;
            if self.interactive {
                return self.toggle(state);
            }
            return ChipOutcome::Activated(self.id.clone());
        }
        ChipOutcome::Ignored
    }

    /// EventResult.
    pub fn handle_key_result(
        &self,
        state: &mut ChipState,
        key: KeyEvent,
    ) -> EventResult<ChipOutcome<Id>> {
        match self.handle_key(state, key) {
            ChipOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }
}

// Legacy paint entry points used by older call sites.
impl<Id: Clone> Tag<'_, Id> {
    /// Paint (legacy name).
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut TagState) {
        let _ = self.paint(area, buffer, state);
    }
}

impl<Id: Clone> Chip<'_, Id> {
    /// Paint (legacy name).
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut ChipState) {
        let _ = self.paint(area, buffer, state);
    }
}

// ── TokenStrip (TokenField shared behavior) ──────────────────────────────────

/// How tokens are laid out in a strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TokenStripLayout {
    /// Single row; overflow horizontally with scroll offset.
    #[default]
    Scroll,
    /// Wrap to next rows.
    Wrap,
}

impl TokenStripLayout {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Scroll => "scroll",
            Self::Wrap => "wrap",
        }
    }
}

/// One strip entry (tag or chip projection).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenItem<'a, Id> {
    /// Identity.
    pub id: Id,
    /// Label.
    pub label: &'a str,
    /// Chip (selectable) vs tag (entity).
    pub selectable: bool,
    /// Removable.
    pub removable: bool,
    /// Selected (chips).
    pub selected: bool,
    /// Status.
    pub status: TokenStatus,
    /// Disabled.
    pub disabled: bool,
}

impl<'a, Id> TokenItem<'a, Id> {
    /// Selectable chip item.
    #[must_use]
    pub const fn chip(id: Id, label: &'a str) -> Self {
        Self {
            id,
            label,
            selectable: true,
            removable: false,
            selected: false,
            status: TokenStatus::Default,
            disabled: false,
        }
    }

    /// Static/removable tag item.
    #[must_use]
    pub const fn tag(id: Id, label: &'a str) -> Self {
        Self {
            id,
            label,
            selectable: false,
            removable: true,
            selected: false,
            status: TokenStatus::Default,
            disabled: false,
        }
    }

    /// Removable affordance.
    #[must_use]
    pub const fn removable(mut self, on: bool) -> Self {
        self.removable = on;
        self
    }

    /// Selected (chips).
    #[must_use]
    pub const fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    /// Error / loading status.
    #[must_use]
    pub const fn status(mut self, status: TokenStatus) -> Self {
        self.status = status;
        self
    }

    /// Disabled (skipped by roving).
    #[must_use]
    pub const fn disabled(mut self, on: bool) -> Self {
        self.disabled = on;
        self
    }
}

/// Strip outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TokenStripOutcome<Id> {
    /// Ignored.
    Ignored,
    /// Roving cursor moved.
    CursorMoved {
        /// From.
        from: Option<Id>,
        /// To.
        to: Option<Id>,
    },
    /// Chip selected.
    Selected(Id),
    /// Chip unselected.
    Unselected(Id),
    /// Token removed.
    Remove(Id),
    /// Token activated.
    Activated(Id),
    /// Overflow summary activated (`+N`).
    OverflowActivated,
    /// Part focus within token.
    PartChanged(TokenPart),
    /// Add-filter affordance activated.
    Add,
    /// Pointer entered or left a remove glyph.
    HoverChanged,
}

/// Token strip / TokenField geometry host.
#[derive(Debug, Clone)]
pub struct TokenStripState<Id> {
    /// Surface keyboard ownership.
    pub surface_focused: bool,
    /// Which token is active.
    pub roving: RovingFocusGroup<Id>,
    /// Part within the focused token.
    pub part: TokenPart,
    /// Horizontal scroll (cells) for Scroll layout.
    pub scroll: u16,
    /// Per-token regions (id, full rect).
    pub regions: Vec<(Id, Rect)>,
    /// Overflow chip region.
    pub overflow_region: Option<Rect>,
    /// Add-filter affordance region.
    pub add_region: Option<Rect>,
    /// Leading control (` match all ▾ `) region. Source ChipBar `lead`.
    pub lead_region: Option<Rect>,
    /// When false, chips stay idle even if the strip owns keyboard focus.
    pub show_chip_cursor: bool,
    /// Add-filter owns the strip cursor.
    pub add_focused: bool,
    /// Token whose remove glyph is hovered.
    pub hovered_remove: Option<Id>,
    /// Indices not shown (overflow).
    pub overflow_ids: Vec<Id>,
}

impl<Id> Default for TokenStripState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> TokenStripState<Id> {
    /// Empty strip state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            surface_focused: false,
            roving: RovingFocusGroup::new()
                .orientation(RovingOrientation::Horizontal)
                .wrap(true),
            part: TokenPart::Body,
            scroll: 0,
            regions: Vec::new(),
            overflow_region: None,
            add_region: None,
            lead_region: None,
            show_chip_cursor: false,
            add_focused: false,
            hovered_remove: None,
            overflow_ids: Vec::new(),
        }
    }

    /// Surface focus.
    pub const fn set_surface_focused(&mut self, on: bool) {
        self.surface_focused = on;
    }

    /// Cursor id.
    #[must_use]
    pub const fn cursor(&self) -> Option<&Id> {
        self.roving.active()
    }
}

impl<Id: Clone + PartialEq> TokenStripState<Id> {
    /// Set cursor.
    pub fn set_cursor(&mut self, id: Option<Id>) {
        self.roving.set_active(id);
        self.part = TokenPart::Body;
    }
}

/// Horizontal token field / filter strip.
#[derive(Debug, Clone)]
pub struct TokenStrip<'a, Id> {
    items: &'a [TokenItem<'a, Id>],
    system: &'a DesignSystem,
    layout: TokenStripLayout,
    /// Max tokens before `+N` overflow (0 = show all that fit).
    max_visible: usize,
    gap: u16,
    add_label: Option<&'a str>,
    /// Leading label such as `match all ▾` (source ChipBar `lead`).
    lead: Option<&'a str>,
}

impl<'a, Id> TokenStrip<'a, Id> {
    /// Strip over items.
    #[must_use]
    pub const fn new(items: &'a [TokenItem<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            items,
            system,
            layout: TokenStripLayout::Scroll,
            max_visible: 0,
            gap: 1,
            add_label: Some("+ Add filter"),
            lead: None,
        }
    }

    /// Wrap layout.
    #[must_use]
    pub const fn wrap(mut self) -> Self {
        self.layout = TokenStripLayout::Wrap;
        self
    }

    /// Scroll layout.
    #[must_use]
    pub const fn scroll(mut self) -> Self {
        self.layout = TokenStripLayout::Scroll;
        self
    }

    /// Cap visible tokens; remainder become `+N`.
    #[must_use]
    pub const fn max_visible(mut self, n: usize) -> Self {
        self.max_visible = n;
        self
    }

    /// Gap between tokens.
    #[must_use]
    pub const fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    /// Trailing add affordance (`+ Add filter`). `None` hides it.
    #[must_use]
    pub const fn add_label(mut self, label: Option<&'a str>) -> Self {
        self.add_label = label;
        self
    }

    /// Leading control. Source ChipBar paints `" {lead} "` in muted on the
    /// strip row, then a one-cell gap before the first chip.
    #[must_use]
    pub const fn lead(mut self, lead: Option<&'a str>) -> Self {
        self.lead = lead;
        self
    }
}

impl<'a, Id: Clone + PartialEq + std::fmt::Display> TokenStrip<'a, Id> {
    fn entries(&self) -> Vec<RovingEntry<Id>> {
        self.items
            .iter()
            .filter(|i| !i.disabled)
            .map(|i| RovingEntry::new(i.id.clone(), i.label.to_string()).enabled(true))
            .collect()
    }

    /// Paint strip; fills state regions / overflow.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut TokenStripState<Id>) {
        state.regions.clear();
        state.overflow_region = None;
        state.add_region = None;
        state.lead_region = None;
        state.overflow_ids.clear();
        if area.is_empty()
            || (self.items.is_empty() && self.add_label.is_none() && self.lead.is_none())
        {
            return;
        }
        let entries = self.entries();
        let _ = state.roving.reconcile(&entries);

        let max_v = if self.max_visible == 0 {
            self.items.len()
        } else {
            self.max_visible
        };
        let (visible, overflow): (Vec<_>, Vec<_>) = if self.items.len() > max_v {
            let (a, b) = self.items.split_at(max_v);
            (a.to_vec(), b.to_vec())
        } else {
            (self.items.to_vec(), Vec::new())
        };
        for o in &overflow {
            state.overflow_ids.push(o.id.clone());
        }

        let area = self.paint_lead(area, buffer, state);
        if area.is_empty() {
            return;
        }
        match self.layout {
            TokenStripLayout::Scroll => {
                self.paint_scroll(area, buffer, state, &visible, !overflow.is_empty());
            }
            TokenStripLayout::Wrap => {
                self.paint_wrap(area, buffer, state, &visible, !overflow.is_empty());
            }
        }
    }

    /// Source ChipBar: `" {lead} "` in muted on `surface`, then gap 1.
    fn paint_lead(&self, area: Rect, buffer: &mut Buffer, state: &mut TokenStripState<Id>) -> Rect {
        let Some(lead) = self.lead else {
            return area;
        };
        let text = format!(" {lead} ");
        let w = u16::try_from(display_cols(&text)).unwrap_or(0);
        if w == 0 || area.width <= w {
            return area;
        }
        let surface = self
            .system
            .style(Role::Surface)
            .bg
            .unwrap_or(ratatui_core::style::Color::Reset);
        let style = self.system.style(Role::TextMuted).bg(surface);
        buffer.set_string(area.x, area.y, &text, style);
        state.lead_region = Some(Rect {
            x: area.x,
            y: area.y,
            width: w,
            height: 1.min(area.height),
        });
        let next = area.x.saturating_add(w).saturating_add(self.gap);
        Rect {
            x: next,
            y: area.y,
            width: area.right().saturating_sub(next),
            height: area.height,
        }
    }

    fn paint_scroll(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut TokenStripState<Id>,
        visible: &[TokenItem<'a, Id>],
        has_overflow: bool,
    ) {
        // scroll is offset in cells of content start
        let mut content_x: i32 = -(i32::from(state.scroll));
        let overflow_w = if has_overflow { 5u16 } else { 0 };
        // Source ChipBar never shrinks a chip to keep the add control:
        // a chip that does not fit becomes `…` and the add is leftover-only.
        let budget_right = area.right().saturating_sub(overflow_w);

        for item in visible {
            let w = estimate_item_width(item, self.system);
            let abs_x = content_x;
            content_x += i32::from(w) + i32::from(self.gap);
            if abs_x + i32::from(w) < 0 {
                continue;
            }
            let draw_x = area
                .x
                .saturating_add(u16::try_from(abs_x.max(0)).unwrap_or(0));
            if draw_x.saturating_add(w) > budget_right {
                // Source ChipBar paints `…` at the would-be chip origin even
                // when only one cell remains.
                if draw_x >= area.x && draw_x < buffer.area().right() {
                    buffer.set_stringn(
                        draw_x,
                        area.y,
                        self.system.glyphs.ellipsis(),
                        1,
                        self.system.style(Role::TextMuted),
                    );
                }
                break;
            }
            let rect = Rect {
                x: draw_x,
                y: area.y,
                width: w,
                height: 1.min(area.height),
            };
            self.paint_item(item, rect, buffer, state);
            state.regions.push((item.id.clone(), rect));
        }
        if has_overflow {
            let n = state.overflow_ids.len();
            let label = format!("+{n}");
            let ow = u16::try_from(display_cols(&label).saturating_add(2)).unwrap_or(4);
            let ox = area.right().saturating_sub(ow);
            let rect = Rect {
                x: ox,
                y: area.y,
                width: ow.min(area.width),
                height: 1.min(area.height),
            };
            paint_subtle_chip(self.system, buffer, rect, &label, false);
            state.overflow_region = Some(rect);
        }
        let next_x = state
            .regions
            .last()
            .map(|(_, r)| r.right().saturating_add(self.gap))
            .unwrap_or(area.x);
        let add_right = state.overflow_region.map(|r| r.x).unwrap_or(area.right());
        self.paint_add(
            Rect::new(next_x, area.y, add_right.saturating_sub(next_x), 1),
            buffer,
            state,
            add_right,
        );
    }

    fn paint_wrap(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut TokenStripState<Id>,
        visible: &[TokenItem<'a, Id>],
        has_overflow: bool,
    ) {
        let mut x = area.x;
        let mut y = area.y;
        let bottom = area.bottom();
        for item in visible {
            if y >= bottom {
                break;
            }
            let w = estimate_item_width(item, self.system).min(area.width);
            if x > area.x && x.saturating_add(w) > area.right() {
                x = area.x;
                y = y.saturating_add(1);
                if y >= bottom {
                    break;
                }
            }
            let rect = Rect {
                x,
                y,
                width: w.min(area.right().saturating_sub(x)),
                height: 1,
            };
            self.paint_item(item, rect, buffer, state);
            state.regions.push((item.id.clone(), rect));
            x = x.saturating_add(w).saturating_add(self.gap);
        }
        if has_overflow && y < bottom {
            let n = state.overflow_ids.len();
            let label = format!("+{n}");
            let ow = add_chip_width(&label);
            if x.saturating_add(ow) > area.right() {
                x = area.x;
                y = y.saturating_add(1);
            }
            if y < bottom {
                let rect = Rect {
                    x,
                    y,
                    width: ow.min(area.right().saturating_sub(x)),
                    height: 1,
                };
                paint_subtle_chip(self.system, buffer, rect, &label, false);
                state.overflow_region = Some(rect);
                x = x.saturating_add(ow).saturating_add(self.gap);
            }
        }
        if y < bottom {
            self.paint_add(
                Rect::new(x, y, area.right().saturating_sub(x), 1),
                buffer,
                state,
                area.right(),
            );
        }
    }

    fn paint_item(
        &self,
        item: &TokenItem<'a, Id>,
        rect: Rect,
        buffer: &mut Buffer,
        state: &TokenStripState<Id>,
    ) {
        let focused = state.show_chip_cursor
            && state.surface_focused
            && !state.add_focused
            && state.roving.active() == Some(&item.id);
        let hovered_remove = state.hovered_remove.as_ref() == Some(&item.id);
        if item.selectable {
            let chip = Chip::new(item.id.clone(), item.label, self.system)
                .removable(item.removable)
                .status(item.status)
                .disabled(item.disabled);
            let mut cs = ChipState::new(item.selected);
            cs.set_focused(focused);
            cs.set_hovered_remove(hovered_remove);
            if focused {
                cs.set_part(state.part);
            }
            let _ = chip.paint(rect, buffer, &mut cs);
        } else {
            let tag = Tag::new(item.id.clone(), item.label, self.system)
                .removable(item.removable)
                .status(item.status)
                .disabled(item.disabled);
            let mut ts = TagState::new();
            ts.set_focused(focused);
            ts.hovered_remove = hovered_remove;
            if focused {
                ts.set_part(state.part);
            }
            let _ = tag.paint(rect, buffer, &mut ts);
        }
    }

    fn paint_add(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut TokenStripState<Id>,
        budget_right: u16,
    ) {
        let Some(label) = self.add_label else {
            return;
        };
        let w = add_chip_width(label);
        if w == 0 || area.is_empty() {
            return;
        }
        let x = if area.x >= budget_right {
            return;
        } else {
            area.x
        };
        // Source ChipBar paints add only when the full control fits.
        if x.saturating_add(w) > budget_right {
            return;
        }
        let width = w.min(area.width);
        if width < w {
            return;
        }
        let rect = Rect {
            x,
            y: area.y,
            width,
            height: 1.min(area.height),
        };
        paint_subtle_chip(
            self.system,
            buffer,
            rect,
            label,
            state.surface_focused && state.add_focused,
        );
        state.add_region = Some(rect);
    }

    /// Key path for strip.
    pub fn handle_key(
        &self,
        state: &mut TokenStripState<Id>,
        key: KeyEvent,
    ) -> TokenStripOutcome<Id> {
        if !state.surface_focused || !key.is_press() {
            return TokenStripOutcome::Ignored;
        }
        let entries = self.entries();
        let _ = state.roving.reconcile(&entries);

        // Source ChipBar: ← → move the chip cursor. TokenPart is mouse/Tab-internal,
        // not arrow-internal.
        if matches!(
            key.code,
            KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l')
        ) {
            match state.roving.handle_key(key, &entries) {
                RovingOutcome::Ignored => {}
                RovingOutcome::ActiveChanged { from, to } => {
                    state.part = TokenPart::Body;
                    return TokenStripOutcome::CursorMoved { from, to };
                }
            }
        }

        // Focused token internal part / activate / remove
        if let Some(id) = state.roving.active().cloned() {
            if let Some(item) = self.items.iter().find(|i| i.id == id) {
                // Build ephemeral state
                if item.selectable {
                    let chip = Chip::new(item.id.clone(), item.label, self.system)
                        .removable(item.removable)
                        .status(item.status)
                        .disabled(item.disabled);
                    let mut cs = ChipState::new(item.selected);
                    cs.set_focused(true);
                    cs.set_part(state.part);
                    // Need parts for mouse only; keys use part from state
                    match chip.handle_key(&mut cs, key) {
                        ChipOutcome::Ignored => {}
                        ChipOutcome::Selected(id) => return TokenStripOutcome::Selected(id),
                        ChipOutcome::Unselected(id) => return TokenStripOutcome::Unselected(id),
                        ChipOutcome::Remove(id) => return TokenStripOutcome::Remove(id),
                        ChipOutcome::PartChanged(p) => {
                            state.part = p;
                            return TokenStripOutcome::PartChanged(p);
                        }
                        ChipOutcome::Activated(id) => return TokenStripOutcome::Activated(id),
                        ChipOutcome::HoverChanged => return TokenStripOutcome::HoverChanged,
                    }
                    // If chip ignored Left/Right at edges, fall through to roving
                } else {
                    let tag = Tag::new(item.id.clone(), item.label, self.system)
                        .removable(item.removable)
                        .status(item.status)
                        .disabled(item.disabled);
                    let mut ts = TagState::new();
                    ts.set_focused(true);
                    ts.set_part(state.part);
                    match tag.handle_key(&mut ts, key) {
                        TagOutcome::Ignored => {}
                        TagOutcome::Remove(id) => return TokenStripOutcome::Remove(id),
                        TagOutcome::PartChanged(p) => {
                            state.part = p;
                            return TokenStripOutcome::PartChanged(p);
                        }
                        TagOutcome::Activated(id) => return TokenStripOutcome::Activated(id),
                        TagOutcome::HoverChanged => return TokenStripOutcome::HoverChanged,
                    }
                }
            }
        }

        if state.add_focused {
            match key.code {
                KeyCode::Left | KeyCode::Char('h') => {
                    state.add_focused = false;
                    if let Some(last) = self.items.iter().rev().find(|i| !i.disabled) {
                        state.roving.set_active(Some(last.id.clone()));
                    }
                    return TokenStripOutcome::CursorMoved {
                        from: None,
                        to: state.roving.active().cloned(),
                    };
                }
                KeyCode::Enter | KeyCode::Char(' ') => return TokenStripOutcome::Add,
                _ => return TokenStripOutcome::Ignored,
            }
        }

        // Horizontal scroll
        if matches!(self.layout, TokenStripLayout::Scroll) {
            if matches!(key.code, KeyCode::Home) {
                state.scroll = 0;
            }
        }

        match state.roving.handle_key(key, &entries) {
            RovingOutcome::Ignored => {
                if self.add_label.is_some()
                    && matches!(
                        key.code,
                        KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter
                    )
                    && state.roving.active().is_some()
                    && self
                        .items
                        .iter()
                        .rev()
                        .find(|i| !i.disabled)
                        .is_some_and(|last| state.roving.active() == Some(&last.id))
                {
                    if matches!(key.code, KeyCode::Enter) {
                        return TokenStripOutcome::Ignored;
                    }
                    state.add_focused = true;
                    return TokenStripOutcome::CursorMoved {
                        from: state.roving.active().cloned(),
                        to: None,
                    };
                }
                TokenStripOutcome::Ignored
            }
            RovingOutcome::ActiveChanged { from, to } => {
                state.part = TokenPart::Body;
                state.add_focused = false;
                TokenStripOutcome::CursorMoved { from, to }
            }
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &self,
        state: &mut TokenStripState<Id>,
        event: MouseEvent,
    ) -> TokenStripOutcome<Id> {
        if matches!(event.kind, MouseEventKind::Moved) {
            let mut next = None;
            for (id, rect) in &state.regions {
                if rect.contains(event.position)
                    && event.position.x + 1 >= rect.right().saturating_sub(1)
                    && rect.width > 3
                {
                    next = Some(id.clone());
                    break;
                }
            }
            if next == state.hovered_remove {
                return TokenStripOutcome::Ignored;
            }
            state.hovered_remove = next;
            return TokenStripOutcome::HoverChanged;
        }
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return TokenStripOutcome::Ignored;
        }
        if let Some(r) = state.add_region
            && r.contains(event.position)
        {
            state.surface_focused = true;
            state.add_focused = true;
            return TokenStripOutcome::Add;
        }
        if let Some(r) = state.overflow_region
            && r.contains(event.position)
        {
            state.surface_focused = true;
            return TokenStripOutcome::OverflowActivated;
        }
        for (id, rect) in &state.regions {
            if rect.contains(event.position) {
                state.surface_focused = true;
                state.roving.set_active(Some(id.clone()));
                state.part = TokenPart::Body;
                // Right edge ≈ remove
                if let Some(item) = self.items.iter().find(|i| &i.id == id) {
                    if item.removable
                        && event.position.x + 1 >= rect.right().saturating_sub(1)
                        && rect.width > 3
                    {
                        return TokenStripOutcome::Remove(id.clone());
                    }
                    if item.selectable {
                        // toggle — host applies; we report Selected/Unselected by flipping expected
                        if item.selected {
                            return TokenStripOutcome::Unselected(id.clone());
                        }
                        return TokenStripOutcome::Selected(id.clone());
                    }
                    return TokenStripOutcome::Activated(id.clone());
                }
                return TokenStripOutcome::Activated(id.clone());
            }
        }
        TokenStripOutcome::Ignored
    }
}

fn add_chip_width(label: &str) -> u16 {
    u16::try_from(1 + display_cols(label) + 1).unwrap_or(1)
}

fn paint_subtle_chip(
    system: &DesignSystem,
    buffer: &mut Buffer,
    rect: Rect,
    label: &str,
    focused: bool,
) {
    if rect.is_empty() {
        return;
    }
    let mut style = system.style(Role::TextMuted);
    if focused {
        style = system.style(Role::Text).add_modifier(Modifier::BOLD);
    }
    buffer.set_style(rect, style);
    let gutter = system.glyphs.selection_gutter();
    let gutter_style = if focused {
        style.patch(system.style(Role::Focus))
    } else {
        style
    };
    buffer.set_stringn(rect.x, rect.y, gutter, 1, gutter_style);
    if rect.width > 1 {
        buffer.set_stringn(
            rect.x.saturating_add(1),
            rect.y,
            take_display_cols(label, usize::from(rect.width.saturating_sub(1))),
            usize::from(rect.width.saturating_sub(1)),
            style,
        );
    }
}

fn estimate_item_width<Id: Clone>(item: &TokenItem<'_, Id>, system: &DesignSystem) -> u16 {
    if item.selectable {
        Chip::new(item.id.clone(), item.label, system)
            .removable(item.removable)
            .status(item.status)
            .measure_width(item.selected)
    } else {
        Tag::new(item.id.clone(), item.label, system)
            .removable(item.removable)
            .status(item.status)
            .measure_width()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;
    use crate::style::GlyphSet;

    #[test]
    fn remove_label_explicit() {
        assert_eq!(remove_label("paste"), "remove paste");
    }

    #[test]
    fn tag_remove_and_part_focus() {
        let system = DesignSystem::default();
        let tag = Tag::removable_tag("t1", "file.rs", &system);
        let mut state = TagState::new();
        state.set_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 1));
        let _ = tag.paint(Rect::new(0, 0, 24, 1), &mut buf, &mut state);
        assert!(matches!(
            tag.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)
            ),
            TagOutcome::PartChanged(TokenPart::Remove)
        ));
        assert!(matches!(
            tag.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            ),
            TagOutcome::Remove("t1")
        ));
    }

    #[test]
    fn unselected_chip_sits_on_overlay() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let chip = Chip::new("f", "status = 'pending'", &system).removable(true);
        let mut state = ChipState::new(false);
        let area = Rect::new(0, 0, 24, 1);
        let mut buffer = Buffer::empty(area);
        let _ = chip.paint(area, &mut buffer, &mut state);
        assert_eq!(
            buffer[(1, 0)].bg,
            theme.surface_overlay,
            "unselected chip uses Toggle/Secondary overlay, not surface"
        );
        assert_eq!(buffer[(0, 0)].symbol(), system.glyphs.selection_gutter());
    }

    #[test]
    fn chip_toggle_space() {
        let system = DesignSystem::default();
        let chip = Chip::new("f1", "rust", &system);
        let mut state = ChipState::new(false);
        state.set_focused(true);
        assert!(matches!(
            chip.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)
            ),
            ChipOutcome::Selected("f1")
        ));
        assert!(state.is_selected());
    }

    #[test]
    fn tag_chip_and_strip_mouse_use_painted_regions() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 24, 1);
        let mut buffer = Buffer::empty(area);

        let tag = Tag::removable_tag("tag", "alpha", &system);
        let mut tag_state = TagState::new();
        let parts = tag.paint(area, &mut buffer, &mut tag_state);
        assert!(matches!(
            tag.handle_mouse(
                &mut tag_state,
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: ratatui_core::layout::Position::new(parts.body.x, parts.body.y),
                    modifiers: KeyModifiers::NONE,
                },
            ),
            TagOutcome::Activated("tag")
        ));

        let chip = Chip::new("chip", "beta", &system);
        let mut chip_state = ChipState::new(false);
        let parts = chip.paint(area, &mut buffer, &mut chip_state);
        assert!(matches!(
            chip.handle_mouse(
                &mut chip_state,
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: ratatui_core::layout::Position::new(parts.body.x, parts.body.y),
                    modifiers: KeyModifiers::NONE,
                },
            ),
            ChipOutcome::Selected("chip")
        ));

        let items = [TokenItem::chip("strip", "gamma")];
        let strip = TokenStrip::new(&items, &system);
        let mut strip_state = TokenStripState::new();
        strip.paint(area, &mut buffer, &mut strip_state);
        let hit = strip_state.regions[0].1;
        assert!(matches!(
            strip.handle_mouse(
                &mut strip_state,
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: ratatui_core::layout::Position::new(hit.x, hit.y),
                    modifiers: KeyModifiers::NONE,
                },
            ),
            TokenStripOutcome::Selected("strip")
        ));
    }

    #[test]
    fn non_removable_ignores_delete() {
        let system = DesignSystem::default();
        let tag = Tag::new("t", "static", &system);
        let mut state = TagState::new();
        state.set_focused(true);
        assert!(matches!(
            tag.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE)
            ),
            TagOutcome::Ignored
        ));
    }

    #[test]
    fn token_strip_overflow() {
        let system = DesignSystem::default();
        let items = [
            TokenItem::chip("a", "one"),
            TokenItem::chip("b", "two"),
            TokenItem::chip("c", "three"),
            TokenItem::chip("d", "four"),
        ];
        let strip = TokenStrip::new(&items, &system).max_visible(2);
        let mut state = TokenStripState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        strip.paint(Rect::new(0, 0, 40, 1), &mut buf, &mut state);
        assert_eq!(state.overflow_ids.len(), 2);
        assert!(state.overflow_region.is_some());
    }

    #[test]
    fn token_strip_wrap_and_roving() {
        let system = DesignSystem::default();
        let items = [
            TokenItem::tag("a", "alpha").removable(true),
            TokenItem::chip("b", "beta").selected(true),
        ];
        let strip = TokenStrip::new(&items, &system).wrap();
        let mut state = TokenStripState::new();
        state.set_surface_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
        strip.paint(Rect::new(0, 0, 20, 3), &mut buf, &mut state);
        state.set_cursor(Some("a"));
        let out = strip.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );
        // may be part change or cursor move
        assert!(out != TokenStripOutcome::Ignored || state.part == TokenPart::Remove);
    }

    #[test]
    fn loading_and_error_status() {
        let system = DesignSystem::default();
        let chip = Chip::new("e", "bad", &system).error();
        let mut state = ChipState::new(false);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let _ = chip.paint(Rect::new(0, 0, 20, 1), &mut buf, &mut state);
        assert!(!buf[(1, 0)].symbol().is_empty());
    }

    #[test]
    fn measure_is_cheap() {
        let system = DesignSystem::default();
        let chip = Chip::new("x", "filter", &system).removable(true);
        for _ in 0..20_000 {
            let _ = chip.measure_width(true);
        }
    }

    #[test]
    fn legacy_chip_state_api() {
        let mut s = ChipState::new(true);
        assert!(s.is_selected());
        s.set_selected(false);
        assert!(!s.is_selected());
    }

    #[test]
    fn removable_chip_width_matches_junie_chip_bar() {
        let system = DesignSystem::junie();
        let chip = Chip::new("s", "status = 'pending'", &system).removable(true);
        assert_eq!(
            chip.measure_width(true),
            1 + 18 + 1 + 2 + 1,
            "gutter + label + pad + removable 2 + trail"
        );
        let plain = Chip::new("p", "ok", &system);
        assert_eq!(plain.measure_width(false), 1 + 2 + 1 + 1);
    }

    #[test]
    fn filter_chip_is_gutter_row_not_boxed_pill() {
        let system = DesignSystem::default();
        let chip = Chip::new("rust", "Rust", &system).removable(true);
        let mut state = ChipState::new(true);
        let area = Rect::new(0, 0, 16, 1);
        let mut buffer = Buffer::empty(area);
        let parts = chip.paint(area, &mut buffer, &mut state);
        assert_eq!(
            buffer[(0, 0)].symbol(),
            system.glyphs.selection_gutter(),
            "col0 is the list gutter"
        );
        let row: String = (0..area.width)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            !row.contains('[') && !row.contains(']') && !row.contains('⟨'),
            "chips are not boxed pills: {row:?}"
        );
        assert!(row.contains('×') || row.contains(system.glyphs.resolve(Glyph::Close).text));
        let muted = system.style(Role::TextMuted).fg.expect("text_muted");
        assert_eq!(
            buffer[(parts.remove.x, parts.remove.y)].fg,
            muted,
            "× stays faint until hovered"
        );
        state.hovered_remove = true;
        let _ = chip.paint(area, &mut buffer, &mut state);
        assert_ne!(
            buffer[(parts.remove.x, parts.remove.y)].fg,
            muted,
            "hovered × brightens"
        );
    }

    #[test]
    fn token_strip_keeps_full_chips_and_drops_add_when_tight() {
        let system = DesignSystem::junie();
        let items = [
            TokenItem::chip(0, "status = 'pending'")
                .removable(true)
                .selected(true),
            TokenItem::chip(1, "total > 100")
                .removable(true)
                .selected(true),
            TokenItem::chip(2, "country in (DE, FR)")
                .removable(true)
                .selected(false),
        ];
        let strip = TokenStrip::new(&items, &system).add_label(Some("+ Add filter"));
        let mut state = TokenStripState::new();
        // 42 cells after the lead: three full chips fit, add does not.
        let area = Rect::new(0, 0, 66, 1);
        let mut buffer = Buffer::empty(area);
        strip.paint(area, &mut buffer, &mut state);
        let row: String = (0..area.width)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            row.contains("country in (DE, FR)"),
            "last chip must not clip: {row:?}"
        );
        assert!(!row.contains("Add filter"), "add is leftover-only: {row:?}");
        assert!(state.add_region.is_none(), "add must drop, not clip");
    }

    #[test]
    fn token_strip_does_not_clip_add_into_leftover() {
        let system = DesignSystem::junie();
        let items = [TokenItem::chip(0, "status = 'pending'")
            .removable(true)
            .selected(true)];
        let strip = TokenStrip::new(&items, &system).add_label(Some("+ Add filter"));
        let mut state = TokenStripState::new();
        // Chip 23 + gap 1 = 24; leftover 10 < add 14.
        let area = Rect::new(0, 0, 34, 1);
        let mut buffer = Buffer::empty(area);
        strip.paint(area, &mut buffer, &mut state);
        let row: String = (0..area.width)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            !row.contains('+') && !row.contains("Add"),
            "clipped add must not paint: {row:?}"
        );
        assert!(state.add_region.is_none());
    }

    #[test]
    fn token_strip_paints_subtle_add_filter() {
        let system = DesignSystem::default();
        let items = [TokenItem::chip("a", "Rust").removable(true)];
        let strip = TokenStrip::new(&items, &system);
        let mut state = TokenStripState::new();
        let area = Rect::new(0, 0, 40, 1);
        let mut buffer = Buffer::empty(area);
        strip.paint(area, &mut buffer, &mut state);
        let row: String = (0..area.width)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            row.contains("Add filter") || row.contains('+'),
            "add affordance: {row:?}"
        );
        assert!(state.add_region.is_some());
    }
}

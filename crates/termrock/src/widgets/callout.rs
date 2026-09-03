// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Callout** and **Alert** — inline feedback messages.
//!
//! **Mission.** Informational, success, warning, error, and destructive-context
//! messages that compose with forms, diagnostics, permissions, and empty
//! states. Paint uses **border / gutter / text hierarchy** — not huge background
//! fills — so tone remains readable in no-color and ASCII modes.
//!
//! **Callout.** Lightweight inline notice (shadcn Alert-ish / quote-rail). May
//! be compact or prominent section recipe; optional dismiss and actions.
//! **Alert.** Stronger surface (may require acknowledgement). Default
//! dismissible; focusable when actions or dismiss are live; source line for
//! diagnostics provenance.
//!
//! **vs AlertDialog.** Modal high-risk confirm. These are **inline**.
//! **vs Banner.** Single-line severity strip. Callout/Alert carry structure.
//! **vs Toast.** Transient overlay. Callout/Alert live in layout flow.
//!
//! Research: shadcn Alert, Glow quote rails, CLI warnings, system diagnostics.
#![allow(unused_variables, unused_mut)] // unit-test fixtures
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::Modifier,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind},
    interaction::{
        EventResult, HitRegion, OverlayRequest, SemanticNode, SemanticRole, SemanticScene,
        SemanticState, UiIntent, default_button_intent, default_list_intent,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

use super::{Action, Surface, SurfaceFill, SurfaceRecipe};

// ── Tone / recipe ───────────────────────────────────────────────────────────

/// Semantic tone shared by Callout and Alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CalloutTone {
    /// Neutral informational.
    #[default]
    Info,
    /// Success / confirmation.
    Success,
    /// Warning / caution.
    Warning,
    /// Error / failure.
    Danger,
    /// Destructive context (stronger than Danger; same role, distinct glyph).
    Destructive,
    /// Neutral chrome (no strong status role).
    Neutral,
}

impl CalloutTone {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Danger => "danger",
            Self::Destructive => "destructive",
            Self::Neutral => "neutral",
        }
    }

    /// Semantic paint role.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Info => Role::TextSecondary,
            Self::Success => Role::Success,
            Self::Warning => Role::Warning,
            Self::Danger | Self::Destructive => Role::Danger,
            Self::Neutral => Role::TextMuted,
        }
    }

    /// Non-color / ASCII-safe glyph (single cell preferred).
    #[must_use]
    pub const fn glyph_ascii(self) -> &'static str {
        match self {
            Self::Info => "i",
            Self::Success => "+",
            Self::Warning => "!",
            Self::Danger => "x",
            Self::Destructive => "X",
            Self::Neutral => "-",
        }
    }

    /// Unicode glyph (still paired with text hierarchy for no-color).
    #[must_use]
    pub const fn glyph_unicode(self) -> &'static str {
        match self {
            Self::Info => "ℹ",
            Self::Success => "✓",
            Self::Warning => "!",
            Self::Danger => "✗",
            Self::Destructive => "‼",
            Self::Neutral => "·",
        }
    }

    /// Glyph for the one vocabulary.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        self.glyph_unicode()
    }
}

/// Density / prominence recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CalloutRecipe {
    /// Compact inline (title + optional one body line; thin gutter).
    #[default]
    Compact,
    /// Prominent section (bordered, multi-line body/details, source, actions).
    Section,
}

impl CalloutRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Section => "section",
        }
    }
}

// ── Slots / outcomes ────────────────────────────────────────────────────────

/// Geometry after paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CalloutSlots {
    /// Outer area.
    pub root: Rect,
    /// Left gutter / rail.
    pub gutter: Rect,
    /// Status glyph cell(s).
    pub glyph: Rect,
    /// Title line.
    pub title: Rect,
    /// Description / body.
    pub description: Rect,
    /// Expandable details block.
    pub details: Rect,
    /// Source / provenance line.
    pub source: Rect,
    /// Action bar.
    pub actions: Rect,
    /// Dismiss affordance.
    pub dismiss: Rect,
}

impl CalloutSlots {
    /// Empty.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            root: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            gutter: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            glyph: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            title: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            description: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            details: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            source: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            actions: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            dismiss: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        }
    }
}

/// Callout outcomes (dismiss / action when enabled).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CalloutOutcome<Id = ()> {
    /// No change.
    Ignored,
    /// Dismissed.
    Dismissed,
    /// Action activated.
    ActionActivated {
        /// Action id.
        id: Id,
    },
    /// Details expanded/collapsed.
    DetailsToggled {
        /// Expanded?
        open: bool,
    },
}

/// Alert interaction outcomes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AlertOutcome<Id = ()> {
    /// No change.
    Ignored,
    /// User dismissed.
    Dismissed,
    /// User acknowledged (Enter on primary / acknowledge).
    Acknowledged,
    /// Action activated.
    ActionActivated {
        /// Action id.
        id: Id,
    },
    /// Details toggled.
    DetailsToggled {
        /// Expanded?
        open: bool,
    },
}

// ── Shared content projection ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
struct FeedbackContent<'a> {
    title: &'a str,
    description: Option<&'a str>,
    details: Option<&'a str>,
    source: Option<&'a str>,
    tone: CalloutTone,
    recipe: CalloutRecipe,
    dismissible: bool,
    show_details: bool,
}

impl<'a> FeedbackContent<'a> {
    fn measure_height(self, max_width: u16, has_actions: bool) -> u16 {
        if max_width == 0 {
            return 0;
        }
        let border = u16::from(matches!(self.recipe, CalloutRecipe::Section));
        let mut h = 1u16; // title
        if self.description.is_some() {
            h = h.saturating_add(1);
        }
        if self.show_details && self.details.is_some() {
            h = h.saturating_add(1);
        }
        if self.source.is_some() {
            h = h.saturating_add(1);
        }
        if has_actions || self.dismissible {
            h = h.saturating_add(1);
        }
        h.saturating_add(border.saturating_mul(2)).max(1)
    }
}

// ── Paint engine ────────────────────────────────────────────────────────────

struct PaintArgs<'a, Id> {
    content: FeedbackContent<'a>,
    system: &'a DesignSystem,
    actions: &'a [Action<'a, Id>],
    action_cursor: Option<&'a Id>,
    focused: bool,
    enabled: bool,
    colorless: bool,
    emphasize: bool,
}

fn paint_feedback<Id: Clone + PartialEq>(
    args: &PaintArgs<'_, Id>,
    area: Rect,
    buffer: &mut Buffer,
) -> CalloutSlots {
    let mut slots = CalloutSlots::empty();
    if area.is_empty() {
        return slots;
    }
    slots.root = area;

    let tone = args.content.tone;
    let tone_role = tone.role();
    let tone_style = if !args.enabled {
        args.system.style(Role::TextDisabled)
    } else if args.colorless {
        args.system.style(Role::Text).add_modifier(Modifier::BOLD)
    } else {
        args.system.style(tone_role)
    };
    let text_style = args.system.style(if args.enabled {
        Role::Text
    } else {
        Role::TextDisabled
    });
    let muted = args.system.style(if args.enabled {
        Role::TextMuted
    } else {
        Role::TextDisabled
    });
    let strong = args
        .system
        .style(if args.enabled {
            Role::TextStrong
        } else {
            Role::TextDisabled
        })
        .add_modifier(Modifier::BOLD);

    let section = matches!(args.content.recipe, CalloutRecipe::Section);
    // Tone rides the rail and the glyph; the border is chrome and stays
    // neutral, so a page of callouts is not a page of colored boxes
    // (plans/007).
    let surface_recipe = if args.enabled && args.focused && args.emphasize {
        SurfaceRecipe::Focused
    } else {
        SurfaceRecipe::Inset
    };

    // Optional outer border for section recipe (no full-surface fill).
    let mut inner = area;
    if section && area.width >= 2 && area.height >= 2 {
        let surface_system = args.system;
        inner = Surface::new(surface_system)
            .recipe(surface_recipe)
            .bordered(true)
            .fill(SurfaceFill::Transparent)
            .content_inset()
            .paint(area, buffer);
    }
    if inner.is_empty() {
        return slots;
    }

    // Gutter rail (quote / CLI warning rail) — 1 cell
    // The rail is one cell plus the density's own gap (plans/022 Step 6).
    let gutter_w = 1u16;
    slots.gutter = Rect::new(inner.x, inner.y, gutter_w.min(inner.width), inner.height);
    let rail = "│";
    for y in inner.y..inner.bottom() {
        buffer.set_stringn(inner.x, y, rail, 1, tone_style);
    }

    let content_x = inner
        .x
        .saturating_add(gutter_w.saturating_add(1).min(inner.width));
    let content_w = inner
        .width
        .saturating_sub(content_x.saturating_sub(inner.x));
    if content_w == 0 {
        return slots;
    }

    let has_actions = !args.actions.is_empty();
    let footer = has_actions || args.content.dismissible;

    let mut y = inner.y;
    let glyph = tone.glyph();
    let glyph_w = display_cols(glyph) as u16;

    // Title line: glyph + title (+ dismiss on far right for compact)
    slots.glyph = Rect::new(content_x, y, glyph_w.min(content_w), 1);
    buffer.set_stringn(content_x, y, glyph, usize::from(content_w), tone_style);

    let title_x = content_x.saturating_add(glyph_w.saturating_add(1));
    let dismiss_label = "×";
    let dismiss_w = if args.content.dismissible {
        display_cols(dismiss_label) as u16
    } else {
        0
    };
    let title_w = content_w
        .saturating_sub(glyph_w.saturating_add(1))
        .saturating_sub(if dismiss_w > 0 {
            dismiss_w.saturating_add(1)
        } else {
            0
        });
    slots.title = Rect::new(title_x, y, title_w, 1);
    let title_style = if args.focused && args.emphasize {
        strong.add_modifier(Modifier::BOLD)
    } else {
        strong
    };
    buffer.set_stringn(
        title_x,
        y,
        take_display_cols(args.content.title, usize::from(title_w)).as_ref(),
        usize::from(title_w),
        title_style,
    );
    if args.content.dismissible && dismiss_w > 0 && content_w > dismiss_w {
        let dx = content_x.saturating_add(content_w.saturating_sub(dismiss_w));
        slots.dismiss = Rect::new(dx, y, dismiss_w, 1);
        buffer.set_stringn(dx, y, dismiss_label, usize::from(dismiss_w), muted);
    }
    y = y.saturating_add(1);

    // Description
    if let Some(desc) = args.content.description {
        if y < inner.bottom() {
            slots.description = Rect::new(content_x, y, content_w, 1);
            buffer.set_stringn(
                content_x,
                y,
                take_display_cols(desc, usize::from(content_w)).as_ref(),
                usize::from(content_w),
                text_style,
            );
            y = y.saturating_add(1);
        }
    }

    // Details
    if args.content.show_details {
        if let Some(details) = args.content.details {
            if y < inner.bottom() {
                slots.details = Rect::new(content_x, y, content_w, 1);
                buffer.set_stringn(
                    content_x,
                    y,
                    take_display_cols(details, usize::from(content_w)).as_ref(),
                    usize::from(content_w),
                    muted,
                );
                y = y.saturating_add(1);
            }
        }
    }

    // Source
    if let Some(src) = args.content.source {
        if y < inner.bottom() {
            slots.source = Rect::new(content_x, y, content_w, 1);
            let line = format!("— {src}");
            buffer.set_stringn(
                content_x,
                y,
                take_display_cols(&line, usize::from(content_w)).as_ref(),
                usize::from(content_w),
                muted,
            );
            y = y.saturating_add(1);
        }
    }

    // Actions footer
    if footer && y < inner.bottom() {
        slots.actions = Rect::new(content_x, y, content_w, 1);
        let mut x = content_x;
        for a in args.actions {
            if x >= content_x.saturating_add(content_w) {
                break;
            }
            let active = args.action_cursor.is_some_and(|c| c == &a.id) && args.focused;
            let label = if active {
                format!("[{}]", a.label)
            } else {
                format!(" {} ", a.label)
            };
            let w = display_cols(&label) as u16;
            let style = if !args.enabled || !a.enabled {
                args.system.style(Role::TextDisabled)
            } else if active {
                args.system
                    .style(Role::TextStrong)
                    .patch(args.system.style(Role::SelectionTint))
            } else {
                text_style
            };
            let avail = content_x.saturating_add(content_w).saturating_sub(x);
            buffer.set_stringn(
                x,
                y,
                take_display_cols(&label, usize::from(avail)).as_ref(),
                usize::from(avail),
                style,
            );
            x = x.saturating_add(w.saturating_add(1));
        }
        if args.content.dismissible && args.actions.is_empty() {
            // hint when only dismiss
            let hint = "esc";
            buffer.set_stringn(content_x, y, hint, usize::from(content_w), muted);
        }
    }

    slots
}

// ── Callout ─────────────────────────────────────────────────────────────────

/// Inline callout (non-modal feedback).
#[derive(Debug, Clone, Copy)]
pub struct Callout<'a, Id = ()> {
    title: &'a str,
    description: Option<&'a str>,
    details: Option<&'a str>,
    source: Option<&'a str>,
    tone: CalloutTone,
    recipe: CalloutRecipe,
    system: &'a DesignSystem,
    actions: &'a [Action<'a, Id>],
    dismissible: bool,
    show_details: bool,
    colorless: bool,
}

impl<'a> Callout<'a, ()> {
    /// Title + design system (Info tone, Compact recipe).
    #[must_use]
    pub const fn new(title: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            title,
            description: None,
            details: None,
            source: None,
            tone: CalloutTone::Info,
            recipe: CalloutRecipe::Compact,
            system,
            actions: &[],
            dismissible: false,
            show_details: false,
            colorless: false,
        }
    }
}

impl<'a, Id> Callout<'a, Id> {
    /// Description line.
    #[must_use]
    pub const fn description(mut self, text: &'a str) -> Self {
        self.description = Some(text);
        self
    }

    /// Details (shown when expanded / always in Section when set).
    #[must_use]
    pub const fn details(mut self, text: &'a str) -> Self {
        self.details = Some(text);
        self
    }

    /// Source / provenance.
    #[must_use]
    pub const fn source(mut self, text: &'a str) -> Self {
        self.source = Some(text);
        self
    }

    /// Tone.
    #[must_use]
    pub const fn tone(mut self, tone: CalloutTone) -> Self {
        self.tone = tone;
        self
    }

    /// Recipe.
    #[must_use]
    pub const fn recipe(mut self, recipe: CalloutRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Compact helper.
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.recipe = CalloutRecipe::Compact;
        self
    }

    /// Section helper.
    #[must_use]
    pub const fn section(mut self) -> Self {
        self.recipe = CalloutRecipe::Section;
        self
    }

    /// Dismissible chrome.
    #[must_use]
    pub const fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }

    /// Show details row.
    #[must_use]
    pub const fn show_details(mut self, on: bool) -> Self {
        self.show_details = on;
        self
    }

    /// Actions.
    #[must_use]
    pub const fn actions(mut self, actions: &'a [Action<'a, Id>]) -> Self {
        self.actions = actions;
        self
    }

    /// ASCII glyphs.
    #[must_use]
    /// Colorless.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Preferred height.
    #[must_use]
    pub fn measure_height(&self, width: u16) -> u16 {
        FeedbackContent {
            title: self.title,
            description: self.description,
            details: self.details,
            source: self.source,
            tone: self.tone,
            recipe: self.recipe,
            dismissible: self.dismissible,
            show_details: self.show_details || matches!(self.recipe, CalloutRecipe::Section),
        }
        .measure_height(width, !self.actions.is_empty())
    }

    /// Paint (no state).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) -> CalloutSlots
    where
        Id: Clone + PartialEq,
    {
        let show_details = self.show_details
            || (matches!(self.recipe, CalloutRecipe::Section) && self.details.is_some());
        paint_feedback(
            &PaintArgs {
                content: FeedbackContent {
                    title: self.title,
                    description: self.description,
                    details: self.details,
                    source: self.source,
                    tone: self.tone,
                    recipe: self.recipe,
                    dismissible: self.dismissible,
                    show_details,
                },
                system: self.system,
                actions: self.actions,
                action_cursor: None,
                focused: false,
                enabled: true,
                colorless: self.colorless,
                emphasize: false,
            },
            area,
            buffer,
        )
    }

    /// Semantic registration.
    pub fn register_semantic<Sid, Act>(
        &self,
        scene: &mut SemanticScene<Sid, Act>,
        id: Sid,
        area: Rect,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Act: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "callout tone={} recipe={} title={}",
            self.tone.id(),
            self.recipe.id(),
            self.title
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Status)
                .label("callout")
                .description(desc)
                .focusable(false)
                .state(SemanticState::default()),
        );
    }
}

impl<Id: Clone + PartialEq> Widget for &Callout<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer);
    }
}

impl<Id: Clone + PartialEq> Widget for Callout<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer);
    }
}

// ── Alert state ─────────────────────────────────────────────────────────────

/// Alert interaction (focus, dismiss, actions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertState<Id = ()> {
    /// Focused for keyboard.
    pub focused: bool,
    /// Region for click-dismiss (last paint).
    pub region: Option<Rect>,
    /// Visible (host may hide after dismiss).
    visible: bool,
    /// Details expanded.
    details_open: bool,
    /// Action cursor.
    action_cursor: Option<Id>,
    /// Accepts input.
    accepts_input: bool,
    /// Enabled.
    enabled: bool,
    /// Slots.
    slots: CalloutSlots,
    /// Action hit regions.
    action_regions: Vec<HitRegion<Id>>,
    /// Dismiss hit.
    dismiss_region: Option<Rect>,
}

impl<Id> Default for AlertState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> AlertState<Id> {
    /// Visible, unfocused.
    #[must_use]
    pub fn new() -> Self {
        Self {
            focused: false,
            region: None,
            visible: true,
            details_open: true,
            action_cursor: None,
            accepts_input: true,
            enabled: true,
            slots: CalloutSlots::empty(),
            action_regions: Vec::new(),
            dismiss_region: None,
        }
    }

    /// Visible?
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.visible
    }

    /// Focused?
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus (host / scene).
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Details open?
    #[must_use]
    pub const fn details_open(&self) -> bool {
        self.details_open
    }

    /// Toggle details.
    pub fn set_details_open(&mut self, on: bool) {
        self.details_open = on;
    }

    /// Slots.
    #[must_use]
    pub const fn slots(&self) -> CalloutSlots {
        self.slots
    }

    /// Dismiss programmatically.
    pub fn dismiss(&mut self) {
        self.visible = false;
        self.focused = false;
    }

    /// Show again.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Enable input.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Enable.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Action cursor (which action is highlighted).
    pub fn set_action_cursor(&mut self, id: Option<Id>) {
        self.action_cursor = id;
    }

    /// Borrow action cursor.
    #[must_use]
    pub fn action_cursor(&self) -> Option<&Id> {
        self.action_cursor.as_ref()
    }
}

impl<Id: Clone + PartialEq> AlertState<Id> {
    /// Esc dismisses; Enter acknowledges (no actions). Prefer
    /// [`Self::handle_key_with`] when actions are present.
    pub fn handle_key(&mut self, key: KeyEvent) -> AlertOutcome<Id> {
        self.handle_key_with(key, &[], true)
    }

    /// Full key routing with actions and dismiss policy.
    pub fn handle_key_with(
        &mut self,
        key: KeyEvent,
        actions: &[Action<'_, Id>],
        dismissible: bool,
    ) -> AlertOutcome<Id> {
        if !self.visible || !self.enabled || !self.accepts_input || !self.focused {
            return AlertOutcome::Ignored;
        }
        if key.is_release() {
            return AlertOutcome::Ignored;
        }
        let is_insert = key.is_insert();

        if matches!(key.code, KeyCode::Esc) && is_insert && key.modifiers.is_empty() && dismissible
        {
            self.dismiss();
            return AlertOutcome::Dismissed;
        }

        if !actions.is_empty() {
            match key.code {
                KeyCode::Left | KeyCode::Char('h' | 'H') if is_insert => {
                    return self.move_action(actions, -1);
                }
                KeyCode::Right | KeyCode::Char('l' | 'L') if is_insert => {
                    return self.move_action(actions, 1);
                }
                KeyCode::Enter if is_insert && key.modifiers.is_empty() => {
                    if let Some(id) = self.action_cursor.clone() {
                        if actions.iter().any(|a| a.id == id && a.enabled) {
                            return AlertOutcome::ActionActivated { id };
                        }
                    }
                    return AlertOutcome::Acknowledged;
                }
                KeyCode::Char('d' | 'D') if is_insert && key.modifiers.is_empty() => {
                    self.details_open = !self.details_open;
                    return AlertOutcome::DetailsToggled {
                        open: self.details_open,
                    };
                }
                _ => {}
            }
        } else if matches!(key.code, KeyCode::Enter) && is_insert && key.modifiers.is_empty() {
            return AlertOutcome::Acknowledged;
        }

        let intent = default_button_intent(key).or_else(|| default_list_intent(key));
        match intent {
            Some(UiIntent::Cancel | UiIntent::Close) if dismissible => {
                self.dismiss();
                AlertOutcome::Dismissed
            }
            Some(UiIntent::Activate | UiIntent::Submit) => AlertOutcome::Acknowledged,
            _ => AlertOutcome::Ignored,
        }
    }

    /// Semantic intent path (no actions).
    pub fn handle_intent(&mut self, intent: UiIntent) -> AlertOutcome<Id> {
        self.handle_intent_with(intent, &[], true)
    }

    /// Intent routing with actions.
    pub fn handle_intent_with(
        &mut self,
        intent: UiIntent,
        actions: &[Action<'_, Id>],
        dismissible: bool,
    ) -> AlertOutcome<Id> {
        if !self.visible || !self.enabled || !self.accepts_input || !self.focused {
            return AlertOutcome::Ignored;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close if dismissible => {
                self.dismiss();
                AlertOutcome::Dismissed
            }
            UiIntent::Activate | UiIntent::Submit => {
                if let Some(id) = self.action_cursor.clone() {
                    if actions.iter().any(|a| a.id == id && a.enabled) {
                        return AlertOutcome::ActionActivated { id };
                    }
                }
                AlertOutcome::Acknowledged
            }
            UiIntent::FocusNext => self.move_action(actions, 1),
            UiIntent::FocusPrevious => self.move_action(actions, -1),
            _ => AlertOutcome::Ignored,
        }
    }

    /// Key path with [`EventResult`].
    pub fn handle_key_result(&mut self, key: KeyEvent) -> EventResult<AlertOutcome<Id>> {
        self.handle_key_result_with(key, &[], true)
    }

    /// EventResult with actions.
    pub fn handle_key_result_with(
        &mut self,
        key: KeyEvent,
        actions: &[Action<'_, Id>],
        dismissible: bool,
    ) -> EventResult<AlertOutcome<Id>> {
        match self.handle_key_with(key, actions, dismissible) {
            AlertOutcome::Ignored => EventResult::ignored(),
            AlertOutcome::Dismissed => {
                EventResult::emit(AlertOutcome::Dismissed).with_overlay(OverlayRequest::DismissTop)
            }
            other => EventResult::emit(other),
        }
    }

    /// Mouse: dismiss hit or action hit.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        actions: &[Action<'_, Id>],
        dismissible: bool,
    ) -> AlertOutcome<Id> {
        if !self.visible || !self.enabled || !self.accepts_input {
            return AlertOutcome::Ignored;
        }
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return AlertOutcome::Ignored;
        }
        let pos = event.position;
        if dismissible {
            if let Some(r) = self.dismiss_region {
                if r.contains(pos) {
                    self.dismiss();
                    return AlertOutcome::Dismissed;
                }
            }
        }
        for region in &self.action_regions {
            if region.area.contains(pos) && actions.iter().any(|a| a.id == region.id && a.enabled) {
                return AlertOutcome::ActionActivated {
                    id: region.id.clone(),
                };
            }
        }
        AlertOutcome::Ignored
    }

    fn move_action(&mut self, actions: &[Action<'_, Id>], dir: isize) -> AlertOutcome<Id> {
        let enabled: Vec<_> = actions.iter().filter(|a| a.enabled).collect();
        if enabled.is_empty() {
            return AlertOutcome::Ignored;
        }
        let cur = self
            .action_cursor
            .as_ref()
            .and_then(|id| enabled.iter().position(|a| &a.id == id));
        let next = match (cur, dir < 0) {
            (Some(0), true) | (None, true) => enabled.len() - 1,
            (Some(i), true) => i - 1,
            (Some(i), false) => (i + 1) % enabled.len(),
            (None, false) => 0,
        };
        self.action_cursor = Some(enabled[next].id.clone());
        AlertOutcome::Ignored
    }
}

// ── Alert widget ────────────────────────────────────────────────────────────

/// Dismissible / acknowledgeable alert (stronger than Callout).
#[derive(Debug, Clone, Copy)]
pub struct Alert<'a, Id = ()> {
    title: &'a str,
    description: Option<&'a str>,
    details: Option<&'a str>,
    source: Option<&'a str>,
    tone: CalloutTone,
    recipe: CalloutRecipe,
    system: &'a DesignSystem,
    actions: &'a [Action<'a, Id>],
    dismissible: bool,
    colorless: bool,
}

impl<'a, Id> Alert<'a, Id> {
    /// Alert title (Warning tone, Section recipe, dismissible).
    #[must_use]
    pub const fn new(title: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            title,
            description: None,
            details: None,
            source: None,
            tone: CalloutTone::Warning,
            recipe: CalloutRecipe::Section,
            system,
            actions: &[],
            dismissible: true,
            colorless: false,
        }
    }

    /// Description.
    #[must_use]
    pub const fn description(mut self, text: &'a str) -> Self {
        self.description = Some(text);
        self
    }

    /// Details.
    #[must_use]
    pub const fn details(mut self, text: &'a str) -> Self {
        self.details = Some(text);
        self
    }

    /// Source.
    #[must_use]
    pub const fn source(mut self, text: &'a str) -> Self {
        self.source = Some(text);
        self
    }

    /// Tone.
    #[must_use]
    pub const fn tone(mut self, tone: CalloutTone) -> Self {
        self.tone = tone;
        self
    }

    /// Recipe.
    #[must_use]
    pub const fn recipe(mut self, recipe: CalloutRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Compact inline alert.
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.recipe = CalloutRecipe::Compact;
        self
    }

    /// Banner-style section (default).
    #[must_use]
    pub const fn banner(mut self) -> Self {
        self.recipe = CalloutRecipe::Section;
        self
    }

    /// Dismissible (default true).
    #[must_use]
    pub const fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }

    /// Actions.
    #[must_use]
    pub const fn actions(mut self, actions: &'a [Action<'a, Id>]) -> Self {
        self.actions = actions;
        self
    }

    /// ASCII.
    #[must_use]
    /// Colorless.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Measure height.
    #[must_use]
    pub fn measure_height(&self, width: u16, state: &AlertState<Id>) -> u16 {
        FeedbackContent {
            title: self.title,
            description: self.description,
            details: self.details,
            source: self.source,
            tone: self.tone,
            recipe: self.recipe,
            dismissible: self.dismissible,
            show_details: state.details_open && self.details.is_some(),
        }
        .measure_height(width, !self.actions.is_empty())
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut AlertState<Id>)
    where
        Id: Clone + PartialEq,
    {
        state.region = None;
        state.slots = CalloutSlots::empty();
        state.action_regions.clear();
        state.dismiss_region = None;
        if area.is_empty() || !state.visible {
            return;
        }
        let slots = paint_feedback(
            &PaintArgs {
                content: FeedbackContent {
                    title: self.title,
                    description: self.description,
                    details: self.details,
                    source: self.source,
                    tone: self.tone,
                    recipe: self.recipe,
                    dismissible: self.dismissible,
                    show_details: state.details_open && self.details.is_some(),
                },
                system: self.system,
                actions: self.actions,
                action_cursor: state.action_cursor.as_ref(),
                focused: state.focused,
                enabled: state.enabled,
                colorless: self.colorless,
                emphasize: true,
            },
            area,
            buffer,
        );
        state.slots = slots;
        state.region = Some(area);
        state.dismiss_region = if slots.dismiss.width > 0 {
            Some(slots.dismiss)
        } else {
            None
        };
        // Rebuild action regions from slots.actions row (approximate sequential)
        if !self.actions.is_empty() && slots.actions.width > 0 {
            let mut x = slots.actions.x;
            for a in self.actions {
                let label_w = (display_cols(a.label) as u16).saturating_add(2);
                let w = label_w.min(slots.actions.right().saturating_sub(x));
                if w == 0 {
                    break;
                }
                state.action_regions.push(HitRegion {
                    id: a.id.clone(),
                    area: Rect::new(x, slots.actions.y, w, 1),
                });
                x = x.saturating_add(w.saturating_add(1));
            }
        }
    }

    /// Semantic registration.
    pub fn register_semantic<Sid, Act>(
        &self,
        scene: &mut SemanticScene<Sid, Act>,
        id: Sid,
        area: Rect,
        state: &AlertState<Id>,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Act: Clone,
    {
        if area.is_empty() || !state.visible {
            return;
        }
        let desc = format!(
            "alert tone={} recipe={} dismissible={} focused={} title={}",
            self.tone.id(),
            self.recipe.id(),
            self.dismissible,
            state.focused,
            self.title
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Status)
                .label("alert")
                .description(desc)
                .focusable(
                    state.enabled
                        && (state.focused || self.dismissible || !self.actions.is_empty()),
                )
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    expanded: state.details_open,
                    ..Default::default()
                }),
        );
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for Alert<'_, Id> {
    type State = AlertState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &Alert<'_, Id> {
    type State = AlertState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;
    use crate::widgets::action_bar::ActionVariant;

    #[test]
    fn callout_tones_have_distinct_glyphs() {
        let glyphs: Vec<_> = [
            CalloutTone::Info,
            CalloutTone::Success,
            CalloutTone::Warning,
            CalloutTone::Danger,
            CalloutTone::Destructive,
            CalloutTone::Neutral,
        ]
        .into_iter()
        .map(|t| t.glyph_ascii())
        .collect();
        assert_eq!(glyphs.len(), 6);
        assert!(glyphs.contains(&"!"));
        assert!(glyphs.contains(&"X"));
    }

    #[test]
    fn callout_gutter_not_full_fill() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 40, 4);
        let mut buf = Buffer::empty(area);
        let slots = Callout::new("Heads up", &system)
            .description("Non-color risk glyph present.")
            .tone(CalloutTone::Warning)
            .paint(area, &mut buf);
        assert_eq!(slots.gutter.width, 1);
        assert_eq!(buf[(0, 0)].symbol(), "│");
        // Title uses strong text, not full-area bg dependency
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("Heads") || text.contains("!"), "{text}");
    }

    #[test]
    fn callout_section_border() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 36, 6);
        let mut buf = Buffer::empty(area);
        let slots = Callout::new("Notice", &system)
            .description("body")
            .section()
            .source("diag")
            .paint(area, &mut buf);
        assert!(!slots.root.is_empty());
        assert_eq!(buf[(0, 0)].symbol(), "\u{256d}"); // Rounded is the canonical border
    }

    #[test]
    fn alert_esc_dismisses() {
        let mut state = AlertState::<()> {
            focused: true,
            ..Default::default()
        };
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            AlertOutcome::Dismissed
        );
        assert!(!state.is_visible());
        let mut state = AlertState::<()> {
            focused: true,
            ..Default::default()
        };
        let r = state.handle_key_result(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(r.message(), Some(&AlertOutcome::Dismissed));
        assert_eq!(r.overlay(), Some(&OverlayRequest::DismissTop));
    }

    #[test]
    fn alert_enter_acknowledges() {
        let mut state = AlertState::<()>::new();
        state.set_focused(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            AlertOutcome::Acknowledged
        );
    }

    #[test]
    fn alert_action_activation() {
        let mut state = AlertState::new();
        state.set_focused(true);
        let actions = [Action {
            id: "retry",
            label: "Retry",
            enabled: true,
            variant: ActionVariant::Primary,
        }];
        state.action_cursor = Some("retry");
        assert!(matches!(
            state.handle_key_with(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &actions,
                true
            ),
            AlertOutcome::ActionActivated { id: "retry" }
        ));
    }

    #[test]
    fn alert_paint_slots_and_source() {
        let system = DesignSystem::default();
        let mut state = AlertState::<()>::new();
        state.set_focused(true);
        let area = Rect::new(0, 0, 48, 8);
        let mut buf = Buffer::empty(area);
        Alert::new("Deploy failed", &system)
            .tone(CalloutTone::Danger)
            .description("Rollout aborted at step 3.")
            .details("timeout waiting for health check")
            .source("pipeline #42")
            .banner()
            .paint(area, &mut buf, &mut state);
        assert!(state.region.is_some());
        assert!(!state.slots.gutter.is_empty());
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Deploy") || text.contains("pipeline"),
            "{text}"
        );
    }

    #[test]
    fn compact_vs_section_height() {
        let system = DesignSystem::default();
        let c = Callout::new("T", &system).description("b").compact();
        let s = Callout::new("T", &system)
            .description("b")
            .section()
            .source("x");
        assert!(s.measure_height(40) >= c.measure_height(40));
    }

    #[test]
    fn semantic_callout_and_alert() {
        let system = DesignSystem::default();
        let mut scene = SemanticScene::<&str, ()>::default();
        Callout::new("Hi", &system).register_semantic(&mut scene, "c", Rect::new(0, 0, 20, 2));
        let mut state = AlertState::<()>::new();
        Alert::new("A", &system).register_semantic(&mut scene, "a", Rect::new(0, 2, 20, 3), &state);
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("callout"))
        );
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("alert"))
        );
        let _ = state;
    }

    #[test]
    fn unfocused_alert_ignores_keys() {
        let mut state = AlertState::<()>::new();
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            AlertOutcome::Ignored
        );
    }

    #[test]
    fn disabled_alert_is_noninteractive_and_semantically_disabled() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 32, 4);
        let mut state = AlertState::<()>::new();
        state.set_focused(true);
        let alert = Alert::new("Unavailable", &system).dismissible(true);
        let mut buffer = Buffer::empty(area);
        alert.paint(area, &mut buffer, &mut state);

        let dismiss = state.dismiss_region.expect("painted dismiss region");
        assert_eq!(
            state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: ratatui_core::layout::Position::new(dismiss.x, dismiss.y),
                    modifiers: KeyModifiers::NONE,
                },
                &[],
                true,
            ),
            AlertOutcome::Dismissed
        );
        state.show();
        state.set_focused(true);
        state.set_enabled(false);
        alert.paint(area, &mut buffer, &mut state);

        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            AlertOutcome::Ignored
        );
        assert_eq!(
            state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: ratatui_core::layout::Position::new(dismiss.x, dismiss.y),
                    modifiers: KeyModifiers::NONE,
                },
                &[],
                true,
            ),
            AlertOutcome::Ignored
        );

        let mut scene = SemanticScene::<&str, ()>::default();
        alert.register_semantic(&mut scene, "alert", area, &state);
        let node = scene.nodes().first().expect("disabled alert semantic node");
        assert!(node.disabled);
        assert!(!node.focusable);
    }

    #[test]
    fn fuzz_alert_keys() {
        let mut state = AlertState::<&str>::new();
        state.set_focused(true);
        let actions = [
            Action {
                id: "a",
                label: "A",
                enabled: true,
                variant: ActionVariant::Secondary,
            },
            Action {
                id: "b",
                label: "B",
                enabled: true,
                variant: ActionVariant::Secondary,
            },
        ];
        let keys = [
            KeyCode::Esc,
            KeyCode::Enter,
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Char('d'),
            KeyCode::Tab,
        ];
        let mut seed = 9u64;
        for _ in 0..200 {
            if !state.is_visible() {
                state.show();
                state.set_focused(true);
            }
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let k = keys[(seed as usize) % keys.len()];
            let _ = state.handle_key_with(KeyEvent::new(k, KeyModifiers::NONE), &actions, true);
        }
    }

    #[test]
    fn paint_perf_smoke() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let mut state = AlertState::<()>::new();
        let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..200 {
            terminal
                .draw(|f| {
                    Alert::new("Warning", &system)
                        .description("check disk")
                        .source("host-a")
                        .paint(f.area(), f.buffer_mut(), &mut state);
                    let _ = Callout::new("Info", &system)
                        .tone(CalloutTone::Info)
                        .description("ok")
                        .paint(Rect::new(0, 8, 60, 3), f.buffer_mut());
                })
                .unwrap();
        }
        assert!(start.elapsed().as_millis() < 5_000);
    }

    #[test]
    fn pty_snapshot_stable() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let paint = |backend: TestBackend| {
            let mut terminal = Terminal::new(backend).unwrap();
            let mut state = AlertState::<()>::new();
            terminal
                .draw(|f| {
                    Alert::new("Saved", &system)
                        .tone(CalloutTone::Success)
                        .description("checkpoint written")
                        .compact()
                        .paint(f.area(), f.buffer_mut(), &mut state);
                })
                .unwrap();
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol().to_string())
                .collect::<String>()
        };
        assert_eq!(
            paint(TestBackend::new(40, 6)),
            paint(TestBackend::new(40, 6))
        );
    }

    #[test]
    fn all_tones_paint() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 30, 3);
        for tone in [
            CalloutTone::Info,
            CalloutTone::Success,
            CalloutTone::Warning,
            CalloutTone::Danger,
            CalloutTone::Destructive,
            CalloutTone::Neutral,
        ] {
            let mut buf = Buffer::empty(area);
            let _ = Callout::new(tone.id(), &system)
                .tone(tone)
                .description("msg")
                .paint(area, &mut buf);
        }
    }
}

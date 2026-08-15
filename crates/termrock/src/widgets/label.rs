// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Label and Description — consistent field caption primitives.
//!
//! **Mission.** Label primary control names; Description carries help, error,
//! warning, or meta copy. Both can associate with a target control id for
//! semantic scene / help tooling (form accessibility model adapted to TUI).
//!
//! **Contraction.** Descriptions drop before primary labels when width is
//! tight ([`DROP_DESCRIPTION_WIDTH`]). Compact layout prefers a single row.
//!
//! References: Radix/shadcn Label, accessible form labeling, terminal settings.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState};
use crate::style::{DesignSystem, GlyphSet, Role};
use crate::widgets::text::{Text, TextSpan};

/// Width below which descriptions are omitted (labels may remain).
pub const DROP_DESCRIPTION_WIDTH: u16 = 28;
/// Width below which required/optional marks may be elided in compact mode.
pub const DROP_MARK_WIDTH: u16 = 14;

/// How the label relates to its control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CaptionLayout {
    /// Label above control / description below label (form default).
    #[default]
    Stacked,
    /// Label on one row (control sits beside — host places control).
    Inline,
    /// Minimal single-line; drop description first, then marks.
    Compact,
}

impl CaptionLayout {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Stacked => "stacked",
            Self::Inline => "inline",
            Self::Compact => "compact",
        }
    }
}

/// Field requirement mark on the label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum LabelMark {
    /// No mark.
    #[default]
    None,
    /// Required (`*` non-color cue).
    Required,
    /// Optional (dim `(opt)` when width allows).
    Optional,
}

impl LabelMark {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Required => "required",
            Self::Optional => "optional",
        }
    }
}

/// Visual / semantic tone of the label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum LabelTone {
    /// Normal field label.
    #[default]
    Default,
    /// Control disabled.
    Disabled,
    /// Validation failed (label emphasizes error association).
    Invalid,
    /// Soft warning.
    Warning,
    /// Control owns focus (bold).
    Focused,
}

impl LabelTone {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Disabled => "disabled",
            Self::Invalid => "invalid",
            Self::Warning => "warning",
            Self::Focused => "focused",
        }
    }
}

/// Description semantic kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DescriptionKind {
    /// Help / hint under the field.
    #[default]
    Help,
    /// Validation error.
    Error,
    /// Warning (not yet invalid).
    Warning,
    /// Secondary metadata.
    Meta,
}

impl DescriptionKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Help => "help",
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Meta => "meta",
        }
    }

    fn role(self) -> Role {
        match self {
            Self::Help | Self::Meta => Role::TextMuted,
            Self::Error => Role::Danger,
            Self::Warning => Role::Warning,
        }
    }
}

/// Geometry for a lone label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LabelParts {
    /// Outer allocation used.
    pub root: Rect,
    /// Painted label band.
    pub label: Rect,
}

/// Geometry for a description line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DescriptionParts {
    /// Outer allocation used.
    pub root: Rect,
    /// Painted description band (zero height when contracted away).
    pub description: Rect,
    /// True when omitted due to narrow width or empty text.
    pub contracted: bool,
}

/// Geometry for label + optional description caption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CaptionParts {
    /// Outer allocation.
    pub root: Rect,
    /// Label band.
    pub label: Rect,
    /// Description band (may be empty).
    pub description: Rect,
    /// Description was dropped for width.
    pub description_contracted: bool,
}

/// Primary field / control label.
#[derive(Debug, Clone)]
pub struct Label<'a, Id = ()> {
    text: &'a str,
    /// Target control id (semantic association).
    for_id: Option<Id>,
    mark: LabelMark,
    tone: LabelTone,
    layout: CaptionLayout,
    system: &'a DesignSystem,
}

impl<'a, Id> Label<'a, Id> {
    /// Label text for a field or control.
    #[must_use]
    pub const fn new(text: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            text,
            for_id: None,
            mark: LabelMark::None,
            tone: LabelTone::Default,
            layout: CaptionLayout::Stacked,
            system,
        }
    }

    /// Associate with a control id (form / settings target).
    #[must_use]
    pub fn for_id(mut self, id: Id) -> Self {
        self.for_id = Some(id);
        self
    }

    /// Optional target borrow.
    #[must_use]
    pub const fn target(&self) -> Option<&Id> {
        self.for_id.as_ref()
    }

    /// Requirement mark.
    #[must_use]
    pub const fn mark(mut self, mark: LabelMark) -> Self {
        self.mark = mark;
        self
    }

    /// Required field.
    #[must_use]
    pub const fn required(mut self) -> Self {
        self.mark = LabelMark::Required;
        self
    }

    /// Optional field mark.
    #[must_use]
    pub const fn optional(mut self) -> Self {
        self.mark = LabelMark::Optional;
        self
    }

    /// Tone.
    #[must_use]
    pub const fn tone(mut self, tone: LabelTone) -> Self {
        self.tone = tone;
        self
    }

    /// Disabled tone.
    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.tone = LabelTone::Disabled;
        self
    }

    /// Invalid tone.
    #[must_use]
    pub const fn invalid(mut self) -> Self {
        self.tone = LabelTone::Invalid;
        self
    }

    /// Warning tone.
    #[must_use]
    pub const fn warning(mut self) -> Self {
        self.tone = LabelTone::Warning;
        self
    }

    /// Focused tone.
    #[must_use]
    pub const fn focused(mut self) -> Self {
        self.tone = LabelTone::Focused;
        self
    }

    /// Layout recipe.
    #[must_use]
    pub const fn layout_mode(mut self, layout: CaptionLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Compact recipe.
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.layout = CaptionLayout::Compact;
        self
    }

    /// Inline recipe.
    #[must_use]
    pub const fn inline(mut self) -> Self {
        self.layout = CaptionLayout::Inline;
        self
    }

    /// Raw label text (without marks).
    #[must_use]
    pub const fn text(&self) -> &'a str {
        self.text
    }

    /// Tone.
    #[must_use]
    pub const fn tone_of(&self) -> LabelTone {
        self.tone
    }

    /// Mark.
    #[must_use]
    pub const fn mark_of(&self) -> LabelMark {
        self.mark
    }

    /// Layout mode.
    #[must_use]
    pub const fn layout_of(&self) -> CaptionLayout {
        self.layout
    }

    /// Whether marks should paint for this width / layout.
    #[must_use]
    pub fn show_mark(&self, width: u16) -> bool {
        if matches!(self.mark, LabelMark::None) {
            return false;
        }
        if matches!(self.layout, CaptionLayout::Compact) && width < DROP_MARK_WIDTH {
            return false;
        }
        true
    }

    /// Decorated display string (marks + disabled glyph).
    #[must_use]
    pub fn decorated(&self, width: u16) -> String {
        let mut out = self.text.to_string();
        if self.show_mark(width) {
            match self.mark {
                LabelMark::None => {}
                LabelMark::Required => out.push_str(" *"),
                LabelMark::Optional => {
                    if width >= 22 {
                        out.push_str(" (opt)");
                    }
                }
            }
        }
        if matches!(self.tone, LabelTone::Disabled) {
            let mark = self.system.glyphs.disabled_mark();
            out.push(' ');
            out.push_str(mark);
        }
        out
    }

    /// Plain text for copy / help (includes mark glyphs when present).
    #[must_use]
    pub fn plain(&self) -> String {
        self.decorated(80)
    }

    /// Help line for semantic scene / Studio (without target id).
    #[must_use]
    pub fn semantic_description(&self) -> String {
        let mut parts = Vec::new();
        match self.mark {
            LabelMark::Required => parts.push("required"),
            LabelMark::Optional => parts.push("optional"),
            LabelMark::None => {}
        }
        match self.tone {
            LabelTone::Default => {}
            LabelTone::Disabled => parts.push("disabled"),
            LabelTone::Invalid => parts.push("invalid"),
            LabelTone::Warning => parts.push("warning"),
            LabelTone::Focused => parts.push("focused"),
        }
        if parts.is_empty() {
            "field label".into()
        } else {
            parts.join(", ")
        }
    }

    fn resolve_role(&self) -> Role {
        match self.tone {
            LabelTone::Default | LabelTone::Focused => Role::Text,
            LabelTone::Disabled => Role::TextDisabled,
            LabelTone::Invalid => Role::Danger,
            LabelTone::Warning => Role::Warning,
        }
    }

    /// Layout (single row).
    #[must_use]
    pub fn layout(&self, area: Rect) -> LabelParts {
        if area.is_empty() {
            return LabelParts {
                root: area,
                label: area,
            };
        }
        LabelParts {
            root: area,
            label: Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1u16.min(area.height),
            },
        }
    }

    /// Paint label into `area`.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) -> LabelParts {
        let parts = self.layout(area);
        if parts.label.is_empty() {
            return parts;
        }
        let decorated = self.decorated(parts.label.width);
        let mut span = TextSpan::new(decorated).role(self.resolve_role());
        if matches!(self.tone, LabelTone::Focused) {
            span = span.strong();
        }
        if matches!(self.tone, LabelTone::Disabled) {
            span = span.dim();
        }
        let _ = Text::spans([span], self.system)
            .truncate()
            .paint(parts.label, buffer);
        parts
    }

    /// Register as semantic content associated with `for_id` when present.
    pub fn register_semantic<Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        let parts = self.layout(area);
        if parts.label.is_empty() {
            return;
        }
        let desc = match &self.for_id {
            Some(target) => format!("{}; for={target}", self.semantic_description()),
            None => self.semantic_description(),
        };
        let _ = scene.register(
            SemanticNode::content(id, parts.label)
                .role(SemanticRole::Content)
                .label(self.text)
                .description(desc)
                .focusable(false)
                .state(SemanticState {
                    invalid: matches!(self.tone, LabelTone::Invalid),
                    ..Default::default()
                }),
        );
    }
}

impl<Id> Widget for &Label<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer);
    }
}

impl<Id> Widget for Label<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

/// Supporting description under a label or control.
#[derive(Debug, Clone)]
pub struct Description<'a, Id = ()> {
    text: &'a str,
    for_id: Option<Id>,
    kind: DescriptionKind,
    system: &'a DesignSystem,
}

impl<'a, Id> Description<'a, Id> {
    /// Help-style description.
    #[must_use]
    pub const fn new(text: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            text,
            for_id: None,
            kind: DescriptionKind::Help,
            system,
        }
    }

    /// Error description.
    #[must_use]
    pub const fn error(text: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            text,
            for_id: None,
            kind: DescriptionKind::Error,
            system,
        }
    }

    /// Warning description.
    #[must_use]
    pub const fn warning(text: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            text,
            for_id: None,
            kind: DescriptionKind::Warning,
            system,
        }
    }

    /// Meta description.
    #[must_use]
    pub const fn meta(text: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            text,
            for_id: None,
            kind: DescriptionKind::Meta,
            system,
        }
    }

    /// Associate with control id.
    #[must_use]
    pub fn for_id(mut self, id: Id) -> Self {
        self.for_id = Some(id);
        self
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, kind: DescriptionKind) -> Self {
        self.kind = kind;
        self
    }

    /// Kind of this description.
    #[must_use]
    pub const fn kind_of(&self) -> DescriptionKind {
        self.kind
    }

    /// Target id.
    #[must_use]
    pub const fn target(&self) -> Option<&Id> {
        self.for_id.as_ref()
    }

    /// Body text.
    #[must_use]
    pub const fn text(&self) -> &'a str {
        self.text
    }

    /// Whether this description should paint at `width`.
    #[must_use]
    pub fn visible_at(&self, width: u16) -> bool {
        !self.text.is_empty() && width >= DROP_DESCRIPTION_WIDTH
    }

    /// Plain text.
    #[must_use]
    pub fn plain(&self) -> &str {
        self.text
    }

    /// Semantic help string (kind only; target appended in `register_semantic`).
    #[must_use]
    pub fn semantic_description(&self) -> String {
        self.kind.id().to_string()
    }

    /// Layout; contracts to zero height when too narrow.
    #[must_use]
    pub fn layout(&self, area: Rect) -> DescriptionParts {
        if area.is_empty() || !self.visible_at(area.width) {
            return DescriptionParts {
                root: area,
                description: Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: 0,
                },
                contracted: !self.text.is_empty(),
            };
        }
        DescriptionParts {
            root: area,
            description: Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1u16.min(area.height),
            },
            contracted: false,
        }
    }

    /// Paint description (no-op when contracted).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) -> DescriptionParts {
        let parts = self.layout(area);
        if parts.description.is_empty() {
            return parts;
        }
        let _ = Text::new(self.text, self.system)
            .role(self.kind.role())
            .truncate()
            .paint(parts.description, buffer);
        parts
    }

    /// Register semantic content node.
    pub fn register_semantic<Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        let parts = self.layout(area);
        if parts.description.is_empty() {
            return;
        }
        let desc = match &self.for_id {
            Some(target) => format!("{} for {target}", self.semantic_description()),
            None => self.semantic_description(),
        };
        let _ = scene.register(
            SemanticNode::content(id, parts.description)
                .role(SemanticRole::Content)
                .label(self.text)
                .description(desc)
                .focusable(false)
                .state(SemanticState {
                    invalid: matches!(self.kind, DescriptionKind::Error),
                    ..Default::default()
                }),
        );
    }
}

impl<Id> Widget for &Description<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer);
    }
}

impl<Id> Widget for Description<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

/// Label + optional description caption (stacked / compact / inline recipes).
#[derive(Debug, Clone)]
pub struct FieldCaption<'a, Id = ()> {
    label: Label<'a, Id>,
    description: Option<Description<'a, Id>>,
    layout: CaptionLayout,
    system: &'a DesignSystem,
}

impl<'a, Id: Clone> FieldCaption<'a, Id> {
    /// Caption from label text (no description yet).
    #[must_use]
    pub fn new(label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            label: Label::new(label, system),
            description: None,
            layout: CaptionLayout::Stacked,
            system,
        }
    }

    /// From existing label.
    #[must_use]
    pub fn from_label(label: Label<'a, Id>, system: &'a DesignSystem) -> Self {
        let layout = label.layout_of();
        Self {
            label,
            description: None,
            layout,
            system,
        }
    }

    /// Associate both with control id.
    #[must_use]
    pub fn for_id(mut self, id: Id) -> Self {
        self.label = self.label.for_id(id.clone());
        if let Some(d) = self.description.take() {
            self.description = Some(d.for_id(id));
        }
        self
    }

    /// Attach description.
    #[must_use]
    pub fn description(mut self, description: Description<'a, Id>) -> Self {
        let d = if let Some(id) = self.label.target().cloned() {
            description.for_id(id)
        } else {
            description
        };
        self.description = Some(d);
        self
    }

    /// Help text convenience.
    #[must_use]
    pub fn help(self, text: &'a str) -> Self {
        let system = self.system;
        let d = Description::new(text, system);
        self.description(d)
    }

    /// Error text convenience.
    #[must_use]
    pub fn error(mut self, text: &'a str) -> Self {
        let system = self.system;
        self.label = self.label.invalid();
        let d = Description::error(text, system);
        self.description(d)
    }

    /// Required mark.
    #[must_use]
    pub fn required(mut self) -> Self {
        self.label = self.label.required();
        self
    }

    /// Optional mark.
    #[must_use]
    pub fn optional(mut self) -> Self {
        self.label = self.label.optional();
        self
    }

    /// Disabled.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.label = self.label.disabled();
        self
    }

    /// Focused label.
    #[must_use]
    pub fn focused(mut self) -> Self {
        self.label = self.label.focused();
        self
    }

    /// Warning label tone.
    #[must_use]
    pub fn warning(mut self) -> Self {
        self.label = self.label.warning();
        self
    }

    /// Layout recipe.
    #[must_use]
    pub fn layout_mode(mut self, layout: CaptionLayout) -> Self {
        self.layout = layout;
        self.label = self.label.layout_mode(layout);
        self
    }

    /// Compact.
    #[must_use]
    pub fn compact(self) -> Self {
        self.layout_mode(CaptionLayout::Compact)
    }

    /// Inline.
    #[must_use]
    pub fn inline(self) -> Self {
        self.layout_mode(CaptionLayout::Inline)
    }

    /// Stacked.
    #[must_use]
    pub fn stacked(self) -> Self {
        self.layout_mode(CaptionLayout::Stacked)
    }

    /// Label borrow.
    #[must_use]
    pub const fn label(&self) -> &Label<'a, Id> {
        &self.label
    }

    /// Description borrow.
    #[must_use]
    pub const fn description_of(&self) -> Option<&Description<'a, Id>> {
        self.description.as_ref()
    }

    /// Natural height for width (1 or 2; description may contract).
    #[must_use]
    pub fn measure_height(&self, width: u16) -> u16 {
        let mut h = 1u16;
        if let Some(d) = &self.description
            && d.visible_at(width)
            && !matches!(self.layout, CaptionLayout::Inline | CaptionLayout::Compact)
        {
            h = 2;
        }
        // Compact/inline: description only if width allows and height budget separate —
        // default single row for compact/inline.
        if matches!(self.layout, CaptionLayout::Compact | CaptionLayout::Inline) {
            return 1;
        }
        h
    }

    /// Layout caption into `area`. Description contracts before the label.
    #[must_use]
    pub fn layout(&self, area: Rect) -> CaptionParts {
        if area.is_empty() {
            return CaptionParts {
                root: area,
                label: area,
                description: Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: 0,
                },
                description_contracted: false,
            };
        }
        let show_desc = self
            .description
            .as_ref()
            .is_some_and(|d| d.visible_at(area.width))
            && area.height >= 2
            && !matches!(self.layout, CaptionLayout::Compact | CaptionLayout::Inline);

        let label = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1u16.min(area.height),
        };
        let (description, contracted) = if show_desc {
            (
                Rect {
                    x: area.x,
                    y: area.y.saturating_add(1),
                    width: area.width,
                    height: 1,
                },
                false,
            )
        } else {
            (
                Rect {
                    x: area.x,
                    y: area.y.saturating_add(1),
                    width: area.width,
                    height: 0,
                },
                self.description
                    .as_ref()
                    .is_some_and(|d| !d.text.is_empty()),
            )
        };
        CaptionParts {
            root: area,
            label,
            description,
            description_contracted: contracted,
        }
    }

    /// Paint label (+ description when not contracted).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) -> CaptionParts {
        let parts = self.layout(area);
        let _ = self.label.paint(parts.label, buffer);
        if let Some(d) = &self.description
            && parts.description.height > 0
        {
            let _ = d.paint(parts.description, buffer);
        }
        parts
    }

    /// Register label and description semantic nodes.
    pub fn register_semantic<Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        label_id: Id,
        description_id: Option<Id>,
        area: Rect,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        let parts = self.layout(area);
        self.label.register_semantic(scene, label_id, parts.label);
        if let (Some(d), Some(did)) = (&self.description, description_id)
            && parts.description.height > 0
        {
            d.register_semantic(scene, did, parts.description);
        }
    }
}

impl<Id: Clone> Widget for &FieldCaption<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer);
    }
}

impl<Id: Clone> Widget for FieldCaption<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

/// Extract plain text from a ratatui [`ratatui_core::text::Line`] for Label paint.
#[must_use]
pub fn line_plain(line: &ratatui_core::text::Line<'_>) -> String {
    line.spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<Vec<_>>()
        .join("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::GlyphSet;

    #[test]
    fn required_mark_and_disabled_glyph() {
        let system = DesignSystem::default();
        let l = Label::<()>::new("Name", &system).required().disabled();
        let d = l.decorated(40);
        assert!(d.contains('*'));
        assert!(d.contains('⊘') || d.contains('x'));
    }

    #[test]
    fn compact_drops_mark_when_tiny() {
        let system = DesignSystem::default();
        let l = Label::<()>::new("N", &system).required().compact();
        assert!(!l.show_mark(10));
        assert!(l.show_mark(20));
    }

    #[test]
    fn description_contracts_before_label() {
        let system = DesignSystem::default();
        let cap = FieldCaption::<&str>::new("Endpoint", &system)
            .for_id("ep")
            .help("https://…")
            .required();
        let wide = cap.layout(Rect::new(0, 0, 40, 2));
        assert_eq!(wide.description.height, 1);
        assert!(!wide.description_contracted);
        let narrow = cap.layout(Rect::new(0, 0, 20, 2));
        assert_eq!(narrow.description.height, 0);
        assert!(narrow.description_contracted);
        // label still present
        assert_eq!(narrow.label.height, 1);
    }

    #[test]
    fn error_sets_invalid_tone() {
        let system = DesignSystem::default();
        let cap = FieldCaption::<()>::new("Port", &system).error("must be a number");
        assert!(matches!(cap.label().tone_of(), LabelTone::Invalid));
        assert!(matches!(
            cap.description_of().unwrap().kind_of(),
            DescriptionKind::Error
        ));
    }

    #[test]
    fn tones_and_kinds_ids() {
        assert_eq!(LabelTone::Warning.id(), "warning");
        assert_eq!(DescriptionKind::Help.id(), "help");
        assert_eq!(CaptionLayout::Stacked.id(), "stacked");
    }

    #[test]
    fn paint_label_and_description() {
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 32, 2));
        let cap = FieldCaption::<()>::new("Name", &system)
            .required()
            .help("display name");
        let parts = cap.paint(Rect::new(0, 0, 32, 2), &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "N");
        assert!(parts.description.height > 0);
        assert_eq!(buf[(0, 1)].symbol(), "d");
    }

    #[test]
    fn ascii_disabled_mark() {
        let system = DesignSystem::default().glyphs(GlyphSet::Ascii);
        let l = Label::<()>::new("X", &system).disabled();
        assert!(l.decorated(40).contains('~'));
    }

    #[test]
    fn semantic_registers_for_id() {
        let system = DesignSystem::default();
        let l = Label::new("Host", &system).for_id("host").required();
        let mut scene = SemanticScene::<&str, ()>::new();
        scene.begin_frame();
        l.register_semantic(&mut scene, "lbl-host", Rect::new(0, 0, 20, 1));
        assert_eq!(scene.len(), 1);
        let n = &scene.nodes()[0];
        assert!(n.description.as_ref().unwrap().contains("for=host"));
        assert!(n.description.as_ref().unwrap().contains("required"));
    }

    #[test]
    fn measure_height_stacked_vs_compact() {
        let system = DesignSystem::default();
        let stacked = FieldCaption::<()>::new("A", &system).help("b");
        assert_eq!(stacked.measure_height(40), 2);
        assert_eq!(stacked.measure_height(10), 1);
        let compact = FieldCaption::<()>::new("A", &system).help("b").compact();
        assert_eq!(compact.measure_height(40), 1);
    }

    #[test]
    fn line_plain_joins_spans() {
        use ratatui_core::text::{Line, Span};
        let line = Line::from(vec![Span::raw("Hello"), Span::raw(" "), Span::raw("世界")]);
        assert_eq!(line_plain(&line), "Hello 世界");
    }

    #[test]
    fn layout_is_cheap() {
        let system = DesignSystem::default();
        let cap = FieldCaption::<&str>::new("Endpoint", &system)
            .for_id("ep")
            .required()
            .help("base URL for the agent");
        let area = Rect::new(0, 0, 36, 2);
        for _ in 0..20_000 {
            let _ = cap.layout(area);
        }
    }

    #[test]
    fn empty_area_safe() {
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        let parts = Label::<()>::new("X", &system).paint(Rect::new(0, 0, 0, 0), &mut buf);
        assert!(parts.label.is_empty() || parts.root.is_empty());
    }
}

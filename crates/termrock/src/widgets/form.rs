// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Field, Fieldset, and Form — compositional form architecture.
//!
//! **Mission.** Shared chrome for every input control: labels, descriptions,
//! required/optional, validation, warnings, async pending, help, dirty/touched
//! projection, and form-level submit / error summary / first-invalid focus.
//!
//! **Ownership.** Domain values live in the host. Fields **project** display
//! strings and status; Form never stores application state. Focus is host /
//! [`InteractionScene`] owned — pass [`Form::focused_field`] for paint.
//!
//! **vs FieldCaption.** FieldCaption is label+description chrome only. Field
//! adds value row, status, dirty/touched, and form integration.
//!
//! Research: shadcn form composition, React Hook Form concepts, Huh, Textual,
//! desktop settings panels.

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Modifier, Style},
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    interaction::{HitRegion, PageMove, UiIntent},
    layout::{ResponsiveSurface, form_grid_template},
    scroll::max_offset,
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
    widgets::{
        field_row::{FieldRow, FieldRowValue},
        label::Description,
    },
};

// ── Field ───────────────────────────────────────────────────────────────────

/// Supporting status under a field value (host-projected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FieldStatus<'a> {
    /// No supporting line.
    #[default]
    None,
    /// Neutral help / description.
    Help(&'a str),
    /// Non-blocking warning.
    Warning(&'a str),
    /// Validation error.
    Error(&'a str),
    /// Async validation in flight.
    Pending(&'a str),
}

impl<'a> FieldStatus<'a> {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Help(_) => "help",
            Self::Warning(_) => "warning",
            Self::Error(_) => "error",
            Self::Pending(_) => "pending",
        }
    }

    /// Message text if any.
    #[must_use]
    pub const fn message(self) -> Option<&'a str> {
        match self {
            Self::None => None,
            Self::Help(s) | Self::Warning(s) | Self::Error(s) | Self::Pending(s) => Some(s),
        }
    }

    /// Whether this is a blocking validation error.
    #[must_use]
    pub const fn is_error(self) -> bool {
        matches!(self, Self::Error(_))
    }

    /// Whether async work is pending.
    #[must_use]
    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Pending(_))
    }
}

/// One form field projection (controlled; host owns domain values).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field<'a, Id> {
    /// Stable identity (focus, hits, outcomes).
    pub id: Id,
    /// Label text.
    pub label: &'a str,
    /// Displayed value (host-formatted).
    pub value: FieldRowValue<'a>,
    /// Optional long description (contracts before label on narrow).
    pub description: Option<&'a str>,
    /// Supporting status line.
    pub status: FieldStatus<'a>,
    /// Required mark.
    pub required: bool,
    /// Optional mark (when not required).
    pub show_optional: bool,
    /// Enabled for interaction.
    pub enabled: bool,
    /// Read-only presentation.
    pub read_only: bool,
    /// Host: value differs from baseline.
    pub dirty: bool,
    /// Host: field has been visited/blurred.
    pub touched: bool,
}

impl<'a, Id> Field<'a, Id> {
    /// Enabled field with label and value.
    #[must_use]
    pub const fn new(id: Id, label: &'a str, value: &'a str) -> Self {
        Self {
            id,
            label,
            value: FieldRowValue::Plain(value),
            description: None,
            status: FieldStatus::None,
            required: false,
            show_optional: false,
            enabled: true,
            read_only: false,
            dirty: false,
            touched: false,
        }
    }

    /// Masks the projected value using the canonical FieldRow recipe.
    #[must_use]
    pub fn masked(mut self, len: usize) -> Self {
        self.value = FieldRowValue::Masked { len };
        self
    }

    /// Projects a missing value using the canonical FieldRow recipe.
    #[must_use]
    pub fn unset(mut self, hint: &'a str) -> Self {
        self.value = FieldRowValue::Unset { hint };
        self
    }

    /// Help status (and optional description keep separate).
    #[must_use]
    pub const fn help(mut self, help: &'a str) -> Self {
        self.status = FieldStatus::Help(help);
        self
    }

    /// Error status.
    #[must_use]
    pub const fn error(mut self, error: &'a str) -> Self {
        self.status = FieldStatus::Error(error);
        self
    }

    /// Warning status.
    #[must_use]
    pub const fn warning(mut self, warning: &'a str) -> Self {
        self.status = FieldStatus::Warning(warning);
        self
    }

    /// Async validation pending.
    #[must_use]
    pub const fn pending(mut self, message: &'a str) -> Self {
        self.status = FieldStatus::Pending(message);
        self
    }

    /// Explicit status.
    #[must_use]
    pub const fn status(mut self, status: FieldStatus<'a>) -> Self {
        self.status = status;
        self
    }

    /// Description under label (independent of status).
    #[must_use]
    pub const fn description(mut self, description: &'a str) -> Self {
        self.description = Some(description);
        self
    }

    /// Required.
    #[must_use]
    pub const fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Show optional mark.
    #[must_use]
    pub const fn optional(mut self, on: bool) -> Self {
        self.show_optional = on;
        self
    }

    /// Enabled.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Read-only.
    #[must_use]
    pub const fn read_only(mut self, on: bool) -> Self {
        self.read_only = on;
        self
    }

    /// Dirty projection.
    #[must_use]
    pub const fn dirty(mut self, on: bool) -> Self {
        self.dirty = on;
        self
    }

    /// Touched projection.
    #[must_use]
    pub const fn touched(mut self, on: bool) -> Self {
        self.touched = on;
        self
    }

    /// Whether invalid (error status).
    #[must_use]
    pub const fn is_invalid(&self) -> bool {
        self.status.is_error()
    }

    /// Activatable.
    #[must_use]
    pub const fn can_activate(&self) -> bool {
        self.enabled && !self.read_only
    }
}

/// Historical name for [`Field`].
pub type FormField<'a, Id> = Field<'a, Id>;

// ── Fieldset ────────────────────────────────────────────────────────────────

/// Labeled group of fields (fieldset / section).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fieldset<'a, Id> {
    /// Legend / section title.
    pub legend: &'a str,
    /// Optional group description.
    pub description: Option<&'a str>,
    /// Fields in order.
    pub fields: &'a [Field<'a, Id>],
}

impl<'a, Id> Fieldset<'a, Id> {
    /// Legend + fields.
    #[must_use]
    pub const fn new(legend: &'a str, fields: &'a [Field<'a, Id>]) -> Self {
        Self {
            legend,
            description: None,
            fields,
        }
    }

    /// Group description.
    #[must_use]
    pub const fn description(mut self, description: &'a str) -> Self {
        self.description = Some(description);
        self
    }
}

/// Historical name for [`Fieldset`].
pub type FormSection<'a, Id> = Fieldset<'a, Id>;

// ── Layout ──────────────────────────────────────────────────────────────────

/// Form layout recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FormLayout {
    /// 1–2 columns from width (default).
    #[default]
    Responsive,
    /// Force single column stacked.
    Stacked,
    /// Denser rows (less vertical padding).
    Compact,
    /// Prefer label and value on one row when wide.
    Inline,
}

impl FormLayout {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Responsive => "responsive",
            Self::Stacked => "stacked",
            Self::Compact => "compact",
            Self::Inline => "inline",
        }
    }

    fn field_row_height(self) -> usize {
        match self {
            Self::Compact | Self::Inline => 3,
            Self::Responsive | Self::Stacked => 4,
        }
    }

    fn section_header_height(self) -> usize {
        match self {
            Self::Compact => 1,
            _ => 2,
        }
    }
}

// ── Outcomes / regions / state ──────────────────────────────────────────────

/// Form interaction outcomes (no hidden app state).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormOutcome<Id> {
    /// No change.
    Ignored,
    /// Focused field requested activation (Enter / re-click).
    Activated(Id),
    /// Viewport scrolled.
    Scrolled,
    /// Host should submit (values already on host model).
    SubmitRequested,
    /// Host should reset projections to baseline.
    ResetRequested,
}

/// Painted hit geometry for one field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormFieldRegion<Id> {
    /// Field id.
    pub id: Id,
    /// Full field hit rect.
    pub area: Rect,
    /// Label row.
    pub label: Option<Rect>,
    /// Value row.
    pub value: Option<Rect>,
    /// Supporting status/help row.
    pub supporting: Option<Rect>,
}

/// Runtime state: scroll, hover, hits — **not** domain values or field focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormState<Id> {
    hovered: Option<Id>,
    accepts_input: bool,
    offset: usize,
    viewport_height: usize,
    content_height: usize,
    column_count: u8,
    ensure_visible: Option<Id>,
    regions: Vec<HitRegion<Id>>,
    field_regions: Vec<FormFieldRegion<Id>>,
    scrollbar_region: Option<Rect>,
    /// Last error summary lines painted (host may also read via helpers).
    error_summary_count: usize,
}

impl<Id> Default for FormState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> FormState<Id> {
    /// Fresh scroll state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            hovered: None,
            accepts_input: true,
            offset: 0,
            viewport_height: 0,
            content_height: 0,
            column_count: 1,
            ensure_visible: None,
            regions: Vec::new(),
            field_regions: Vec::new(),
            scrollbar_region: None,
            error_summary_count: 0,
        }
    }

    /// Hovered field.
    #[must_use]
    pub const fn hovered(&self) -> Option<&Id> {
        self.hovered.as_ref()
    }

    /// Accepts interaction.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Enable/disable surface.
    pub const fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Scroll target after host focus change.
    pub fn ensure_visible(&mut self, id: Option<Id>) {
        self.ensure_visible = id;
    }

    /// Scroll offset.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Columns used.
    #[must_use]
    pub const fn column_count(&self) -> u8 {
        self.column_count
    }

    /// Content height.
    #[must_use]
    pub const fn content_height(&self) -> usize {
        self.content_height
    }

    /// Hit regions.
    #[must_use]
    pub fn regions(&self) -> &[HitRegion<Id>] {
        &self.regions
    }

    /// Field regions.
    #[must_use]
    pub fn field_regions(&self) -> &[FormFieldRegion<Id>] {
        &self.field_regions
    }

    /// Error summary row count from last paint.
    #[must_use]
    pub const fn error_summary_count(&self) -> usize {
        self.error_summary_count
    }

    /// Scroll by delta.
    pub fn scroll_by(&mut self, delta: isize, content_len: usize) -> bool {
        let before = self.offset;
        let maximum = max_offset(content_len, self.viewport_height);
        self.offset = if delta.is_negative() {
            self.offset.saturating_sub(delta.unsigned_abs())
        } else {
            self.offset
                .saturating_add(delta.unsigned_abs())
                .min(maximum)
        };
        before != self.offset
    }

    /// Scrollbar track click.
    pub fn scroll_to_position(&mut self, position: Position) -> bool {
        let Some(area) = self.scrollbar_region else {
            return false;
        };
        if !area.contains(position) {
            return false;
        }
        self.offset = crate::scroll::offset_for_track_position(
            self.content_height,
            self.viewport_height,
            area.height,
            usize::from(position.y.saturating_sub(area.y)),
        );
        true
    }
}

// ── Pure form helpers (no hidden state) ─────────────────────────────────────

/// First invalid enabled field id (source order).
#[must_use]
pub fn first_invalid_id<'a, Id: Clone>(fieldsets: &'a [Fieldset<'_, Id>]) -> Option<&'a Id> {
    fieldsets
        .iter()
        .flat_map(|fs| fs.fields.iter())
        .find(|f| f.enabled && f.is_invalid())
        .map(|f| &f.id)
}

/// Collect error messages with field labels (for summary banners).
#[must_use]
pub fn collect_errors<'a, Id>(fieldsets: &'a [Fieldset<'_, Id>]) -> Vec<(&'a str, &'a str)> {
    fieldsets
        .iter()
        .flat_map(|fs| fs.fields.iter())
        .filter_map(|f| match f.status {
            FieldStatus::Error(msg) => Some((f.label, msg)),
            _ => None,
        })
        .collect()
}

/// Any projected dirty field.
#[must_use]
pub fn any_dirty<Id>(fieldsets: &[Fieldset<'_, Id>]) -> bool {
    fieldsets
        .iter()
        .flat_map(|fs| fs.fields.iter())
        .any(|f| f.dirty)
}

/// Any projected touched field.
#[must_use]
pub fn any_touched<Id>(fieldsets: &[Fieldset<'_, Id>]) -> bool {
    fieldsets
        .iter()
        .flat_map(|fs| fs.fields.iter())
        .any(|f| f.touched)
}

/// All required fields have non-empty trimmed values (host still owns real validation).
#[must_use]
pub fn required_filled<Id>(fieldsets: &[Fieldset<'_, Id>]) -> bool {
    fieldsets
        .iter()
        .flat_map(|fs| fs.fields.iter())
        .filter(|f| f.required && f.enabled)
        .all(|f| f.value.is_set())
}

impl<Id: Clone + PartialEq> FormState<Id> {
    /// Scroll first invalid field into view; returns its id for host scene.focus.
    pub fn focus_first_invalid(&mut self, fieldsets: &[Fieldset<'_, Id>]) -> Option<Id> {
        let id = first_invalid_id(fieldsets)?.clone();
        self.ensure_visible(Some(id.clone()));
        Some(id)
    }

    /// Keys: activate / page / submit / reset chords.
    pub fn handle_key(
        &mut self,
        fieldsets: &[Fieldset<'_, Id>],
        key: KeyEvent,
        focused_field: Option<&Id>,
    ) -> FormOutcome<Id> {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return FormOutcome::Ignored;
        }
        // Form-level chords before default form intent
        if key.kind == KeyEventKind::Press {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(
                    key.code,
                    KeyCode::Enter | KeyCode::Char('s') | KeyCode::Char('S')
                )
            {
                return FormOutcome::SubmitRequested;
            }
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R'))
                && !key.modifiers.contains(KeyModifiers::SHIFT)
            {
                return FormOutcome::ResetRequested;
            }
        }
        if let Some(intent) = crate::interaction::default_form_intent(key) {
            return self.handle_intent(fieldsets, intent, focused_field);
        }
        FormOutcome::Ignored
    }

    /// Intent routing.
    pub fn handle_intent(
        &mut self,
        fieldsets: &[Fieldset<'_, Id>],
        intent: UiIntent,
        focused_field: Option<&Id>,
    ) -> FormOutcome<Id> {
        if !self.accepts_input {
            return FormOutcome::Ignored;
        }
        match intent {
            UiIntent::Submit => FormOutcome::SubmitRequested,
            UiIntent::Activate => {
                let Some(id) = focused_field else {
                    // No field focus → treat as submit request
                    return FormOutcome::SubmitRequested;
                };
                fieldsets
                    .iter()
                    .flat_map(|fs| fs.fields.iter())
                    .find(|f| f.can_activate() && &f.id == id)
                    .map_or(FormOutcome::Ignored, |f| {
                        FormOutcome::Activated(f.id.clone())
                    })
            }
            UiIntent::Page(PageMove::Forward) => {
                if self.scroll_by(self.viewport_height.max(1) as isize, self.content_height) {
                    FormOutcome::Scrolled
                } else {
                    FormOutcome::Ignored
                }
            }
            UiIntent::Page(PageMove::Backward) => {
                if self.scroll_by(-(self.viewport_height.max(1) as isize), self.content_height) {
                    FormOutcome::Scrolled
                } else {
                    FormOutcome::Ignored
                }
            }
            UiIntent::Cancel => FormOutcome::Ignored,
            _ => FormOutcome::Ignored,
        }
    }

    /// Hover update.
    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        self.hovered = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    /// Activate if already host-focused.
    pub fn click(&mut self, position: Position, focused_field: Option<&Id>) -> FormOutcome<Id> {
        if !self.accepts_input {
            return FormOutcome::Ignored;
        }
        let Some(id) = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone())
        else {
            return FormOutcome::Ignored;
        };
        if focused_field == Some(&id) {
            FormOutcome::Activated(id)
        } else {
            FormOutcome::Ignored
        }
    }

    /// Hit id for host scene.focus.
    #[must_use]
    pub fn hit_id(&self, position: Position) -> Option<&Id> {
        self.regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| &region.id)
    }
}

// ── Form widget ─────────────────────────────────────────────────────────────

/// Responsive form over borrowed fieldsets.
#[derive(Debug, Clone, Copy)]
pub struct Form<'a, Id> {
    fieldsets: &'a [Fieldset<'a, Id>],
    system: &'a DesignSystem,
    focused_field: Option<&'a Id>,
    layout: FormLayout,
    /// Paint error summary strip above sections when any Error status exists.
    show_error_summary: bool,
}

impl<'a, Id> Form<'a, Id> {
    /// Fieldsets + design system.
    #[must_use]
    pub const fn new(fieldsets: &'a [Fieldset<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            fieldsets,
            system,
            focused_field: None,
            layout: FormLayout::Responsive,
            show_error_summary: true,
        }
    }

    /// Host-owned focused field for chrome.
    #[must_use]
    pub const fn focused_field(mut self, id: Option<&'a Id>) -> Self {
        self.focused_field = id;
        self
    }

    /// Layout recipe.
    #[must_use]
    pub const fn layout(mut self, layout: FormLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Compact rows.
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.layout = FormLayout::Compact;
        self
    }

    /// Stacked single column.
    #[must_use]
    pub const fn stacked(mut self) -> Self {
        self.layout = FormLayout::Stacked;
        self
    }

    /// Inline-preferring layout.
    #[must_use]
    pub const fn inline(mut self) -> Self {
        self.layout = FormLayout::Inline;
        self
    }

    /// Toggle error summary strip.
    #[must_use]
    pub const fn show_error_summary(mut self, on: bool) -> Self {
        self.show_error_summary = on;
        self
    }

    /// Borrowed fieldsets.
    #[must_use]
    pub const fn fieldsets(&self) -> &'a [Fieldset<'a, Id>] {
        self.fieldsets
    }
}

const COLUMN_GAP: u16 = 2;

impl<Id: Clone + PartialEq> StatefulWidget for &Form<'_, Id> {
    type State = FormState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        state.field_regions.clear();
        state.scrollbar_region = None;
        state.error_summary_count = 0;
        state.viewport_height = usize::from(area.height);
        if area.is_empty() || self.fieldsets.is_empty() {
            state.offset = 0;
            state.content_height = 0;
            state.column_count = 1;
            return;
        }

        let errors = collect_errors(self.fieldsets);
        let summary_rows = if self.show_error_summary && !errors.is_empty() {
            1usize.saturating_add(errors.len().min(3))
        } else {
            0
        };

        let (initial_columns, body_height) = dimensions(self.fieldsets, area.width, self.layout);
        let content_height = body_height.saturating_add(summary_rows);
        let show_scrollbar = content_height > usize::from(area.height) && area.width > 1;
        let content_area = Rect {
            width: area.width.saturating_sub(u16::from(show_scrollbar)),
            ..area
        };
        let (columns, body_height) = if show_scrollbar {
            dimensions(self.fieldsets, content_area.width, self.layout)
        } else {
            (initial_columns, body_height)
        };
        let content_height = body_height.saturating_add(summary_rows);
        state.column_count = columns;
        state.content_height = content_height;

        if let Some(ref id) = state.ensure_visible
            && let Some((top, bottom)) =
                field_bounds(self.fieldsets, columns, self.layout, summary_rows, id)
        {
            if top < state.offset {
                state.offset = top;
            } else if bottom > state.offset.saturating_add(state.viewport_height) {
                state.offset = bottom.saturating_sub(state.viewport_height);
            }
        }
        state.ensure_visible = None;
        state.offset = state
            .offset
            .min(max_offset(content_height, state.viewport_height));

        let mut content_y = 0usize;

        // Error summary
        if summary_rows > 0 {
            state.error_summary_count = summary_rows;
            paint_string(
                buffer,
                content_area,
                state.offset,
                content_y,
                content_area.x,
                "Errors",
                self.system.style(Role::Danger).add_modifier(Modifier::BOLD),
            );
            content_y = content_y.saturating_add(1);
            for (label, msg) in errors.iter().take(3) {
                let line = format!("• {label}: {msg}");
                let text = take_display_cols(&line, usize::from(content_area.width));
                paint_string(
                    buffer,
                    content_area,
                    state.offset,
                    content_y,
                    content_area.x,
                    &text,
                    self.system.style(Role::Danger),
                );
                content_y = content_y.saturating_add(1);
            }
        }

        let column_width = if columns == 2 {
            content_area.width.saturating_sub(COLUMN_GAP) / 2
        } else {
            content_area.width
        };
        let field_h = self.layout.field_row_height();
        let header_h = self.layout.section_header_height();

        for section in self.fieldsets {
            paint_string(
                buffer,
                content_area,
                state.offset,
                content_y,
                content_area.x,
                section.legend,
                self.system
                    .style(Role::TextStrong)
                    .add_modifier(Modifier::BOLD),
            );
            content_y = content_y.saturating_add(header_h);
            if let Some(desc) = section.description {
                if !desc.is_empty() && header_h >= 2 {
                    // description shared on header second line when stacked
                    let text = take_display_cols(desc, usize::from(content_area.width));
                    paint_string(
                        buffer,
                        content_area,
                        state.offset,
                        content_y.saturating_sub(1),
                        content_area.x,
                        &text,
                        self.system.style(Role::TextMuted),
                    );
                }
            }

            for (index, field) in section.fields.iter().enumerate() {
                let column = index % usize::from(columns);
                let row = index / usize::from(columns);
                let field_y = content_y.saturating_add(row.saturating_mul(field_h));
                let visible_start = state.offset;
                let visible_end = visible_start.saturating_add(state.viewport_height);
                if field_y >= visible_end
                    || field_y.saturating_add(field_h.saturating_sub(1)) <= visible_start
                {
                    continue;
                }
                let x = content_area.x.saturating_add(
                    u16::try_from(column)
                        .unwrap_or(u16::MAX)
                        .saturating_mul(column_width.saturating_add(COLUMN_GAP)),
                );
                let field_area = Rect::new(x, content_area.y, column_width, 3);
                let is_focused =
                    state.accepts_input && self.focused_field.is_some_and(|id| id == &field.id);
                paint_field(
                    buffer,
                    content_area,
                    state.offset,
                    field_y,
                    field_area,
                    field,
                    self.system,
                    self.layout,
                    is_focused,
                    state.hovered.as_ref() == Some(&field.id),
                );
                let visible = visible_rect(
                    content_area,
                    state.offset,
                    field_y,
                    field_h.saturating_sub(1),
                    x,
                    column_width,
                );
                let label = visible_rect(content_area, state.offset, field_y, 1, x, column_width);
                let value = visible_rect(
                    content_area,
                    state.offset,
                    field_y.saturating_add(1),
                    1,
                    x,
                    column_width,
                );
                let supporting = visible_rect(
                    content_area,
                    state.offset,
                    field_y.saturating_add(2),
                    1,
                    x,
                    column_width,
                );
                if let Some(area) = visible {
                    state.field_regions.push(FormFieldRegion {
                        id: field.id.clone(),
                        area,
                        label,
                        value,
                        supporting,
                    });
                }
                if field.enabled
                    && let Some(visible) = visible
                {
                    state.regions.push(HitRegion {
                        id: field.id.clone(),
                        area: visible,
                    });
                }
            }
            let rows = section.fields.len().div_ceil(usize::from(columns));
            content_y = content_y.saturating_add(rows.saturating_mul(field_h));
        }

        if show_scrollbar {
            let scrollbar = Rect::new(area.right().saturating_sub(1), area.y, 1, area.height);
            state.scrollbar_region = Some(scrollbar);
            crate::scroll::paint_list_scrollbar(
                buffer,
                scrollbar,
                content_height,
                state.viewport_height,
                u16::try_from(state.offset).unwrap_or(u16::MAX),
                self.system,
            );
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for Form<'_, Id> {
    type State = FormState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

fn columns_for(width: u16, layout: FormLayout) -> u8 {
    match layout {
        FormLayout::Stacked | FormLayout::Compact => 1,
        FormLayout::Inline => {
            if width >= 48 {
                1
            } else {
                1
            }
        }
        FormLayout::Responsive => {
            let class = ResponsiveSurface::Form.classify(width, 24);
            let policy_cols = ResponsiveSurface::Form.form_columns(width);
            let template_cols = form_grid_template(width).col_count();
            if policy_cols >= 2
                && template_cols >= 2
                && class.anatomy.multi_pane
                && !class.anatomy.line_mode
            {
                2
            } else {
                1
            }
        }
    }
}

fn dimensions<Id>(fieldsets: &[Fieldset<'_, Id>], width: u16, layout: FormLayout) -> (u8, usize) {
    let columns = columns_for(width, layout);
    let header = layout.section_header_height();
    let field_h = layout.field_row_height();
    let height = fieldsets.iter().fold(0usize, |height, section| {
        height.saturating_add(header).saturating_add(
            section
                .fields
                .len()
                .div_ceil(usize::from(columns))
                .saturating_mul(field_h),
        )
    });
    (columns, height)
}

fn field_bounds<Id: PartialEq>(
    fieldsets: &[Fieldset<'_, Id>],
    columns: u8,
    layout: FormLayout,
    summary_rows: usize,
    focused: &Id,
) -> Option<(usize, usize)> {
    let header = layout.section_header_height();
    let field_h = layout.field_row_height();
    let mut content_y = summary_rows;
    for section in fieldsets {
        content_y = content_y.saturating_add(header);
        if let Some(index) = section.fields.iter().position(|field| &field.id == focused) {
            let top = content_y.saturating_add(index / usize::from(columns) * field_h);
            return Some((top, top.saturating_add(field_h.saturating_sub(1))));
        }
        content_y = content_y.saturating_add(
            section
                .fields
                .len()
                .div_ceil(usize::from(columns))
                .saturating_mul(field_h),
        );
    }
    None
}

#[expect(
    clippy::too_many_arguments,
    reason = "paint projection keeps Form public API small"
)]
fn paint_field<Id: Clone>(
    buffer: &mut Buffer,
    viewport: Rect,
    offset: usize,
    content_y: usize,
    field_area: Rect,
    field: &Field<'_, Id>,
    system: &DesignSystem,
    layout: FormLayout,
    focused: bool,
    hovered: bool,
) {
    let invalid = field.is_invalid();
    let label_row = visible_rect(
        viewport,
        offset,
        content_y,
        1,
        field_area.x,
        field_area.width,
    );
    if let Some(row) = label_row {
        let marker = if field.required {
            Some("*")
        } else if field.dirty {
            Some("·")
        } else {
            None
        };
        let annotation = if field.read_only {
            Some("read only")
        } else if field.show_optional && !field.required {
            Some("optional")
        } else {
            None
        };
        let label_cols = if matches!(layout, FormLayout::Inline) && row.width >= 20 {
            (row.width / 3).max(8)
        } else {
            8
        };
        let mut field_row = FieldRow::new(system, field.label, field.value.clone())
            .label_cols(label_cols)
            .required(field.required)
            .selected(focused)
            .hovered(hovered)
            .enabled(field.enabled && !field.read_only)
            .invalid(invalid);
        if let Some(marker) = marker {
            field_row = field_row.marker(marker);
        }
        if let Some(annotation) = annotation {
            field_row = field_row.annotation(annotation);
        }
        field_row.paint(row, buffer);
    }

    let supporting_y = content_y.saturating_add(2);
    if let Some(row) = visible_rect(
        viewport,
        offset,
        supporting_y,
        1,
        field_area.x,
        field_area.width,
    ) {
        match field.status {
            FieldStatus::Error(msg) => {
                let _ = Description::error(msg, system)
                    .for_id(field.id.clone())
                    .paint(row, buffer);
            }
            FieldStatus::Warning(msg) => {
                let _ = Description::new(msg, system)
                    .for_id(field.id.clone())
                    .paint(row, buffer);
            }
            FieldStatus::Pending(msg) => {
                let line = format!("… {msg}");
                let text = take_display_cols(&line, usize::from(row.width));
                buffer.set_stringn(
                    row.x,
                    row.y,
                    &text,
                    usize::from(row.width),
                    system.style(Role::TextMuted),
                );
            }
            FieldStatus::Help(msg) => {
                let _ = Description::new(msg, system)
                    .for_id(field.id.clone())
                    .paint(row, buffer);
            }
            FieldStatus::None => {
                if let Some(desc) = field.description {
                    let _ = Description::new(desc, system)
                        .for_id(field.id.clone())
                        .paint(row, buffer);
                }
            }
        }
    }
}

fn paint_string(
    buffer: &mut Buffer,
    viewport: Rect,
    offset: usize,
    content_y: usize,
    x: u16,
    text: &str,
    style: Style,
) {
    let Some(y) = visible_y(viewport, offset, content_y) else {
        return;
    };
    let width = viewport.right().saturating_sub(x);
    if width == 0 {
        return;
    }
    let clipped = take_display_cols(text, usize::from(width));
    buffer.set_stringn(x, y, &clipped, usize::from(width), style);
    let _ = display_cols(&clipped);
}

fn visible_y(viewport: Rect, offset: usize, content_y: usize) -> Option<u16> {
    let relative = content_y.checked_sub(offset)?;
    if relative >= usize::from(viewport.height) {
        return None;
    }
    Some(
        viewport
            .y
            .saturating_add(u16::try_from(relative).unwrap_or(u16::MAX)),
    )
}

fn visible_rect(
    viewport: Rect,
    offset: usize,
    content_y: usize,
    height: usize,
    x: u16,
    width: u16,
) -> Option<Rect> {
    let visible_start = offset;
    let visible_end = offset.saturating_add(usize::from(viewport.height));
    let top = content_y.max(visible_start);
    let bottom = content_y.saturating_add(height).min(visible_end);
    if top >= bottom || width == 0 {
        return None;
    }
    let y = viewport
        .y
        .saturating_add(u16::try_from(top.saturating_sub(offset)).unwrap_or(u16::MAX));
    let h = u16::try_from(bottom.saturating_sub(top)).unwrap_or(0);
    if h == 0 {
        None
    } else {
        Some(Rect::new(x, y, width, h))
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::interaction::default_form_intent;
    use ratatui_core::layout::Rect;

    fn sample_fields() -> [Field<'static, &'static str>; 3] {
        [
            Field::new("name", "Name", "Ada")
                .required(true)
                .help("Display name")
                .dirty(true)
                .touched(true),
            Field::new("endpoint", "Endpoint", "")
                .required(true)
                .error("required")
                .touched(true),
            Field::new("mode", "Mode", "off").enabled(false),
        ]
    }

    #[test]
    fn tab_does_not_change_focus_authority() {
        let fields = sample_fields();
        let sections = [Fieldset::new("General", &fields)];
        let mut state = FormState::new();
        let out = state.handle_key(
            &sections,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            Some(&"name"),
        );
        assert_eq!(out, FormOutcome::Ignored);
    }

    #[test]
    fn enter_activates_host_focused_field() {
        let fields = sample_fields();
        let sections = [Fieldset::new("General", &fields)];
        let mut state = FormState::new();
        let out = state.handle_key(
            &sections,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            Some(&"name"),
        );
        assert_eq!(out, FormOutcome::Activated("name"));
    }

    #[test]
    fn submit_and_reset_chords() {
        let fields = sample_fields();
        let sections = [Fieldset::new("General", &fields)];
        let mut state = FormState::new();
        let out = state.handle_key(
            &sections,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
            Some(&"name"),
        );
        assert_eq!(out, FormOutcome::SubmitRequested);
        let out = state.handle_key(
            &sections,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
            Some(&"name"),
        );
        assert_eq!(out, FormOutcome::ResetRequested);
    }

    #[test]
    fn first_invalid_and_error_summary() {
        let fields = sample_fields();
        let sections = [Fieldset::new("General", &fields)];
        assert_eq!(first_invalid_id(&sections), Some(&"endpoint"));
        let errs = collect_errors(&sections);
        assert_eq!(errs.len(), 1);
        assert!(any_dirty(&sections));
        assert!(any_touched(&sections));
        assert!(!required_filled(&sections));
    }

    #[test]
    fn focus_first_invalid_sets_ensure_visible() {
        let fields = sample_fields();
        let sections = [Fieldset::new("General", &fields)];
        let mut state = FormState::new();
        assert_eq!(state.focus_first_invalid(&sections), Some("endpoint"));
    }

    #[test]
    fn paint_exposes_regions_and_summary() {
        let system = DesignSystem::default();
        let fields = sample_fields();
        let sections = [Fieldset::new("General", &fields).description("Profile")];
        let form = Form::new(&sections, &system).focused_field(Some(&"name"));
        let mut state = FormState::new();
        let area = Rect::new(0, 0, 40, 16);
        let mut buf = Buffer::empty(area);
        form.render(area, &mut buf, &mut state);
        assert!(!state.field_regions().is_empty());
        assert!(state.error_summary_count() >= 1);
    }

    #[test]
    fn compact_and_stacked_layouts() {
        let system = DesignSystem::default();
        let fields = sample_fields();
        let sections = [Fieldset::new("G", &fields)];
        let mut state = FormState::new();
        let area = Rect::new(0, 0, 36, 14);
        let mut buf = Buffer::empty(area);
        Form::new(&sections, &system)
            .compact()
            .render(area, &mut buf, &mut state);
        Form::new(&sections, &system)
            .stacked()
            .render(area, &mut buf, &mut state);
        Form::new(&sections, &system)
            .inline()
            .render(area, &mut buf, &mut state);
        assert!(state.content_height() > 0);
    }

    #[test]
    fn typed_values_render_masked_and_unset_rows() {
        let system = DesignSystem::default();
        let fields = [
            Field::new("token", "Token", "ignored").masked(5),
            Field::new("region", "Region", "")
                .unset("required")
                .required(true),
        ];
        let sections = [Fieldset::new("Auth", &fields)];
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);
        Form::new(&sections, &system).render(area, &mut buf, &mut FormState::new());
        let text = buf
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("●●●●●"));
        assert!(text.contains("required"));
    }

    #[test]
    fn default_form_intent_has_no_y_grant_and_maps_activate() {
        assert_eq!(
            default_form_intent(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(UiIntent::Activate)
        );
        assert_eq!(
            default_form_intent(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)),
            None
        );
        assert_eq!(
            default_form_intent(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            None
        );
    }

    #[test]
    fn disabled_field_not_activated() {
        let fields = sample_fields();
        let sections = [Fieldset::new("G", &fields)];
        let mut state = FormState::new();
        let out = state.handle_key(
            &sections,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            Some(&"mode"),
        );
        assert_eq!(out, FormOutcome::Ignored);
    }
}

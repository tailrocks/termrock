// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Responsive multi-field form surface.
//!
//! **Focus law (Break F):** field focus is **host / [`InteractionScene`] owned**.
//! Pass the focused field id into [`Form::focused_field`] for paint. Tab/arrow
//! field cycling is **not** handled here — register fields on the scene and use
//! `handle_key_tab_esc` / `focus_move`. This widget only activates, scrolls, and
//! paints.

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    interaction::{HitRegion, NavigationMove, PageMove, UiIntent},
    layout::{form_grid_template, ResponsiveSurface},
    scroll::max_offset,
    style::{DesignSystem, Role},
};

const FIELD_HEIGHT: usize = 4;
const SECTION_HEADER_HEIGHT: usize = 2;
const COLUMN_GAP: u16 = 2;

/// A stable form field with label, value, and validation metadata.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct FormField<'a, Id> {
    /// Stable identity used for selection and activation.
    pub id: Id,
    /// Caller-visible label.
    pub label: Line<'a>,
    /// Caller-owned value displayed by this item.
    pub value: Line<'a>,
    /// Caller-visible help.
    pub help: Option<Line<'a>>,
    /// Optional validation error shown for this field.
    pub error: Option<Line<'a>>,
    /// Whether this item is required.
    pub required: bool,
    /// Whether this item is enabled.
    pub enabled: bool,
}

impl<'a, Id> FormField<'a, Id> {
    /// Creates a field with no help text and valid initial state.
    #[must_use]
    pub const fn new(id: Id, label: Line<'a>, value: Line<'a>) -> Self {
        Self {
            id,
            label,
            value,
            help: None,
            error: None,
            required: false,
            enabled: true,
        }
    }

    /// Sets supplemental help text.
    #[must_use]
    pub fn help(mut self, help: Line<'a>) -> Self {
        self.help = Some(help);
        self
    }

    /// Sets validation error text.
    #[must_use]
    pub fn error(mut self, error: Line<'a>) -> Self {
        self.error = Some(error);
        self
    }

    /// Marks the field as required or optional.
    #[must_use]
    pub const fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Sets whether this item can receive interaction.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// A labeled group of form fields.
#[derive(Debug, Clone)]
pub struct FormSection<'a, Id> {
    /// Caller-visible title.
    pub title: Line<'a>,
    /// Borrowed fields rendered in caller order.
    pub fields: &'a [FormField<'a, Id>],
}

/// Semantic results produced by form interaction (no field-focus authority).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormOutcome<Id> {
    /// The event produced no form-state change.
    Ignored,
    /// The identified enabled field requested activation (Enter / re-click).
    Activated(Id),
    /// Viewport scrolled.
    Scrolled,
}

/// Painted hit geometry for one form field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormFieldRegion<Id> {
    /// Stable identity used for selection and activation.
    pub id: Id,
    /// Painted terminal rectangle used for hit testing.
    pub area: Rect,
    /// Caller-visible label.
    pub label: Option<Rect>,
    /// Caller-owned value displayed by this item.
    pub value: Option<Rect>,
    /// Union geometry for supporting help or error text.
    pub supporting: Option<Rect>,
}

/// Runtime state for [`Form`] — scroll, hover, hit geometry; **not** field focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormState<Id> {
    hovered: Option<Id>,
    /// Whether the form surface accepts keyboard/pointer activate (host gate).
    accepts_input: bool,
    offset: usize,
    viewport_height: usize,
    content_height: usize,
    column_count: u8,
    /// When set, next paint scrolls this id into view (host sets after scene focus).
    ensure_visible: Option<Id>,
    regions: Vec<HitRegion<Id>>,
    field_regions: Vec<FormFieldRegion<Id>>,
    scrollbar_region: Option<Rect>,
}

impl<Id> Default for FormState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> FormState<Id> {
    /// Creates form state at the top of the viewport (inactive until host enables).
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
        }
    }

    /// Returns the stable identity currently under the pointer.
    #[must_use]
    pub const fn hovered(&self) -> Option<&Id> {
        self.hovered.as_ref()
    }

    /// Whether the form currently accepts interaction.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Enables or disables form keyboard and pointer interaction (whole surface).
    pub const fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Deprecated alias for [`Self::set_accepts_input`].
    #[deprecated(note = "use set_accepts_input")]
    pub const fn set_active(&mut self, active: bool) {
        self.accepts_input = active;
    }

    /// Deprecated alias for [`Self::accepts_input`].
    #[deprecated(note = "use accepts_input")]
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.accepts_input
    }

    /// Ask the next paint to scroll `id` into view (after host scene focus change).
    pub fn ensure_visible(&mut self, id: Option<Id>) {
        self.ensure_visible = id;
    }

    /// Returns the zero-based top row of the form viewport.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the number of columns selected by responsive layout.
    #[must_use]
    pub const fn column_count(&self) -> u8 {
        self.column_count
    }

    /// Returns the rendered content height in terminal rows.
    #[must_use]
    pub const fn content_height(&self) -> usize {
        self.content_height
    }

    /// Returns the hit regions produced by the most recent render.
    #[must_use]
    pub fn regions(&self) -> &[HitRegion<Id>] {
        &self.regions
    }

    /// Returns field hit regions produced by the most recent render.
    #[must_use]
    pub fn field_regions(&self) -> &[FormFieldRegion<Id>] {
        &self.field_regions
    }

    /// Moves the scroll position by a signed delta and clamps it to valid content.
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

    /// Scrolls toward a pointer position within the painted viewport.
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

impl<Id: Clone + PartialEq> FormState<Id> {
    /// Routes activation and scroll keys. **Does not move field focus.**
    ///
    /// `focused_field` is the host scene focus id (or `None` if form does not own input).
    pub fn handle_key(
        &mut self,
        sections: &[FormSection<'_, Id>],
        key: KeyEvent,
        focused_field: Option<&Id>,
    ) -> FormOutcome<Id> {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return FormOutcome::Ignored;
        }
        if let Some(intent) = crate::interaction::default_form_intent(key) {
            return self.handle_intent(sections, intent, focused_field);
        }
        FormOutcome::Ignored
    }

    /// Semantic intent routing (activate + page scroll only).
    pub fn handle_intent(
        &mut self,
        sections: &[FormSection<'_, Id>],
        intent: UiIntent,
        focused_field: Option<&Id>,
    ) -> FormOutcome<Id> {
        if !self.accepts_input {
            return FormOutcome::Ignored;
        }
        match intent {
            UiIntent::Activate | UiIntent::Submit => {
                let Some(id) = focused_field else {
                    return FormOutcome::Ignored;
                };
                sections
                    .iter()
                    .flat_map(|section| section.fields)
                    .find(|field| field.enabled && &field.id == id)
                    .map_or(FormOutcome::Ignored, |field| {
                        FormOutcome::Activated(field.id.clone())
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
            // Field cycle is host/scene owned.
            UiIntent::Move(_)
            | UiIntent::Toggle
            | UiIntent::Open
            | UiIntent::Close
            | UiIntent::Cancel
            | UiIntent::Expand
            | UiIntent::Collapse => FormOutcome::Ignored,
            _ => FormOutcome::Ignored,
        }
    }

    /// Updates hover state from the current pointer position and painted hit regions.
    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        self.hovered = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    /// Activate if the hit field is already the host-focused field; else `Ignored`.
    ///
    /// Host must call `scene.focus(hit_id)` when this returns `Ignored` for a hit.
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
            // Do not steal focus authority — host focuses via scene.
            FormOutcome::Ignored
        }
    }

    /// Hit-test helper: field id under position (for host scene.focus).
    #[must_use]
    pub fn hit_id(&self, position: Position) -> Option<&Id> {
        self.regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| &region.id)
    }
}

/// A responsive, navigable form assembled from borrowed sections.
#[derive(Debug, Clone, Copy)]
pub struct Form<'a, Id> {
    sections: &'a [FormSection<'a, Id>],
    system: &'a DesignSystem,
    /// Host scene focus id for field chrome (not stored in state).
    focused_field: Option<&'a Id>,
}

impl<'a, Id> Form<'a, Id> {
    /// Creates a form over the supplied sections and design system.
    #[must_use]
    pub const fn new(sections: &'a [FormSection<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            sections,
            system,
            focused_field: None,
        }
    }

    /// Sets the host-owned focused field id for paint (typically `scene.focused()`).
    #[must_use]
    pub const fn focused_field(mut self, id: Option<&'a Id>) -> Self {
        self.focused_field = id;
        self
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &Form<'_, Id> {
    type State = FormState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        state.field_regions.clear();
        state.scrollbar_region = None;
        state.viewport_height = usize::from(area.height);
        if area.is_empty() || self.sections.is_empty() {
            state.offset = 0;
            state.content_height = 0;
            state.column_count = 1;
            return;
        }

        let (initial_columns, initial_height) = dimensions(self.sections, area.width);
        let show_scrollbar = initial_height > usize::from(area.height) && area.width > 1;
        let content_area = Rect {
            width: area.width.saturating_sub(u16::from(show_scrollbar)),
            ..area
        };
        let (columns, content_height) = if show_scrollbar {
            dimensions(self.sections, content_area.width)
        } else {
            (initial_columns, initial_height)
        };
        state.column_count = columns;
        state.content_height = content_height;

        if let Some(ref id) = state.ensure_visible
            && let Some((top, bottom)) = field_bounds(self.sections, columns, id)
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

        let column_width = if columns == 2 {
            content_area.width.saturating_sub(COLUMN_GAP) / 2
        } else {
            content_area.width
        };
        let mut content_y = 0usize;
        for section in self.sections {
            paint_line(
                buffer,
                content_area,
                state.offset,
                content_y,
                &section.title,
                self.system.style(Role::TextStrong),
            );
            content_y = content_y.saturating_add(SECTION_HEADER_HEIGHT);
            for (index, field) in section.fields.iter().enumerate() {
                let column = index % usize::from(columns);
                let row = index / usize::from(columns);
                let field_y = content_y.saturating_add(row.saturating_mul(FIELD_HEIGHT));
                let visible_start = state.offset;
                let visible_end = visible_start.saturating_add(state.viewport_height);
                if field_y >= visible_end
                    || field_y.saturating_add(FIELD_HEIGHT.saturating_sub(1)) <= visible_start
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
                    is_focused,
                    state.hovered.as_ref() == Some(&field.id),
                );
                let visible = visible_rect(
                    content_area,
                    state.offset,
                    field_y,
                    FIELD_HEIGHT.saturating_sub(1),
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
            content_y = content_y.saturating_add(rows.saturating_mul(FIELD_HEIGHT));
        }

        if show_scrollbar {
            let scrollbar = Rect::new(area.right().saturating_sub(1), area.y, 1, area.height);
            state.scrollbar_region = Some(scrollbar);
            for y in scrollbar.top()..scrollbar.bottom() {
                buffer.set_string(scrollbar.x, y, "│", self.system.style(Role::ScrollTrack));
            }
            if let Some(thumb) = crate::scroll::full_cell_thumb(
                content_height,
                state.viewport_height,
                scrollbar.height,
                state.offset,
            ) {
                for y in thumb.start..thumb.start.saturating_add(thumb.len) {
                    buffer.set_string(
                        scrollbar.x,
                        scrollbar.y.saturating_add(y),
                        "█",
                        self.system.style(Role::ScrollThumb),
                    );
                }
            }
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for Form<'_, Id> {
    type State = FormState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

fn columns_for(width: u16) -> u8 {
    // Grid template + Form surface policy (multi-pane anatomy must allow).
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

fn dimensions<Id>(sections: &[FormSection<'_, Id>], width: u16) -> (u8, usize) {
    let columns = columns_for(width);
    let height = sections.iter().fold(0usize, |height, section| {
        height.saturating_add(SECTION_HEADER_HEIGHT).saturating_add(
            section
                .fields
                .len()
                .div_ceil(usize::from(columns))
                .saturating_mul(FIELD_HEIGHT),
        )
    });
    (columns, height)
}

fn field_bounds<Id: PartialEq>(
    sections: &[FormSection<'_, Id>],
    columns: u8,
    focused: &Id,
) -> Option<(usize, usize)> {
    let mut content_y = 0usize;
    for section in sections {
        content_y = content_y.saturating_add(SECTION_HEADER_HEIGHT);
        if let Some(index) = section.fields.iter().position(|field| &field.id == focused) {
            let top = content_y.saturating_add(index / usize::from(columns) * FIELD_HEIGHT);
            return Some((top, top.saturating_add(FIELD_HEIGHT.saturating_sub(1))));
        }
        content_y = content_y.saturating_add(
            section
                .fields
                .len()
                .div_ceil(usize::from(columns))
                .saturating_mul(FIELD_HEIGHT),
        );
    }
    None
}

#[expect(
    clippy::too_many_arguments,
    reason = "paint projection keeps Form public API small"
)]
fn paint_field<Id>(
    buffer: &mut Buffer,
    viewport: Rect,
    offset: usize,
    content_y: usize,
    field_area: Rect,
    field: &FormField<'_, Id>,
    system: &DesignSystem,
    focused: bool,
    hovered: bool,
) {
    let mut label_style = if field.enabled {
        system.style(Role::Text)
    } else {
        system.style(Role::TextDisabled).add_modifier(Modifier::DIM)
    };
    let mut value_style = if field.error.is_some() {
        system.style(Role::InputInvalid)
    } else {
        system.style(Role::Input)
    };
    if focused {
        label_style = label_style.add_modifier(Modifier::BOLD);
        value_style = value_style.patch(system.style(Role::Focus));
        // Non-color focus cue: leading glyph on value row.
        paint_string(
            buffer,
            viewport,
            offset,
            content_y.saturating_add(1),
            field_area.x,
            "›",
            system.style(Role::Focus),
        );
    } else if hovered && field.enabled {
        label_style = label_style.add_modifier(Modifier::UNDERLINED);
    }
    if !field.enabled {
        value_style = value_style
            .patch(system.style(Role::TextDisabled))
            .add_modifier(Modifier::DIM);
    }

    let mut label = field.label.clone();
    if field.required {
        label.push_span(" *");
    }
    if !field.enabled {
        label.push_span(" ⊘");
    }
    paint_line(buffer, viewport, offset, content_y, &label, label_style);
    let value_x = if focused {
        field_area.x.saturating_add(2)
    } else {
        field_area.x
    };
    let value_width = field_area.width.saturating_sub(if focused { 2 } else { 0 });
    paint_line_at(
        buffer,
        viewport,
        offset,
        content_y.saturating_add(1),
        value_x,
        value_width,
        &field.value,
        value_style,
    );
    let supporting = field
        .error
        .as_ref()
        .or(field.help.as_ref())
        .map(|line| (line, field.error.is_some()));
    if let Some((line, is_error)) = supporting {
        let style = if is_error {
            system.style(Role::Danger)
        } else {
            system.style(Role::TextMuted)
        };
        paint_line(
            buffer,
            viewport,
            offset,
            content_y.saturating_add(2),
            line,
            style,
        );
    }
}

fn paint_line(
    buffer: &mut Buffer,
    viewport: Rect,
    offset: usize,
    content_y: usize,
    line: &Line<'_>,
    style: Style,
) {
    paint_line_at(
        buffer,
        viewport,
        offset,
        content_y,
        viewport.x,
        viewport.width,
        line,
        style,
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "clipped line painting is explicit"
)]
fn paint_line_at(
    buffer: &mut Buffer,
    viewport: Rect,
    offset: usize,
    content_y: usize,
    x: u16,
    width: u16,
    line: &Line<'_>,
    style: Style,
) {
    let Some(y) = visible_y(viewport, offset, content_y) else {
        return;
    };
    if width == 0 {
        return;
    }
    buffer.set_line(x, y, line, width);
    buffer.set_style(Rect::new(x, y, width, 1), style);
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
    buffer.set_stringn(x, y, text, usize::from(width), style);
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

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::input::KeyModifiers;
    use crate::interaction::default_form_intent;

    #[test]
    fn tab_does_not_change_focus_authority() {
        let fields = [FormField::new("a", Line::from("A"), Line::from("1"))];
        let sections = [FormSection {
            title: Line::from("S"),
            fields: &fields,
        }];
        let mut state = FormState::new();
        let out = state.handle_key(
            &sections,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            Some(&"a"),
        );
        assert_eq!(out, FormOutcome::Ignored);
    }

    #[test]
    fn enter_activates_host_focused_field() {
        let fields = [
            FormField::new("a", Line::from("A"), Line::from("1")),
            FormField::new("b", Line::from("B"), Line::from("2")),
        ];
        let sections = [FormSection {
            title: Line::from("S"),
            fields: &fields,
        }];
        let mut state = FormState::new();
        let out = state.handle_key(
            &sections,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            Some(&"b"),
        );
        assert_eq!(out, FormOutcome::Activated("b"));
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
        // Move is host/scene — not form intent.
        assert_eq!(
            default_form_intent(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            None
        );
    }
}

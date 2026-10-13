use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    interaction::HitRegion,
    scroll::max_offset,
    style::{Role, Theme},
};

const FIELD_HEIGHT: usize = 4;
const SECTION_HEADER_HEIGHT: usize = 2;
const COLUMN_GAP: u16 = 2;
const MIN_COLUMN_WIDTH: u16 = 30;

#[derive(Debug, Clone)]
#[non_exhaustive]
/// A stable form field with label, value, and validation metadata.
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
    #[must_use]
    /// Creates a field with no help text and valid initial state.
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

    #[must_use]
    /// Sets supplemental help text.
    pub fn help(mut self, help: Line<'a>) -> Self {
        self.help = Some(help);
        self
    }

    #[must_use]
    /// Sets validation error text.
    pub fn error(mut self, error: Line<'a>) -> Self {
        self.error = Some(error);
        self
    }

    #[must_use]
    /// Marks the field as required or optional.
    pub const fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    #[must_use]
    /// Sets whether this item can receive interaction.
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

#[derive(Debug, Clone)]
/// A labeled group of form fields.
pub struct FormSection<'a, Id> {
    /// Caller-visible title.
    pub title: Line<'a>,
    /// Borrowed fields rendered in caller order.
    pub fields: &'a [FormField<'a, Id>],
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
/// Semantic results produced by form interaction.
pub enum FormOutcome<Id> {
    /// Reports ignored.
    Ignored,
    /// Reports focus changed.
    FocusChanged(Id),
    /// Reports activated.
    Activated(Id),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Painted hit geometry for one form field.
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

#[derive(Debug, Clone, PartialEq, Eq)]
/// Runtime state for `Form`.
pub struct FormState<Id> {
    focused: Option<Id>,
    hovered: Option<Id>,
    active: bool,
    offset: usize,
    viewport_height: usize,
    content_height: usize,
    column_count: u8,
    follow_focus: bool,
    regions: Vec<HitRegion<Id>>,
    field_regions: Vec<FormFieldRegion<Id>>,
    scrollbar_region: Option<Rect>,
}

impl<Id> Default for FormState<Id> {
    fn default() -> Self {
        Self {
            focused: None,
            hovered: None,
            active: false,
            offset: 0,
            viewport_height: 0,
            content_height: 0,
            column_count: 1,
            follow_focus: false,
            regions: Vec::new(),
            field_regions: Vec::new(),
            scrollbar_region: None,
        }
    }
}

impl<Id> FormState<Id> {
    #[must_use]
    /// Creates unfocused form state at the top of the viewport.
    pub const fn new(focused: Option<Id>) -> Self {
        Self {
            focused,
            hovered: None,
            active: true,
            offset: 0,
            viewport_height: 0,
            content_height: 0,
            column_count: 1,
            follow_focus: true,
            regions: Vec::new(),
            field_regions: Vec::new(),
            scrollbar_region: None,
        }
    }

    #[must_use]
    /// Returns whether this focus state currently owns focus.
    pub const fn focused(&self) -> Option<&Id> {
        self.focused.as_ref()
    }

    #[must_use]
    /// Returns the stable identity currently under the pointer.
    pub const fn hovered(&self) -> Option<&Id> {
        self.hovered.as_ref()
    }

    #[must_use]
    /// Returns whether `active`.
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Sets `active`.
    pub const fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    /// Moves focus to the supplied stable identity when it is enabled.
    pub fn focus(&mut self, focused: Option<Id>) {
        self.focused = focused;
        self.follow_focus = true;
    }

    #[must_use]
    /// Returns the signed distance from the live tail in rows.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    #[must_use]
    /// Returns the number of columns selected by responsive layout.
    pub const fn column_count(&self) -> u8 {
        self.column_count
    }

    #[must_use]
    /// Returns the rendered content height in terminal rows.
    pub const fn content_height(&self) -> usize {
        self.content_height
    }

    #[must_use]
    /// Returns the hit regions produced by the most recent render.
    pub fn regions(&self) -> &[HitRegion<Id>] {
        &self.regions
    }

    #[must_use]
    /// Returns field hit regions produced by the most recent render.
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
        self.follow_focus = false;
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
        self.follow_focus = false;
        true
    }
}

impl<Id: Clone + PartialEq> FormState<Id> {
    /// Handles the `handle_key` interaction.
    pub fn handle_key(
        &mut self,
        sections: &[FormSection<'_, Id>],
        key: KeyEvent,
    ) -> FormOutcome<Id> {
        if !self.active || key.kind == KeyEventKind::Release {
            return FormOutcome::Ignored;
        }
        match key.code {
            KeyCode::Tab | KeyCode::Down => self.move_focus(sections, true),
            KeyCode::BackTab | KeyCode::Up => self.move_focus(sections, false),
            KeyCode::Home => self.focus_boundary(sections, false),
            KeyCode::End => self.focus_boundary(sections, true),
            KeyCode::Enter => sections
                .iter()
                .flat_map(|section| section.fields)
                .find(|field| field.enabled && self.focused.as_ref() == Some(&field.id))
                .map_or(FormOutcome::Ignored, |field| {
                    FormOutcome::Activated(field.id.clone())
                }),
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

    /// Maps a pointer position to the semantic outcome of the painted hit region.
    pub fn click(&mut self, position: Position) -> FormOutcome<Id> {
        let Some(id) = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone())
        else {
            return FormOutcome::Ignored;
        };
        if self.focused.as_ref() == Some(&id) {
            FormOutcome::Activated(id)
        } else {
            self.focused = Some(id.clone());
            self.follow_focus = true;
            FormOutcome::FocusChanged(id)
        }
    }

    fn move_focus(&mut self, sections: &[FormSection<'_, Id>], forward: bool) -> FormOutcome<Id> {
        let enabled_count = sections
            .iter()
            .flat_map(|section| section.fields)
            .filter(|field| field.enabled)
            .count();
        if enabled_count == 0 {
            return FormOutcome::Ignored;
        }
        let current = sections
            .iter()
            .flat_map(|section| section.fields)
            .filter(|field| field.enabled)
            .position(|field| self.focused.as_ref() == Some(&field.id));
        let target = match (current, forward) {
            (Some(index), true) => index.saturating_add(1) % enabled_count,
            (Some(index), false) => index.checked_sub(1).unwrap_or(enabled_count - 1),
            (None, true) => 0,
            (None, false) => enabled_count - 1,
        };
        self.focus_enabled_index(sections, target)
    }

    fn focus_boundary(
        &mut self,
        sections: &[FormSection<'_, Id>],
        from_end: bool,
    ) -> FormOutcome<Id> {
        let count = sections
            .iter()
            .flat_map(|section| section.fields)
            .filter(|field| field.enabled)
            .count();
        if count == 0 {
            FormOutcome::Ignored
        } else {
            self.focus_enabled_index(sections, if from_end { count - 1 } else { 0 })
        }
    }

    fn focus_enabled_index(
        &mut self,
        sections: &[FormSection<'_, Id>],
        target: usize,
    ) -> FormOutcome<Id> {
        let Some(id) = sections
            .iter()
            .flat_map(|section| section.fields)
            .filter(|field| field.enabled)
            .nth(target)
            .map(|field| field.id.clone())
        else {
            return FormOutcome::Ignored;
        };
        self.focused = Some(id.clone());
        self.follow_focus = true;
        FormOutcome::FocusChanged(id)
    }
}

#[derive(Debug, Clone, Copy)]
/// A responsive, navigable form assembled from borrowed sections.
pub struct Form<'a, Id> {
    sections: &'a [FormSection<'a, Id>],
    theme: &'a Theme,
}

impl<'a, Id> Form<'a, Id> {
    #[must_use]
    /// Creates a form over the supplied sections and theme.
    pub const fn new(sections: &'a [FormSection<'a, Id>], theme: &'a Theme) -> Self {
        Self { sections, theme }
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

        if state.follow_focus
            && let Some((top, bottom)) = focused_bounds(self.sections, columns, &state.focused)
        {
            if top < state.offset {
                state.offset = top;
            } else if bottom > state.offset.saturating_add(state.viewport_height) {
                state.offset = bottom.saturating_sub(state.viewport_height);
            }
        }
        state.follow_focus = false;
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
                self.theme.style(Role::TextStrong),
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
                paint_field(
                    buffer,
                    content_area,
                    state.offset,
                    field_y,
                    field_area,
                    field,
                    self.theme,
                    state.active && state.focused.as_ref() == Some(&field.id),
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
                buffer.set_string(scrollbar.x, y, "│", self.theme.style(Role::ScrollTrack));
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
                        self.theme.style(Role::ScrollThumb),
                    );
                }
            }
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for Form<'_, Id> {
    type State = FormState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}

fn columns_for(width: u16) -> u8 {
    if width
        >= MIN_COLUMN_WIDTH
            .saturating_mul(2)
            .saturating_add(COLUMN_GAP)
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

fn focused_bounds<Id: PartialEq>(
    sections: &[FormSection<'_, Id>],
    columns: u8,
    focused: &Option<Id>,
) -> Option<(usize, usize)> {
    let mut content_y = 0usize;
    for section in sections {
        content_y = content_y.saturating_add(SECTION_HEADER_HEIGHT);
        if let Some(index) = section
            .fields
            .iter()
            .position(|field| focused.as_ref() == Some(&field.id))
        {
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
    theme: &Theme,
    focused: bool,
    hovered: bool,
) {
    let mut label_style = if field.enabled {
        theme.style(Role::Text)
    } else {
        theme.style(Role::TextDisabled).add_modifier(Modifier::DIM)
    };
    let mut value_style = if field.error.is_some() {
        theme.style(Role::InputInvalid)
    } else {
        theme.style(Role::Input)
    };
    if focused {
        label_style = label_style.add_modifier(Modifier::BOLD);
        value_style = value_style.patch(theme.style(Role::Focus));
    } else if hovered && field.enabled {
        label_style = label_style.add_modifier(Modifier::UNDERLINED);
    }
    if !field.enabled {
        value_style = value_style
            .patch(theme.style(Role::TextDisabled))
            .add_modifier(Modifier::DIM);
    }

    let required_width = u16::from(field.required && field_area.width > 0);
    paint_line_at(
        buffer,
        viewport,
        offset,
        content_y,
        field_area.x,
        field_area.width.saturating_sub(required_width),
        &field.label,
        label_style,
    );
    if field.required && field_area.width > 0 {
        paint_text_at(
            buffer,
            viewport,
            offset,
            content_y,
            field_area.right().saturating_sub(1),
            1,
            "*",
            theme.style(Role::Accent).add_modifier(Modifier::BOLD),
        );
    }
    let disabled_width = u16::from(!field.enabled && field_area.width > 0);
    paint_line_at(
        buffer,
        viewport,
        offset,
        content_y.saturating_add(1),
        field_area.x,
        field_area.width.saturating_sub(disabled_width),
        &field.value,
        value_style,
    );
    if !field.enabled && field_area.width > 0 {
        paint_text_at(
            buffer,
            viewport,
            offset,
            content_y.saturating_add(1),
            field_area.right().saturating_sub(1),
            1,
            "⊘",
            value_style,
        );
    }
    if let Some(error) = &field.error {
        paint_line_at(
            buffer,
            viewport,
            offset,
            content_y.saturating_add(2),
            field_area.x,
            field_area.width,
            error,
            theme.style(Role::Danger),
        );
    } else if let Some(help) = &field.help {
        paint_line_at(
            buffer,
            viewport,
            offset,
            content_y.saturating_add(2),
            field_area.x,
            field_area.width,
            help,
            theme.style(Role::TextMuted),
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
    buffer.set_line(x, y, line, width);
    buffer.set_style(Rect::new(x, y, width, 1), style);
}

#[expect(
    clippy::too_many_arguments,
    reason = "clipped text painting is explicit"
)]
fn paint_text_at(
    buffer: &mut Buffer,
    viewport: Rect,
    offset: usize,
    content_y: usize,
    x: u16,
    width: u16,
    text: &str,
    style: Style,
) {
    let Some(y) = visible_y(viewport, offset, content_y) else {
        return;
    };
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
    let content_bottom = content_y.saturating_add(height);
    let viewport_bottom = offset.saturating_add(usize::from(viewport.height));
    let visible_top = content_y.max(offset);
    let visible_bottom = content_bottom.min(viewport_bottom);
    if visible_top >= visible_bottom || width == 0 {
        return None;
    }
    Some(Rect::new(
        x,
        viewport
            .y
            .saturating_add(u16::try_from(visible_top.saturating_sub(offset)).unwrap_or(u16::MAX)),
        width,
        u16::try_from(visible_bottom.saturating_sub(visible_top)).unwrap_or(u16::MAX),
    ))
}

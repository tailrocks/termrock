use ratatui_core::{
    buffer::Buffer,
    layout::{Margin, Position, Rect},
    style::{Modifier, Style},
    widgets::{StatefulWidget, Widget},
};
use ratatui_widgets::{block::Block, borders::Borders};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    interaction::{
        NavigationMove, Outcome, OverlayId, OverlayOutcome, OverlaySize, OverlaySpec, OverlayStack,
        PageMove, UiIntent, place_overlay,
    },
    style::{DesignSystem, Role, VisualState},
    text::{display_cols, take_display_cols, truncate_cols},
};

use super::{List, ListRow, ListState, RowRole, TextInput, TextInputOutcome, TextInputState};

/// Default overlay id for a select/picker popup on an [`OverlayStack`].
pub const PICKER_OVERLAY_ID: &str = "termrock.picker";

/// Preferred picker popup size before clamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickerSize {
    /// Preferred width in cells.
    pub width: u16,
    /// Preferred height in rows.
    pub height: u16,
}

impl Default for PickerSize {
    fn default() -> Self {
        Self {
            width: 36,
            height: 12,
        }
    }
}

impl From<PickerSize> for OverlaySize {
    fn from(value: PickerSize) -> Self {
        Self {
            width: value.width,
            height: value.height,
            min_width: 16,
            min_height: 4,
            max_width: 0,
            max_height: 0,
        }
    }
}

/// Place a picker/select popup under `anchor` (Select kind policy).
#[must_use]
pub fn place_picker(bounds: Rect, anchor: Rect, preferred: PickerSize) -> Rect {
    place_overlay(
        bounds,
        Some(anchor),
        OverlaySize::from(preferred),
        crate::interaction::OverlayPolicy::for_kind(crate::interaction::OverlayKind::Select),
    )
}

/// Centered in the upper third of `bounds` (command-palette / quick-open class).
#[must_use]
pub fn place_picker_modal(bounds: Rect, preferred: PickerSize) -> Rect {
    place_upper_third(bounds, preferred.width, preferred.height)
}

fn place_upper_third(bounds: Rect, width: u16, height: u16) -> Rect {
    if bounds.is_empty() || width == 0 || height == 0 {
        return Rect::default();
    }
    let width = width.min(bounds.width).max(1);
    let height = height.min(bounds.height).max(1);
    let x = bounds
        .x
        .saturating_add(bounds.width.saturating_sub(width) / 2);
    let y = bounds
        .y
        .saturating_add((bounds.height.saturating_sub(height) / 3).max(1));
    Rect::new(
        x,
        y.min(bounds.bottom().saturating_sub(height)),
        width,
        height,
    )
}

/// Open a select-kind picker popup on the overlay stack.
pub fn open_picker_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    preferred: PickerSize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::select(
            PICKER_OVERLAY_ID,
            anchor,
            OverlaySize::from(preferred),
            opener_focus,
        ),
    )
}

/// Dismiss the default picker overlay id.
pub fn dismiss_picker_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(PICKER_OVERLAY_ID))
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
/// Semantic results produced by picker interaction.
pub enum PickerOutcome<Id> {
    /// The input produced no picker action.
    Ignored,
    /// Query text or its cursor changed; the caller should rebuild its projection.
    QueryChanged,
    /// Result-list cursor moved (not scene surface focus).
    CursorMoved,
    /// The selected visible identity was activated.
    Activated(Id),
    /// Alt+Enter: open the selected identity in a new tab (junie `ChosenAlt`).
    ActivatedAlt(Id),
    /// Tab: host should cycle the picker scope (junie `NextScope`).
    NextScope,
    /// Delete: host secondary (junie `PickerEvent::Secondary`, close-tab).
    Secondary(Id),
    /// Escape was pressed while the query was already empty.
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Query and stable-selection state for [`Picker`].
///
/// Callers own matching, scoring, ordering, candidate lifecycle, and labels.
/// Rebuild the visible [`ListRow`] projection after [`PickerOutcome::QueryChanged`],
/// then call [`Self::reconcile`] before rendering or handling another key.
///
/// Scene/overlay surface focus is host-owned via [`Self::set_accepts_input`].
pub struct PickerState<Id> {
    query: TextInputState,
    list: ListState<Id>,
    previous_visible: Vec<Id>,
    accepts_input: bool,
    searchable: bool,
}

impl<Id: Clone + PartialEq> PickerState<Id> {
    /// Creates empty query state with an optional stable selection.
    #[must_use]
    pub fn new(selected: Option<Id>) -> Self {
        Self {
            query: {
                let mut query = TextInputState::new("").with_allow_empty(true);
                query.set_editing(true);
                query
            },
            list: ListState::new(selected),
            previous_visible: Vec::new(),
            accepts_input: true,
            searchable: true,
        }
    }

    /// Host input gate (overlay top / scene ownership).
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Whether printable keys edit the query (junie `searchable`; default true).
    ///
    /// When false, `j`/`k` move the list. [`Picker::searchable`] copies this
    /// flag at render so paint and keys stay one value.
    pub const fn set_searchable(&mut self, on: bool) {
        self.searchable = on;
    }

    /// Whether the query field is live.
    #[must_use]
    pub const fn searchable(&self) -> bool {
        self.searchable
    }

    /// Returns the query used by the caller-owned projection.
    #[must_use]
    pub fn query_text(&self) -> &str {
        self.query.value()
    }

    /// Returns the text-input state for cursor and validation inspection.
    #[must_use]
    pub const fn query(&self) -> &TextInputState {
        &self.query
    }

    /// Returns mutable query state for consumer-specific constraints.
    pub const fn query_mut(&mut self) -> &mut TextInputState {
        &mut self.query
    }

    /// Returns the list state for selection and painted-geometry inspection.
    #[must_use]
    pub const fn list(&self) -> &ListState<Id> {
        &self.list
    }
}

impl<Id: Clone + PartialEq> Default for PickerState<Id> {
    fn default() -> Self {
        Self::new(None)
    }
}

impl<Id: Clone + PartialEq> PickerState<Id> {
    /// Keeps a surviving stable identity selected, otherwise falls back to the
    /// same selectable index clamped into the new projection.
    pub fn reconcile(&mut self, visible: &[ListRow<'_, Id>]) {
        let selectable_count = visible
            .iter()
            .filter(|row| row.enabled && row.role == RowRole::Item)
            .count();
        let unchanged = self.previous_visible.len() == selectable_count
            && self.previous_visible.iter().eq(visible
                .iter()
                .filter(|row| row.enabled && row.role == RowRole::Item)
                .map(|row| &row.id));
        let fallback = self
            .list
            .selected()
            .and_then(|selected| self.previous_visible.iter().position(|id| id == selected))
            .unwrap_or(0);
        let selected_survives = self.list.selected().is_some_and(|selected| {
            visible
                .iter()
                .any(|row| row.enabled && row.role == RowRole::Item && &row.id == selected)
        });
        if unchanged && selected_survives {
            return;
        }
        if !unchanged {
            let mut index = 0;
            for row in visible
                .iter()
                .filter(|row| row.enabled && row.role == RowRole::Item)
            {
                if let Some(existing) = self.previous_visible.get_mut(index) {
                    existing.clone_from(&row.id);
                } else {
                    self.previous_visible.push(row.id.clone());
                }
                index += 1;
            }
            self.previous_visible.truncate(index);
        }
        if self.previous_visible.is_empty() {
            self.list.select(None);
            return;
        }
        if selected_survives {
            return;
        }
        let fallback = fallback.min(self.previous_visible.len() - 1);
        self.list
            .select(Some(self.previous_visible[fallback].clone()));
    }

    /// Routes keys like junie `Picker::on_key`.
    ///
    /// Searchable (default): printable including `j`/`k`/Space edit the query;
    /// arrows / Ctrl+n/p/j/k move the list; Tab is [`PickerOutcome::NextScope`];
    /// Alt+Enter is [`PickerOutcome::ActivatedAlt`]. Choice pickers
    /// (`searchable: false`) use `j`/`k` as list motion.
    pub fn handle_key(&mut self, visible: &[ListRow<'_, Id>], key: KeyEvent) -> PickerOutcome<Id> {
        if !self.accepts_input || key.is_release() {
            return PickerOutcome::Ignored;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        // Activation, cancellation, scope handoff, secondary actions, and
        // query clearing are one-shot. Consume repeats before they can reach
        // the picker state machine; text editing and navigation stay held-key
        // repeatable below.
        if !key.is_press()
            && (matches!(
                key.code,
                KeyCode::Esc | KeyCode::Enter | KeyCode::Tab | KeyCode::Delete
            ) || (ctrl && self.searchable && matches!(key.code, KeyCode::Char('u'))))
        {
            return PickerOutcome::Ignored;
        }
        match key.code {
            KeyCode::Esc => self.handle_intent(visible, UiIntent::Cancel),
            KeyCode::Enter if alt => self.activate(visible, true),
            KeyCode::Enter => self.activate(visible, false),
            KeyCode::Down if !ctrl && !alt => {
                self.handle_intent(visible, UiIntent::Move(NavigationMove::Next))
            }
            KeyCode::Up if !ctrl && !alt => {
                self.handle_intent(visible, UiIntent::Move(NavigationMove::Previous))
            }
            KeyCode::PageDown => self.handle_intent(visible, UiIntent::Page(PageMove::Forward)),
            KeyCode::PageUp => self.handle_intent(visible, UiIntent::Page(PageMove::Backward)),
            KeyCode::Tab => PickerOutcome::NextScope,
            KeyCode::Delete if !ctrl && !alt => match self.list.selected() {
                Some(id) => PickerOutcome::Secondary(id.clone()),
                None => PickerOutcome::Ignored,
            },
            KeyCode::Backspace if self.searchable && !ctrl && !alt => self.route_query(key),
            KeyCode::Char('n' | 'j') if ctrl => {
                self.handle_intent(visible, UiIntent::Move(NavigationMove::Next))
            }
            KeyCode::Char('p' | 'k') if ctrl => {
                self.handle_intent(visible, UiIntent::Move(NavigationMove::Previous))
            }
            KeyCode::Char('u') if ctrl && self.searchable => {
                if self.query.value().is_empty() {
                    PickerOutcome::Ignored
                } else {
                    self.query.clear();
                    PickerOutcome::QueryChanged
                }
            }
            KeyCode::Char('j' | 'J') if !self.searchable && !ctrl && !alt => {
                self.handle_intent(visible, UiIntent::Move(NavigationMove::Next))
            }
            KeyCode::Char('k' | 'K') if !self.searchable && !ctrl && !alt => {
                self.handle_intent(visible, UiIntent::Move(NavigationMove::Previous))
            }
            KeyCode::Char(_) if self.searchable && !ctrl && !alt => self.route_query(key),
            _ => PickerOutcome::Ignored,
        }
    }

    fn activate(&mut self, visible: &[ListRow<'_, Id>], alt: bool) -> PickerOutcome<Id> {
        match self.list.handle_intent(visible, UiIntent::Activate) {
            Outcome::Activated(id) if alt => PickerOutcome::ActivatedAlt(id),
            Outcome::Activated(id) => PickerOutcome::Activated(id),
            _ => PickerOutcome::Ignored,
        }
    }

    fn route_query(&mut self, key: KeyEvent) -> PickerOutcome<Id> {
        match self.query.handle_key(key) {
            TextInputOutcome::Changed | TextInputOutcome::Cleared => PickerOutcome::QueryChanged,
            TextInputOutcome::Cancelled => PickerOutcome::Cancelled,
            TextInputOutcome::Submitted(_)
            | TextInputOutcome::Ignored
            | TextInputOutcome::ClipboardCopy { .. }
            | TextInputOutcome::ClipboardCut { .. }
            | TextInputOutcome::ClipboardPasteRequest => PickerOutcome::Ignored,
        }
    }

    /// Applies a semantic intent to the results list (query is separate).
    pub fn handle_intent(
        &mut self,
        visible: &[ListRow<'_, Id>],
        intent: crate::interaction::UiIntent,
    ) -> PickerOutcome<Id> {
        if !self.accepts_input {
            return PickerOutcome::Ignored;
        }
        use crate::interaction::UiIntent;
        match intent {
            UiIntent::Cancel | UiIntent::Close => {
                if !self.query.value().is_empty() {
                    self.query.clear();
                    PickerOutcome::QueryChanged
                } else {
                    PickerOutcome::Cancelled
                }
            }
            UiIntent::Activate | UiIntent::Open | UiIntent::Submit => self.activate(visible, false),
            UiIntent::Move(_) | UiIntent::Page(_) | UiIntent::Toggle => {
                match self.list.handle_intent(visible, intent) {
                    Outcome::Changed | Outcome::CheckToggled(_) => PickerOutcome::CursorMoved,
                    Outcome::Activated(id) => PickerOutcome::Activated(id),
                    _ => PickerOutcome::Ignored,
                }
            }
            UiIntent::Expand | UiIntent::Collapse => PickerOutcome::Ignored,
            _ => PickerOutcome::Ignored,
        }
    }

    /// Updates list hover from geometry painted by the latest picker render.
    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        if !self.accepts_input {
            return None;
        }
        self.list.hover(position)
    }

    /// Activates a list row from geometry painted by the latest picker render.
    pub fn click(&mut self, position: Position) -> PickerOutcome<Id> {
        if !self.accepts_input {
            return PickerOutcome::Ignored;
        }
        match self.list.click(position) {
            Outcome::Activated(id) => PickerOutcome::Activated(id),
            _ => PickerOutcome::Ignored,
        }
    }

    /// Scrolls the result list and clamps it to the supplied projection length.
    pub fn scroll_by(&mut self, delta: isize, visible_len: usize) -> bool {
        if !self.accepts_input {
            return false;
        }
        self.list.scroll_by(delta, visible_len)
    }
}

#[derive(Debug, Clone, Copy)]
/// Strongly defaulted query-plus-list composition over caller-filtered rows.
///
/// The first row is a [`TextInput`]; remaining rows render a [`List`] or the
/// product-neutral empty cue. Picker owns no overlay, matching, or async policy.
pub struct Picker<'a, Id> {
    rows: &'a [ListRow<'a, Id>],
    system: &'a DesignSystem,
    label: &'a str,
    placeholder: &'a str,
    empty_message: &'a str,
    focused: bool,
    colorless: bool,
    title: &'a str,
    scope: Option<&'a str>,
    searchable: bool,
    hints: Option<&'a str>,
}

impl<'a, Id> Picker<'a, Id> {
    /// Creates a picker with `Filter`, `Type to filter`, and `No matches` defaults.
    #[must_use]
    pub const fn new(rows: &'a [ListRow<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            rows,
            system,
            label: "Filter",
            placeholder: "Type to filter",
            empty_message: "No matches",
            focused: true,
            // Seeded from the system: a widget that defaults to false is
            // claiming the terminal has Unicode and colour before anyone
            // asked it. Builders below still force either way.
            colorless: system.mono(),
            title: "Picker",
            scope: None,
            searchable: true,
            hints: None,
        }
    }

    /// Overlay title painted on the first inner row (not on the border).
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    /// Right-aligned scope label on the title row (`All · Tab scope`).
    #[must_use]
    pub const fn scope(mut self, scope: &'a str) -> Self {
        self.scope = Some(scope);
        self
    }

    /// Hide the query field (fixed-choice pickers: levels, enums).
    #[must_use]
    pub const fn searchable(mut self, on: bool) -> Self {
        self.searchable = on;
        self
    }

    /// Footer hint row. Default matches junie quick-open / choice copy.
    #[must_use]
    pub const fn hints(mut self, hints: &'a str) -> Self {
        self.hints = Some(hints);
        self
    }

    /// Replaces the empty-query placeholder.
    #[must_use]
    pub const fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// Scene surface focus chrome (list selection still uses list cursor).
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &Picker<'_, Id> {
    type State = PickerState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.searchable = self.searchable;
        state.reconcile(self.rows);
        if area.is_empty() {
            StatefulWidget::render(
                &List::new(self.rows, self.system),
                area,
                buffer,
                &mut state.list,
            );
            return;
        }
        let chrome = area.height >= 6 && area.width >= 16;
        if !chrome {
            let query_area = Rect::new(area.x, area.y, area.width, 1.min(area.height));
            if query_area.height > 0 {
                StatefulWidget::render(
                    &TextInput::new(self.label, self.system).placeholder(self.placeholder),
                    query_area,
                    buffer,
                    &mut state.query,
                );
            }
            if area.height < 2 {
                return;
            }
            let list_area = Rect::new(
                area.x,
                area.y.saturating_add(1),
                area.width,
                area.height.saturating_sub(1),
            );
            if self.rows.is_empty() {
                let mark = "∅ ";
                let msg = if list_area.width < 12 {
                    format!("{mark}empty")
                } else {
                    format!("{mark}{}", self.empty_message)
                };
                buffer.set_stringn(
                    list_area.x,
                    list_area.y,
                    take_display_cols(&msg, usize::from(list_area.width)).as_ref(),
                    usize::from(list_area.width),
                    self.system.style(Role::TextMuted),
                );
                StatefulWidget::render(
                    &List::new(&[], self.system),
                    list_area,
                    buffer,
                    &mut state.list,
                );
            } else {
                StatefulWidget::render(
                    &List::new(self.rows, self.system),
                    list_area,
                    buffer,
                    &mut state.list,
                );
            }
            return;
        }

        let theme = self.system.junie_theme();
        let bg = theme.surface_elevated;
        // junie `fill` only sets bg, so inset/gap cells keep dimmed-page fg.
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let fg = buffer[(x, y)].fg;
                buffer[(x, y)]
                    .set_char(' ')
                    .set_style(Style::new().fg(fg).bg(bg));
            }
        }
        Block::default()
            .borders(Borders::ALL)
            .border_set(self.system.border_set())
            .border_style(theme.border(self.focused).bg(bg))
            .render(area, buffer);

        // junie picker: Margin::new(2, 1)
        let inner = area.inner(Margin::new(2, 1));
        if inner.is_empty() {
            return;
        }

        let mut y = inner.y;
        buffer.set_stringn(
            inner.x,
            y,
            take_display_cols(self.title, usize::from(inner.width)).as_ref(),
            usize::from(inner.width),
            theme.title().bg(bg),
        );
        if let Some(scope) = self.scope {
            let sw = display_cols(scope) as u16;
            if sw > 0 && sw <= inner.width {
                buffer.set_stringn(
                    inner.right().saturating_sub(sw),
                    y,
                    scope,
                    usize::from(sw),
                    theme.muted().bg(bg),
                );
            }
        }
        y = y.saturating_add(1);

        if self.searchable && y < inner.bottom() {
            let field = Rect::new(inner.x, y, inner.width, 1);
            let visual = VisualState {
                focused: true,
                editing: true,
                ..VisualState::default()
            };
            let fs = theme.field_style(visual);
            buffer.set_style(field, fs);
            buffer.set_stringn(
                field.x,
                y,
                self.system.glyphs.selection_gutter(),
                1,
                Style::new()
                    .fg(theme.focus)
                    .bg(fs.bg.unwrap_or(bg))
                    .remove_modifier(Modifier::BOLD),
            );
            let query = state.query.value();
            let text_x = field.x.saturating_add(2);
            let text_w = field.width.saturating_sub(3);
            if text_w > 0 {
                if query.is_empty() {
                    buffer.set_stringn(
                        text_x,
                        y,
                        take_display_cols(self.placeholder, usize::from(text_w)).as_ref(),
                        usize::from(text_w),
                        theme.placeholder(visual),
                    );
                } else {
                    buffer.set_stringn(
                        text_x,
                        y,
                        take_display_cols(query, usize::from(text_w)).as_ref(),
                        usize::from(text_w),
                        fs.add_modifier(Modifier::UNDERLINED)
                            .underline_color(theme.accent),
                    );
                }
            }
            let _ = self.label;
            y = y.saturating_add(2);
        }

        let show_hints = inner.height >= 4;
        let list_bottom = if show_hints {
            inner.bottom().saturating_sub(1)
        } else {
            inner.bottom()
        };
        let list_area = Rect::new(inner.x, y, inner.width, list_bottom.saturating_sub(y));
        if list_area.is_empty() {
            return;
        }

        let viewport = usize::from(list_area.height);
        let overflow = self.rows.len() > viewport && list_area.width > 1;
        let row_w = list_area.width.saturating_sub(u16::from(overflow));

        // Paint list first so hit regions exist; junie row anatomy overwrites cells.
        StatefulWidget::render(
            &List::new(self.rows, self.system).focused(false),
            list_area,
            buffer,
            &mut state.list,
        );
        let offset = state.list().paint_skip();
        for y in list_area.top()..list_area.bottom() {
            for x in list_area.left()..list_area.right() {
                let fg = buffer[(x, y)].fg;
                buffer[(x, y)]
                    .set_char(' ')
                    .set_style(Style::new().fg(fg).bg(bg));
            }
        }

        if self.rows.is_empty() {
            let msg = if list_area.width < 12 {
                "No matches".to_string()
            } else {
                self.empty_message.to_string()
            };
            buffer.set_stringn(
                list_area.x,
                list_area.y,
                take_display_cols(&msg, usize::from(list_area.width)).as_ref(),
                usize::from(list_area.width),
                theme.muted().bg(bg),
            );
        } else {
            let line_plain = |line: &ratatui_core::text::Line<'_>| -> String {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            };
            // junie picker.rs: column widths from every item, not the viewport,
            // so scrolling never shifts columns. Label · detail · tag · group.
            let label_col = self
                .rows
                .iter()
                .map(|row| display_cols(&row.plain_label()) as u16)
                .max()
                .unwrap_or(6)
                .clamp(6, (row_w * 45 / 100).max(6));
            let tag_col = self
                .rows
                .iter()
                .filter_map(|row| row.status.as_ref())
                .map(|line| line.width() as u16)
                .max()
                .unwrap_or(0);
            let group_col = self
                .rows
                .iter()
                .map(|row| {
                    display_cols(&row.badge.as_ref().map(line_plain).unwrap_or_default()) as u16
                })
                .max()
                .unwrap_or(0);
            let mut last_group = String::new();
            let visible = self.rows.iter().skip(offset).take(viewport).enumerate();
            for (k, row) in visible {
                let ry = list_area.y.saturating_add(k as u16);
                if ry >= list_area.bottom() {
                    break;
                }
                let focused = state.list().selected() == Some(&row.id);
                let visual = VisualState {
                    focused: focused && row.enabled,
                    disabled: !row.enabled,
                    ..VisualState::default()
                };
                let st = self.system.row(visual, bg);
                let row_rect = Rect::new(list_area.x, ry, row_w, 1);
                buffer.set_style(row_rect, st);
                let mut gutter = self.system.gutter(visual, st.bg.unwrap_or(bg), false);
                if visual.focused {
                    // junie Cell::set_style merges the row's BOLD onto ▎.
                    gutter = gutter.add_modifier(Modifier::BOLD);
                }
                buffer.set_stringn(
                    row_rect.x,
                    ry,
                    self.system.glyphs.selection_gutter(),
                    1,
                    gutter,
                );
                if row_rect.width > 1 {
                    let glyph = row
                        .leading
                        .as_ref()
                        .and_then(|line| line.spans.first())
                        .map(|span| span.content.as_ref())
                        .unwrap_or(" ");
                    let glyph_style = st
                        .fg(if visual.focused {
                            theme.text_primary
                        } else {
                            theme.text_muted
                        })
                        .remove_modifier(Modifier::BOLD);
                    buffer.set_stringn(row_rect.x.saturating_add(1), ry, glyph, 1, glyph_style);
                }
                let group = row.badge.as_ref().map(line_plain).unwrap_or_default();
                let show_group = !group.is_empty() && group != last_group;
                last_group.clone_from(&group);
                let detail = row.secondary.as_ref().map(line_plain).unwrap_or_default();
                let tag = row.status.as_ref().map(line_plain).unwrap_or_default();
                let tag_w = display_cols(&tag);
                let label_plain = row.plain_label();
                let label = truncate_cols(&label_plain, usize::from(label_col), "…");
                let matched = super::fuzzy_match_label(state.query.value(), &label_plain)
                    .map(|(_, ranges)| ranges)
                    .unwrap_or_default();
                let mut x = row_rect.x.saturating_add(3);
                let mut byte = 0usize;
                for ch in label.chars() {
                    if x >= row_rect.right() {
                        break;
                    }
                    let mut cs = st;
                    let hit = matched
                        .as_slice()
                        .iter()
                        .any(|range| byte >= range.start && byte < range.end);
                    if hit {
                        cs = cs.add_modifier(Modifier::BOLD);
                    } else if !visual.focused {
                        cs = cs.remove_modifier(Modifier::BOLD);
                    }
                    let g = ch.to_string();
                    let gw = display_cols(&g) as u16;
                    buffer.set_stringn(x, ry, &g, usize::from(gw.max(1)), cs);
                    x = x.saturating_add(gw.max(1));
                    byte = byte.saturating_add(ch.len_utf8());
                }
                let mut rx = row_rect.right();
                if group_col > 0 {
                    rx = rx.saturating_sub(group_col.saturating_add(1));
                    if show_group && rx < row_rect.right() {
                        buffer.set_stringn(
                            rx,
                            ry,
                            &group,
                            display_cols(&group),
                            st.fg(theme.text_faint).remove_modifier(Modifier::BOLD),
                        );
                    }
                }
                if tag_col > 0 {
                    rx = rx.saturating_sub(tag_col.saturating_add(2));
                    if tag_w > 0 && rx < row_rect.right() {
                        buffer.set_stringn(
                            rx,
                            ry,
                            &tag,
                            tag_w,
                            st.fg(theme.text_secondary).remove_modifier(Modifier::BOLD),
                        );
                    }
                }
                if !detail.is_empty() {
                    let dx = row_rect
                        .x
                        .saturating_add(3)
                        .saturating_add(label_col)
                        .saturating_add(2);
                    let room = rx.saturating_sub(dx.saturating_add(1));
                    if room >= 4 && dx < row_rect.right() {
                        let shown = truncate_cols(&detail, usize::from(room), "…");
                        buffer.set_stringn(
                            dx,
                            ry,
                            shown.as_ref(),
                            display_cols(shown.as_ref()),
                            st.fg(theme.text_muted).remove_modifier(Modifier::BOLD),
                        );
                    }
                }
            }
        }

        if overflow {
            crate::scroll::paint_overflow_scrollbar(
                buffer,
                Rect {
                    x: list_area.right().saturating_sub(1),
                    y: list_area.y,
                    width: 1,
                    height: list_area.height,
                },
                self.rows.len(),
                viewport,
                u16::try_from(offset).unwrap_or(u16::MAX),
                self.focused,
                self.system,
            );
        }

        if show_hints {
            let hints = self.hints.unwrap_or(if self.searchable {
                picker_search_hints()
            } else {
                PICKER_CHOICE_HINTS
            });
            buffer.set_stringn(
                inner.x,
                inner.bottom().saturating_sub(1),
                truncate_cols(hints, usize::from(inner.width), "…").as_ref(),
                usize::from(inner.width),
                theme.faint().bg(bg),
            );
        }

        let _ = self.colorless;
    }
}

fn picker_search_hints() -> &'static str {
    // junie pickers.rs / tablepro paint this spelled form, not Emacs `A-↵`.
    "↑↓ Move · Enter Open · Alt+Enter New tab · Tab Scope · Esc Clear / Close"
}

const PICKER_CHOICE_HINTS: &str = "↑↓ Move · Enter Set level · Esc Keep";

impl<Id: Clone + PartialEq> StatefulWidget for Picker<'_, Id> {
    type State = PickerState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::text::Line;

    use super::*;
    use crate::input::{KeyEventKind, KeyModifiers};

    fn rows(ids: &[&'static str]) -> Vec<ListRow<'static, &'static str>> {
        ids.iter()
            .map(|id| ListRow {
                id: *id,
                label: Line::from(*id),
                leading: None,
                secondary: None,
                status: None,
                badge: None,
                shortcut: None,
                actions: None,
                custom: None,
                role: RowRole::Item,
                enabled: true,
                loading: false,
            })
            .collect()
    }

    #[test]
    fn modal_sits_in_the_upper_third() {
        let bounds = Rect::new(0, 0, 80, 24);
        let placed = place_picker_modal(bounds, PickerSize::default());
        assert!(placed.y < bounds.height / 2, "upper third: {placed:?}");
        assert!(placed.width >= 24);
    }

    #[test]
    fn reconciliation_is_id_sticky_then_index_fallback() {
        let cases = [
            (Some("beta"), &["alpha", "beta", "gamma"][..], Some("beta")),
            (Some("beta"), &["beta", "gamma"][..], Some("beta")),
            (Some("beta"), &["alpha", "gamma"][..], Some("gamma")),
            (Some("gamma"), &["alpha"][..], Some("alpha")),
            (Some("alpha"), &[][..], None),
        ];
        for (selected, filtered, expected) in cases {
            let mut state = PickerState::new(selected);
            state.reconcile(&rows(&["alpha", "beta", "gamma"]));
            state.reconcile(&rows(filtered));
            assert_eq!(state.list().selected().copied(), expected);
        }

        let mut reordered = PickerState::new(Some("gamma"));
        reordered.reconcile(&rows(&["alpha", "beta", "gamma"]));
        reordered.reconcile(&rows(&["gamma", "alpha"]));
        assert_eq!(reordered.list().selected(), Some(&"gamma"));
    }

    #[test]
    fn disabled_and_separator_rows_never_become_fallbacks() {
        let mut visible = rows(&["enabled"]);
        visible.insert(
            0,
            ListRow {
                id: "separator",
                label: Line::from("Group"),
                leading: None,
                secondary: None,
                status: None,
                badge: None,
                shortcut: None,
                actions: None,
                custom: None,
                role: RowRole::Separator,
                enabled: true,
                loading: false,
            },
        );
        visible.push(ListRow {
            id: "disabled",
            label: Line::from("Disabled"),
            leading: None,
            secondary: None,
            status: None,
            badge: None,
            shortcut: None,
            actions: None,
            custom: None,
            role: RowRole::Item,
            enabled: false,
            loading: false,
        });
        let mut state = PickerState::new(Some("missing"));
        state.reconcile(&visible);
        assert_eq!(state.list().selected(), Some(&"enabled"));
    }

    #[test]
    fn unicode_query_navigation_activation_and_two_stage_escape_are_disjoint() {
        let visible = rows(&["東京", "🧪"]);
        let mut state = PickerState::new(Some("東京"));
        assert_eq!(
            state.handle_key(
                &visible,
                KeyEvent::new(KeyCode::Char('東'), KeyModifiers::NONE)
            ),
            PickerOutcome::QueryChanged
        );
        assert_eq!(state.query_text(), "東");
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            PickerOutcome::CursorMoved
        );
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PickerOutcome::Activated("🧪")
        );
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            PickerOutcome::QueryChanged
        );
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            PickerOutcome::Cancelled
        );
    }

    #[test]
    fn release_and_modified_navigation_are_ignored() {
        let visible = rows(&["alpha", "beta"]);
        let mut state = PickerState::new(Some("alpha"));
        let mut release = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert_eq!(state.handle_key(&visible, release), PickerOutcome::Ignored);
        assert_eq!(
            state.handle_key(
                &visible,
                KeyEvent::new(KeyCode::Down, KeyModifiers::CONTROL)
            ),
            PickerOutcome::Ignored
        );
        assert_eq!(state.list().selected(), Some(&"alpha"));
    }

    #[test]
    fn repeated_one_shot_actions_are_ignored() {
        let visible = rows(&["alpha", "beta"]);
        let mut state = PickerState::new(Some("alpha"));
        assert_eq!(
            state.handle_key(
                &visible,
                KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)
            ),
            PickerOutcome::QueryChanged
        );

        for (code, modifiers) in [
            (KeyCode::Esc, KeyModifiers::NONE),
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Enter, KeyModifiers::ALT),
            (KeyCode::Tab, KeyModifiers::NONE),
            (KeyCode::Delete, KeyModifiers::NONE),
            (KeyCode::Char('u'), KeyModifiers::CONTROL),
        ] {
            let mut repeat = KeyEvent::new(code, modifiers);
            repeat.kind = KeyEventKind::Repeat;
            assert_eq!(
                state.handle_key(&visible, repeat),
                PickerOutcome::Ignored,
                "repeat of {code:?} with {modifiers:?} must not fire a one-shot action"
            );
        }

        assert_eq!(state.query_text(), "x");
        assert_eq!(state.list().selected(), Some(&"alpha"));
    }

    #[test]
    fn accepts_input_gate() {
        let visible = rows(&["alpha"]);
        let mut state = PickerState::new(Some("alpha"));
        state.set_accepts_input(false);
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PickerOutcome::Ignored
        );
    }

    #[test]
    fn cursor_moved_not_selection_changed_surface() {
        let src = include_str!("picker.rs");
        let head = src
            .split("#[cfg(test)]")
            .next()
            .unwrap_or(src)
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                !t.starts_with("//") && !t.starts_with("//!")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!head.contains("SelectionChanged"));
        assert!(head.contains("CursorMoved"));
    }

    #[test]
    fn empty_and_tiny_rendering_are_safe_and_clear_pointer_geometry() {
        let tokens = DesignSystem::default();
        let _theme = tokens.palette.clone();
        let visible = rows(&["alpha"]);
        let mut state = PickerState::new(Some("alpha"));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 2));
        (&Picker::new(&visible, &tokens)).render(Rect::new(0, 0, 8, 2), &mut buffer, &mut state);
        assert_eq!(
            state.click(Position::new(2, 1)),
            PickerOutcome::Activated("alpha")
        );
        (&Picker::new(&[], &tokens)).render(Rect::new(0, 0, 8, 2), &mut buffer, &mut state);
        let empty_cell = buffer[(0, 1)].symbol();
        assert!(
            empty_cell == "∅" || empty_cell == "[" || empty_cell == "N",
            "empty cue: {empty_cell:?}"
        );
        (&Picker::new(&[], &tokens)).render(Rect::new(0, 0, 0, 0), &mut buffer, &mut state);
        assert_eq!(state.click(Position::new(2, 1)), PickerOutcome::Ignored);
    }

    #[test]
    fn mouse_activation_delegates_to_painted_list_geometry() {
        let tokens = DesignSystem::default();
        let visible = rows(&["alpha"]);
        let mut state = PickerState::new(Some("alpha"));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 3));
        (&Picker::new(&visible, &tokens)).render(Rect::new(0, 0, 20, 3), &mut buffer, &mut state);
        assert_eq!(
            state.click(Position::new(2, 1)),
            PickerOutcome::Activated("alpha")
        );
    }

    #[test]
    fn warmed_reconciliation_reuses_projection_capacity() {
        let visible = rows(&["alpha", "beta", "gamma"]);
        let mut state = PickerState::new(Some("alpha"));
        state.reconcile(&visible);
        let capacity = state.previous_visible.capacity();
        for _ in 0..100 {
            state.reconcile(&visible);
        }
        assert_eq!(state.previous_visible.capacity(), capacity);
    }

    #[test]
    fn rendering_a_filtered_projection_clears_stale_hover() {
        let tokens = DesignSystem::default();
        let initial = rows(&["alpha", "beta"]);
        let reordered = rows(&["beta", "alpha"]);
        let filtered = rows(&["alpha"]);
        let mut state = PickerState::new(Some("alpha"));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 4));
        (&Picker::new(&initial, &tokens)).render(Rect::new(0, 0, 20, 4), &mut buffer, &mut state);
        assert_eq!(state.hover(Position::new(2, 2)), Some(&"beta"));
        (&Picker::new(&reordered, &tokens)).render(Rect::new(0, 0, 20, 4), &mut buffer, &mut state);
        assert_eq!(state.list().hovered(), Some(&"alpha"));
        (&Picker::new(&filtered, &tokens)).render(Rect::new(0, 0, 20, 4), &mut buffer, &mut state);
        assert_eq!(state.list().hovered(), None);
    }

    #[test]
    fn junie_modal_is_bar_glyph_label_not_chevron() {
        let system = DesignSystem::junie();
        let rows = [
            ListRow::item("cargo", Line::from("Cargo.toml"))
                .leading(Line::from("F"))
                .secondary(Line::from("Cargo.toml"))
                .badge(Line::from("Files")),
            ListRow::item("readme", Line::from("README.md"))
                .leading(Line::from("F"))
                .secondary(Line::from("README.md")),
        ];
        let mut state = PickerState::new(Some("cargo"));
        let area = Rect::new(0, 0, 64, 12);
        let mut buffer = Buffer::empty(area);
        Picker::new(&rows, &system)
            .title("Open quickly")
            .placeholder("Files and tasks…")
            .scope("All · Tab scope")
            .render(area, &mut buffer, &mut state);
        let row = |y: u16| -> String {
            (0..area.width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect()
        };
        let title = row(1);
        assert!(title.contains("Open quickly"), "{title:?}");
        assert!(title.contains("All · Tab scope"), "{title:?}");
        let query = row(2);
        assert!(query.contains("Files and tasks"), "{query:?}");
        let first = row(4);
        assert!(first.contains("Cargo.toml"), "{first:?}");
        assert_eq!(buffer[(2, 4)].symbol(), "▎", "gutter: {first:?}");
        assert_eq!(buffer[(3, 4)].symbol(), "F", "glyph: {first:?}");
        assert_ne!(buffer[(3, 4)].symbol(), "›");
        let second = row(5);
        assert!(!second.contains('›'), "no chosen marker: {second:?}");
    }

    #[test]
    fn grouped_rows_keep_junie_fixed_detail_column() {
        let system = DesignSystem::junie();
        let rows = [
            ListRow::item("cargo", Line::from("Cargo.toml"))
                .leading(Line::from("F"))
                .secondary(Line::from("Cargo.toml"))
                .badge(Line::from("Files")),
            ListRow::item("arch", Line::from("architecture.md"))
                .leading(Line::from("F"))
                .secondary(Line::from("docs/architecture.md"))
                .badge(Line::from("Files")),
        ];
        let mut state = PickerState::new(Some("cargo"));
        let area = Rect::new(0, 0, 80, 12);
        let mut buffer = Buffer::empty(area);
        Picker::new(&rows, &system)
            .title("Open quickly")
            .placeholder("Files and tasks…")
            .scope("All · Tab scope")
            .render(area, &mut buffer, &mut state);
        let row = |y: u16| -> String {
            (0..area.width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect()
        };
        let first = row(4);
        let second = row(5);
        let cargo_label = first.find("Cargo.toml").expect("label");
        let cargo_path = first[cargo_label + 10..]
            .find("Cargo.toml")
            .map(|i| cargo_label + 10 + i)
            .expect(&first);
        let arch_path = second.find("docs/architecture.md").expect(&second);
        assert_eq!(
            cargo_path, arch_path,
            "junie fixed detail column, not right-aligned paths:\n{first}\n{second}"
        );
    }

    #[test]
    fn modal_picker_sits_in_the_upper_third() {
        let bounds = Rect::new(0, 0, 120, 40);
        let placed = place_picker_modal(bounds, PickerSize::default());
        assert!(placed.y < bounds.height / 2, "upper third, not true center");
        assert_eq!(
            placed.x,
            bounds.width.saturating_sub(placed.width) / 2,
            "horizontally centered"
        );
    }

    #[test]
    fn search_footer_paints_junie_alt_enter_not_emacs_chord() {
        let system = DesignSystem::junie();
        let visible = rows(&["alpha"]);
        let mut state = PickerState::new(Some("alpha"));
        let area = Rect::new(0, 0, 80, 12);
        let mut buffer = Buffer::empty(area);
        Picker::new(&visible, &system)
            .title("Open quickly")
            .render(area, &mut buffer, &mut state);
        let mut text = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                text.push_str(buffer[(x, y)].symbol());
            }
        }
        assert!(
            text.contains("Alt+Enter"),
            "junie picker footer spells Alt+Enter: {text:?}"
        );
        assert!(
            !text.contains("A-↵"),
            "Emacs footer chord is not the junie paint: {text:?}"
        );
    }

    #[test]
    fn searchable_picker_j_and_space_edit_query_not_list() {
        let visible = rows(&["alpha", "beta"]);
        let mut state = PickerState::new(Some("alpha"));
        assert_eq!(
            state.handle_key(
                &visible,
                KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)
            ),
            PickerOutcome::QueryChanged
        );
        assert_eq!(state.query_text(), "j");
        assert_eq!(state.list().selected(), Some(&"alpha"));
        assert_eq!(
            state.handle_key(
                &visible,
                KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)
            ),
            PickerOutcome::QueryChanged
        );
        assert_eq!(state.query_text(), "j ");
        assert_eq!(state.list().selected(), Some(&"alpha"));
    }

    #[test]
    fn picker_tab_is_next_scope_and_ctrl_n_steps_results() {
        let visible = rows(&["alpha", "beta"]);
        let mut state = PickerState::new(Some("alpha"));
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            PickerOutcome::NextScope
        );
        assert_eq!(
            state.handle_key(
                &visible,
                KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL)
            ),
            PickerOutcome::CursorMoved
        );
        assert_eq!(state.list().selected(), Some(&"beta"));
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT)),
            PickerOutcome::ActivatedAlt("beta")
        );
    }

    #[test]
    fn delete_is_junie_secondary() {
        let visible = rows(&["alpha", "beta"]);
        let mut state = PickerState::new(Some("alpha"));
        assert_eq!(
            state.handle_key(&visible, KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE)),
            PickerOutcome::Secondary("alpha")
        );
        assert_eq!(state.list().selected(), Some(&"alpha"));
    }

    #[test]
    fn choice_picker_j_moves_list() {
        let visible = rows(&["alpha", "beta"]);
        let mut state = PickerState::new(Some("alpha"));
        state.set_searchable(false);
        assert_eq!(
            state.handle_key(
                &visible,
                KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)
            ),
            PickerOutcome::CursorMoved
        );
        assert_eq!(state.list().selected(), Some(&"beta"));
        assert_eq!(state.query_text(), "");
    }

    #[test]
    fn chrome_picker_keeps_list_anatomy_and_hint_row() {
        let tokens = DesignSystem::default();
        let visible = rows(&["alpha", "beta"]);
        let mut state = PickerState::new(Some("alpha"));
        let area = Rect::new(0, 0, 40, 12);
        let mut buffer = Buffer::empty(area);
        (&Picker::new(&visible, &tokens).title("Open")).render(area, &mut buffer, &mut state);
        let mut text = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                text.push_str(buffer[(x, y)].symbol());
            }
        }
        assert!(text.contains("alpha"));
        assert!(
            text.contains("Move")
                || text.contains("Enter")
                || text.contains("Open")
                || text.contains("move")
                || text.contains("enter")
                || text.contains("choose"),
            "own hint row: {text:?}"
        );
        let selected = state
            .list()
            .regions()
            .iter()
            .find(|r| r.id == "alpha")
            .expect("selected row painted");
        assert_eq!(
            buffer[(selected.area.x, selected.area.y)].symbol(),
            tokens.glyphs.selection_gutter()
        );
    }

    #[test]
    fn overflowing_picker_uses_overflow_thumb() {
        let system = DesignSystem::default();
        let ids = [
            "r00", "r01", "r02", "r03", "r04", "r05", "r06", "r07", "r08", "r09", "r10", "r11",
            "r12", "r13", "r14", "r15", "r16", "r17", "r18", "r19", "r20", "r21", "r22", "r23",
        ];
        let visible = rows(&ids);
        let mut state = PickerState::new(Some("r00"));
        let area = Rect::new(0, 0, 28, 14);
        let mut buffer = Buffer::empty(area);
        Picker::new(&visible, &system).render(area, &mut buffer, &mut state);
        let thumb = crate::scroll::ScrollbarStyle::Line.vertical_thumb();
        let track = crate::scroll::SCROLLBAR_TRACK;
        // Box borders also use `│`; the overflow gutter is inside the frame.
        let mut sb_x = None;
        for y in 1..area.height.saturating_sub(1) {
            for x in 1..area.width.saturating_sub(1) {
                if buffer[(x, y)].symbol() == thumb {
                    sb_x = Some(x);
                }
            }
        }
        let sb_x = sb_x.expect("overflowing picker paints a thumb");
        let track_ys: Vec<u16> = (1..area.height.saturating_sub(1))
            .filter(|y| {
                let symbol = buffer[(sb_x, *y)].symbol();
                symbol == thumb || symbol == track
            })
            .collect();
        let viewport = track_ys.len();
        let (start, len) = crate::scroll::overflow_thumb(24, viewport, viewport, 0)
            .expect("24 rows overflow the list viewport");
        let thumbs: Vec<u16> = track_ys
            .iter()
            .copied()
            .filter(|y| buffer[(sb_x, *y)].symbol() == thumb)
            .collect();
        assert_eq!(thumbs.len(), len);
        assert_eq!(thumbs[0], track_ys[start]);
        assert_eq!(
            buffer[(sb_x, track_ys[len])].symbol(),
            track,
            "cells after the thumb stay track"
        );
    }

    #[test]
    fn picker_overlay_helpers_open_and_dismiss() {
        use crate::interaction::OverlayKind;
        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(10, 5, 8, 1);
        let mut stack = crate::interaction::OverlayStack::<&'static str>::new();
        let out = open_picker_overlay(
            &mut stack,
            bounds,
            anchor,
            PickerSize::default(),
            Some("trigger"),
        );
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Select);
        let placed = place_picker(bounds, anchor, PickerSize::default());
        assert_eq!(stack.top().unwrap().rect, placed);
        assert!(matches!(
            dismiss_picker_overlay(&mut stack),
            OverlayOutcome::Dismissed {
                focus: Some("trigger"),
                ..
            }
        ));
    }
}

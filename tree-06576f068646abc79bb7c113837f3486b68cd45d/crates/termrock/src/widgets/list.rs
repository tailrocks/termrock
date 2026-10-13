use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    text::Line,
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyEvent, KeyEventKind},
    interaction::{HitRegion, NavigationMove, Outcome, PageMove, UiIntent, default_list_intent},
    scroll::max_offset,
    style::{DesignTokens, Role},
};

use super::{ComposedRow, Selection};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Semantic roles for selectable, disabled, and separator list rows.
pub enum RowRole {
    /// A selectable content row.
    Item,
    /// A non-interactive visual separator row.
    Separator,
}

#[derive(Debug, Clone)]
/// A stable row in a selectable list with composed-part anatomy.
///
/// Parts map to [`ComposedRow`]: leading · primary(label) · secondary · badge ·
/// shortcut · trailing. Narrow terminals drop by
/// shortcut → badge → secondary → trailing → leading → primary.
pub struct ListRow<'a, Id> {
    /// Stable identity used for selection and activation.
    pub id: Id,
    /// Primary label (never dropped first under contraction).
    pub label: Line<'a>,
    /// Optional leading icon / check chrome (composed leading).
    pub leading: Option<Line<'a>>,
    /// Optional secondary metadata line (composed secondary).
    pub secondary: Option<Line<'a>>,
    /// Optional badge (composed badge).
    pub badge: Option<Line<'a>>,
    /// Optional keyboard shortcut hint (composed shortcut).
    pub shortcut: Option<&'a str>,
    /// Optional metadata aligned at the trailing edge (legacy + composed).
    pub trailing: Option<Line<'a>>,
    /// Interaction role controlling selection and hit testing.
    pub role: RowRole,
    /// Whether this item is enabled.
    pub enabled: bool,
    /// Loading placeholder (composed loading).
    pub loading: bool,
}

impl<'a, Id> ListRow<'a, Id> {
    /// Creates a primary-only item row.
    #[must_use]
    pub fn item(id: Id, label: Line<'a>) -> Self {
        Self {
            id,
            label,
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            trailing: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        }
    }

    /// Projects this row into composed anatomy for contraction/paint.
    #[must_use]
    pub fn composed(&self) -> ComposedRow<'a, ()>
    where
        Id: Clone,
    {
        ComposedRow {
            id: (),
            leading: self.leading.clone(),
            primary: self.label.clone(),
            secondary: self.secondary.clone(),
            badge: self.badge.clone().or_else(|| self.trailing.clone()),
            shortcut: self.shortcut,
            enabled: self.enabled,
            loading: self.loading,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Runtime state for `List`.
pub struct ListState<Id> {
    selected: Option<Id>,
    hovered: Option<Id>,
    focused: bool,
    offset: usize,
    viewport_height: usize,
    regions: Vec<HitRegion<Id>>,
    pointer: Option<Position>,
    selection: Option<Selection<Id>>,
    check_regions: Vec<HitRegion<Id>>,
}

impl<Id> Default for ListState<Id> {
    fn default() -> Self {
        Self {
            selected: None,
            hovered: None,
            focused: false,
            offset: 0,
            viewport_height: 0,
            regions: Vec::new(),
            pointer: None,
            selection: None,
            check_regions: Vec::new(),
        }
    }
}

impl<Id> ListState<Id> {
    #[must_use]
    /// Creates list state with no selection, hover, checks, or scroll.
    pub const fn new(selected: Option<Id>) -> Self {
        Self {
            selected,
            hovered: None,
            focused: true,
            offset: 0,
            viewport_height: 0,
            regions: Vec::new(),
            pointer: None,
            selection: None,
            check_regions: Vec::new(),
        }
    }

    /// Replace the stable selected identity.
    pub fn select(&mut self, selected: Option<Id>) {
        self.selected = selected;
    }

    #[must_use]
    /// Returns the stable identity selected for keyboard interaction.
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    #[must_use]
    /// Returns the stable identity currently under the pointer.
    pub const fn hovered(&self) -> Option<&Id> {
        self.hovered.as_ref()
    }

    #[must_use]
    /// Returns whether the list owns keyboard focus.
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Updates whether the list owns keyboard focus.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    #[must_use]
    /// Returns the first visible row index.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    #[must_use]
    /// Returns the painted item hit regions from the most recent render.
    pub fn regions(&self) -> &[HitRegion<Id>] {
        &self.regions
    }

    /// Enables ordered multi-selection with an empty selection.
    pub fn enable_multi_select(&mut self) {
        self.selection.get_or_insert_with(Selection::new);
    }

    /// Disables multi-selection and discards checked identities.
    pub fn disable_multi_select(&mut self) {
        self.selection = None;
    }

    #[must_use]
    /// Returns the ordered multi-selection state, if enabled.
    pub const fn selection(&self) -> Option<&Selection<Id>> {
        self.selection.as_ref()
    }

    /// Returns mutable access to ordered multi-selection state, if enabled.
    pub fn selection_mut(&mut self) -> Option<&mut Selection<Id>> {
        self.selection.as_mut()
    }

    /// Moves the scroll position by a signed delta and clamps it to valid content.
    pub fn scroll_by(&mut self, delta: isize, rows_len: usize) -> bool {
        let before = self.offset;
        let max = max_offset(rows_len, self.viewport_height);
        self.offset = if delta.is_negative() {
            self.offset.saturating_sub(delta.unsigned_abs())
        } else {
            self.offset.saturating_add(delta.unsigned_abs()).min(max)
        };
        before != self.offset
    }

    /// Scrolls toward a pointer position within the painted viewport.
    pub fn scroll_to_position(&mut self, position: Position, rows_len: usize) -> bool {
        self.pointer = Some(position);
        if self.viewport_height == 0 || self.regions.is_empty() {
            return false;
        }
        let first = self.regions[0].area;
        if position.y < first.y {
            return self.scroll_by(-1, rows_len);
        }
        let bottom = first.y.saturating_add(
            u16::try_from(self.viewport_height.saturating_sub(1)).unwrap_or(u16::MAX),
        );
        if position.y > bottom {
            return self.scroll_by(1, rows_len);
        }
        false
    }
}

impl<Id: Clone + PartialEq> ListState<Id> {
    /// Routes navigation, checking, activation, and cancellation keys.
    ///
    /// Keys are mapped through [`default_list_intent`]; prefer
    /// [`Self::handle_intent`] when the application owns keymaps.
    pub fn handle_key(&mut self, rows: &[ListRow<'_, Id>], key: KeyEvent) -> Outcome<Id> {
        if key.kind == KeyEventKind::Release {
            return Outcome::Ignored;
        }
        match default_list_intent(key) {
            Some(intent) => self.handle_intent(rows, intent),
            None => Outcome::Ignored,
        }
    }

    /// Applies a semantic intent to this list.
    pub fn handle_intent(&mut self, rows: &[ListRow<'_, Id>], intent: UiIntent) -> Outcome<Id> {
        match intent {
            UiIntent::Move(NavigationMove::Previous) => self.select_relative(rows, -1),
            UiIntent::Move(NavigationMove::Next) => self.select_relative(rows, 1),
            UiIntent::Move(NavigationMove::First) => self.select_edge(rows, false),
            UiIntent::Move(NavigationMove::Last) => self.select_edge(rows, true),
            UiIntent::Page(PageMove::Backward) => self.select_page(rows, -1),
            UiIntent::Page(PageMove::Forward) => self.select_page(rows, 1),
            UiIntent::Activate | UiIntent::Open | UiIntent::Submit => self.activate(rows),
            UiIntent::Toggle => self.toggle_selected(rows),
            UiIntent::Cancel | UiIntent::Close => Outcome::Cancelled,
            UiIntent::Expand | UiIntent::Collapse => Outcome::Ignored,
        }
    }

    fn toggle_selected(&mut self, rows: &[ListRow<'_, Id>]) -> Outcome<Id> {
        let Some(selection) = self.selection.as_mut() else {
            return Outcome::Ignored;
        };
        let Some(row) = self.selected.as_ref().and_then(|selected| {
            rows.iter()
                .find(|row| row.enabled && row.role == RowRole::Item && &row.id == selected)
        }) else {
            return Outcome::Ignored;
        };
        selection.toggle(&row.id);
        Outcome::CheckToggled(row.id.clone())
    }

    /// Moves selection to the next enabled item, wrapping at the end.
    pub fn select_next(&mut self, rows: &[ListRow<'_, Id>]) -> Outcome<Id> {
        self.select_relative(rows, 1)
    }

    /// Moves selection to the previous enabled item, wrapping at the start.
    pub fn select_previous(&mut self, rows: &[ListRow<'_, Id>]) -> Outcome<Id> {
        self.select_relative(rows, -1)
    }

    fn select_relative(&mut self, rows: &[ListRow<'_, Id>], direction: isize) -> Outcome<Id> {
        let selectable = selectable_indices(rows);
        if selectable.is_empty() {
            self.selected = None;
            return Outcome::Ignored;
        }
        let current = self.selected.as_ref().and_then(|selected| {
            selectable
                .iter()
                .position(|index| &rows[*index].id == selected)
        });
        let next = match (current, direction.is_negative()) {
            (Some(0), true) | (None, true) => selectable.len() - 1,
            (Some(index), true) => index - 1,
            (Some(index), false) => (index + 1) % selectable.len(),
            (None, false) => 0,
        };
        self.selected = Some(rows[selectable[next]].id.clone());
        Outcome::Changed
    }

    fn select_edge(&mut self, rows: &[ListRow<'_, Id>], end: bool) -> Outcome<Id> {
        let selectable = selectable_indices(rows);
        let index = if end {
            selectable.last().copied()
        } else {
            selectable.first().copied()
        };
        let Some(index) = index else {
            self.selected = None;
            return Outcome::Ignored;
        };
        self.selected = Some(rows[index].id.clone());
        Outcome::Changed
    }

    fn select_page(&mut self, rows: &[ListRow<'_, Id>], direction: isize) -> Outcome<Id> {
        let selectable = selectable_indices(rows);
        if selectable.is_empty() {
            self.selected = None;
            return Outcome::Ignored;
        }
        let current = self
            .selected
            .as_ref()
            .and_then(|selected| {
                selectable
                    .iter()
                    .position(|index| &rows[*index].id == selected)
            })
            .unwrap_or(0);
        let page = self.viewport_height.max(1);
        let next = if direction.is_negative() {
            current.saturating_sub(page)
        } else {
            current.saturating_add(page).min(selectable.len() - 1)
        };
        self.selected = Some(rows[selectable[next]].id.clone());
        Outcome::Changed
    }

    #[must_use]
    /// Returns the semantic action associated with the supplied stable identity.
    pub fn activate(&self, rows: &[ListRow<'_, Id>]) -> Outcome<Id> {
        self.selected
            .as_ref()
            .and_then(|selected| {
                rows.iter()
                    .find(|row| row.enabled && row.role == RowRole::Item && &row.id == selected)
            })
            .map_or(Outcome::Ignored, |row| Outcome::Activated(row.id.clone()))
    }

    /// Updates hover state from the current pointer position and painted hit regions.
    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        self.pointer = Some(position);
        self.hovered = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    #[must_use]
    /// Maps a pointer position to the semantic outcome of the painted hit region.
    pub fn click(&mut self, position: Position) -> Outcome<Id> {
        self.pointer = Some(position);
        self.hovered = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        if let Some(id) = self
            .check_regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone())
        {
            self.selected = Some(id.clone());
            if let Some(selection) = self.selection.as_mut() {
                selection.toggle(&id);
                return Outcome::CheckToggled(id);
            }
        }
        let Some(region) = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
        else {
            return Outcome::Ignored;
        };
        self.selected = Some(region.id.clone());
        Outcome::Activated(region.id.clone())
    }
}

impl ListState<usize> {
    /// Create index-addressed list state with the first item selected.
    #[must_use]
    pub const fn for_count(count: usize) -> Self {
        Self::new(if count == 0 { None } else { Some(0) })
    }

    /// Reconcile an index selection after the backing collection changes.
    pub fn reconcile_count(&mut self, count: usize) {
        self.selected = match (self.selected, count) {
            (_, 0) => None,
            (Some(index), _) => Some(if index < count { index } else { count - 1 }),
            (None, _) => Some(0),
        };
    }

    /// Move an index selection by one item, wrapping at either edge.
    pub fn cycle_index(&mut self, count: usize, direction: isize) -> bool {
        if count == 0 {
            self.selected = None;
            return false;
        }
        let current = self.selected.unwrap_or(0).min(count - 1);
        let next = if direction.is_negative() {
            if current == 0 { count - 1 } else { current - 1 }
        } else if current + 1 >= count {
            0
        } else {
            current + 1
        };
        self.selected = Some(next);
        next != current
    }

    /// Move an index selection by a gesture delta without wrapping.
    pub fn move_index(&mut self, count: usize, delta: isize) -> bool {
        if count == 0 {
            self.selected = None;
            return false;
        }
        let current = self.selected.unwrap_or(0).min(count - 1);
        let next = if delta.is_negative() {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            current.saturating_add(delta.unsigned_abs()).min(count - 1)
        };
        self.selected = Some(next);
        next != current
    }

    /// Borrow the selected item from an index-addressed collection.
    #[must_use]
    pub fn selected_item<'a, T>(&self, items: &'a [T]) -> Option<&'a T> {
        self.selected.and_then(|index| items.get(index))
    }
}

#[derive(Debug, Clone, Copy)]
/// Stable-ID list widget rendered with [`ListState`].
///
/// See the `list/selection` lookbook story for selection, metadata, and narrow
/// terminal behavior.
///
/// # Examples
///
/// ```
/// use ratatui_core::text::Line;
/// use termrock::{
///     Theme,
///     input::{KeyCode, KeyEvent, KeyModifiers},
///     interaction::Outcome,
///     widgets::{List, ListRow, ListState, RowRole},
/// };
///
/// let rows = [
///     ListRow { id: "a", label: Line::from("Alpha"), leading: None, secondary: None, badge: None, shortcut: None, trailing: None, role: RowRole::Item, enabled: true , loading: false },
///     ListRow { id: "b", label: Line::from("Beta"), leading: None, secondary: None, badge: None, shortcut: None, trailing: None, role: RowRole::Item, enabled: true , loading: false },
/// ];
/// let tokens = termrock::style::DesignTokens::default();
/// let _widget = List::new(&rows, &tokens);
/// let mut state = ListState::new(Some("a"));
/// let outcome = state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
/// assert!(matches!(outcome, Outcome::Changed));
/// assert_eq!(state.selected(), Some(&"b"));
/// ```
pub struct List<'a, Id> {
    rows: &'a [ListRow<'a, Id>],
    tokens: &'a DesignTokens,
}

impl<'a, Id> List<'a, Id> {
    #[must_use]
    /// Creates a list over borrowed rows; paint uses design-token recipes.
    pub const fn new(rows: &'a [ListRow<'a, Id>], tokens: &'a DesignTokens) -> Self {
        Self { rows, tokens }
    }

    /// Theme borrowed from design tokens.
    #[must_use]
    pub const fn theme(&self) -> &crate::style::Theme {
        &self.tokens.theme
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &List<'_, Id> {
    type State = ListState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        state.check_regions.clear();
        state.viewport_height = usize::from(area.height);
        let scrollable = crate::scroll::is_scrollable(self.rows.len(), state.viewport_height);
        let content_width = area.width.saturating_sub(u16::from(scrollable));
        state.offset = state
            .offset
            .min(max_offset(self.rows.len(), state.viewport_height));
        if let Some(selected) = state.selected.as_ref()
            && let Some(index) = self.rows.iter().position(|row| &row.id == selected)
        {
            if index < state.offset {
                state.offset = index;
            } else if index >= state.offset.saturating_add(state.viewport_height) {
                state.offset = index
                    .saturating_add(1)
                    .saturating_sub(state.viewport_height);
            }
        }
        for (visible, row) in self
            .rows
            .iter()
            .skip(state.offset)
            .take(state.viewport_height)
            .enumerate()
        {
            let rect = Rect::new(
                area.x,
                area.y
                    .saturating_add(u16::try_from(visible).unwrap_or(u16::MAX)),
                content_width,
                1,
            );
            let selected = state.selected.as_ref() == Some(&row.id);
            let hovered = row.enabled
                && row.role == RowRole::Item
                && state
                    .pointer
                    .is_some_and(|position| rect.contains(position));
            let checked = state
                .selection
                .as_ref()
                .is_some_and(|selection| selection.is_checked(&row.id));
            let recipe =
                self.tokens
                    .list_row_recipe(selected, state.focused && selected, row.enabled);
            let style = if hovered && row.enabled && !selected {
                self.tokens.theme.style(Role::LinkHover)
            } else if checked && !selected {
                self.tokens.theme.style(Role::Accent)
            } else {
                recipe.label
            };
            if recipe.use_fill {
                buffer.set_style(rect, style);
            }
            if row.role == RowRole::Separator {
                let rule = self.tokens.glyphs.rule();
                buffer.set_stringn(rect.x, rect.y, rule, usize::from(rect.width), style);
                if rect.width > 2 {
                    let label_x = rect.x.saturating_add(2);
                    let parts = row.composed().parts_for_width(rect.width.saturating_sub(2));
                    buffer.set_line(
                        label_x,
                        rect.y,
                        &parts.primary,
                        rect.right().saturating_sub(label_x),
                    );
                }
            } else {
                if let Some((glyph, gstyle)) = recipe.gutter {
                    buffer.set_stringn(rect.x, rect.y, glyph, 1, gstyle);
                    buffer.set_stringn(rect.x.saturating_add(1), rect.y, " ", 1, style);
                } else {
                    buffer.set_stringn(rect.x, rect.y, "  ", 2, style);
                }
                let check_x = rect.x.saturating_add(2);
                render_check_cell(buffer, state, row, rect, check_x, checked, style);
                let content_x = check_x.saturating_add(u16::from(state.selection.is_some()) * 4);
                if content_x < rect.right() {
                    // Zero-copy contraction (ComposedRow grammar without Line clones).
                    let content_w = rect.right().saturating_sub(content_x);
                    let badge = row.badge.as_ref().or(row.trailing.as_ref());
                    let mut budget = content_w.saturating_sub(1);
                    let shortcut_need = row
                        .shortcut
                        .map(|s| {
                            u16::try_from(crate::text::display_cols(s))
                                .unwrap_or(u16::MAX)
                                .saturating_add(1)
                        })
                        .unwrap_or(0);
                    // Shortcuts need room; never steal the last primary cells.
                    let show_shortcut =
                        row.shortcut.is_some() && content_w >= 12 && budget >= shortcut_need + 2;
                    if show_shortcut {
                        budget = budget.saturating_sub(shortcut_need);
                    }
                    let badge_need = badge
                        .map(|b| {
                            u16::try_from(b.width())
                                .unwrap_or(u16::MAX)
                                .saturating_add(1)
                        })
                        .unwrap_or(0);
                    let show_badge = badge.is_some() && content_w >= 8 && budget > badge_need;
                    if show_badge {
                        budget = budget.saturating_sub(badge_need);
                    }
                    let secondary_need = row
                        .secondary
                        .as_ref()
                        .map(|s| {
                            u16::try_from(s.width())
                                .unwrap_or(u16::MAX)
                                .saturating_add(1)
                        })
                        .unwrap_or(0);
                    let show_secondary = row.secondary.is_some() && budget >= secondary_need;
                    if show_secondary {
                        budget = budget.saturating_sub(secondary_need);
                    }
                    let leading_need = if row.loading {
                        2
                    } else {
                        row.leading
                            .as_ref()
                            .map(|l| {
                                u16::try_from(l.width())
                                    .unwrap_or(u16::MAX)
                                    .saturating_add(1)
                            })
                            .unwrap_or(0)
                    };
                    let show_leading =
                        (row.loading || row.leading.is_some()) && budget >= leading_need;

                    let mut x = content_x;
                    let right = rect.right();
                    if show_leading {
                        if row.loading {
                            buffer.set_stringn(x, rect.y, "…", 1, style);
                            x = x.saturating_add(2);
                        } else if let Some(lead) = row.leading.as_ref() {
                            let lw = u16::try_from(lead.width())
                                .unwrap_or(u16::MAX)
                                .min(right.saturating_sub(x));
                            if lw > 0 {
                                buffer.set_line(x, rect.y, lead, lw);
                                x = x.saturating_add(lw).saturating_add(1);
                            }
                        }
                    }
                    let reserve = if show_badge { badge_need } else { 0 }
                        .saturating_add(if show_shortcut { shortcut_need } else { 0 });
                    let mid_end = right.saturating_sub(reserve);
                    let primary_budget = mid_end.saturating_sub(x);
                    if primary_budget > 0 {
                        buffer.set_line(x, rect.y, &row.label, primary_budget);
                        x = x.saturating_add(
                            u16::try_from(row.label.width())
                                .unwrap_or(u16::MAX)
                                .min(primary_budget),
                        );
                    }
                    if show_secondary && let Some(sec) = row.secondary.as_ref() {
                        let avail = mid_end.saturating_sub(x);
                        if avail > 2 {
                            x = x.saturating_add(1);
                            let sw = u16::try_from(sec.width())
                                .unwrap_or(u16::MAX)
                                .min(mid_end.saturating_sub(x));
                            if sw > 0 {
                                buffer.set_line(x, rect.y, sec, sw);
                            }
                        }
                    }
                    let mut cursor = right;
                    if show_shortcut && let Some(sc) = row.shortcut {
                        let w = u16::try_from(crate::text::display_cols(sc))
                            .unwrap_or(u16::MAX)
                            .min(cursor.saturating_sub(content_x));
                        if w > 0 {
                            cursor = cursor.saturating_sub(w);
                            buffer.set_stringn(cursor, rect.y, sc, usize::from(w), style);
                        }
                    }
                    if show_badge && let Some(b) = badge {
                        let w = u16::try_from(b.width())
                            .unwrap_or(u16::MAX)
                            .min(cursor.saturating_sub(content_x));
                        if w > 0 {
                            if show_shortcut {
                                cursor = cursor.saturating_sub(1);
                            }
                            cursor = cursor.saturating_sub(w);
                            buffer.set_line(cursor, rect.y, b, w);
                        }
                    }
                }
            }
            if row.enabled && row.role == RowRole::Item && !rect.is_empty() {
                state.regions.push(HitRegion {
                    id: row.id.clone(),
                    area: rect,
                });
            }
        }
        if scrollable {
            crate::scroll::render_scrollbar(
                buffer,
                Rect::new(area.right().saturating_sub(1), area.y, 1, area.height),
                crate::scroll::ScrollbarSpec::new(
                    crate::scroll::ScrollAxis::Vertical,
                    crate::scroll::ScrollbarGeometry::new(
                        self.rows.len(),
                        state.viewport_height,
                        u16::try_from(state.offset).unwrap_or(u16::MAX),
                    ),
                ),
                &self.tokens.theme,
            );
        }
        state.hovered = state.pointer.and_then(|position| {
            state
                .regions
                .iter()
                .find(|region| region.area.contains(position))
                .map(|region| region.id.clone())
        });
    }
}

fn render_check_cell<Id: Clone>(
    buffer: &mut Buffer,
    state: &mut ListState<Id>,
    row: &ListRow<'_, Id>,
    rect: Rect,
    check_x: u16,
    checked: bool,
    style: ratatui_core::style::Style,
) {
    if state.selection.is_none() || check_x >= rect.right() {
        return;
    }

    let marker = if checked { "[x] " } else { "[ ] " };
    let available = rect.right().saturating_sub(check_x);
    buffer.set_stringn(
        check_x,
        rect.y,
        marker,
        usize::from(available.min(4)),
        style,
    );
    if row.enabled && available >= 3 {
        state.check_regions.push(HitRegion {
            id: row.id.clone(),
            area: Rect::new(check_x, rect.y, 3, 1),
        });
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for List<'_, Id> {
    type State = ListState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

fn selectable_indices<Id>(rows: &[ListRow<'_, Id>]) -> Vec<usize> {
    rows.iter()
        .enumerate()
        .filter_map(|(index, row)| (row.enabled && row.role == RowRole::Item).then_some(index))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};
    use crate::interaction::{NavigationMove, UiIntent};

    #[test]
    fn handle_intent_moves_and_activates_without_raw_keys() {
        let rows = rows();
        let mut state = ListState::new(Some("first"));
        assert_eq!(
            state.handle_intent(&rows, UiIntent::Move(NavigationMove::Next)),
            Outcome::Changed
        );
        assert_eq!(state.selected(), Some(&"second"));
        assert_eq!(
            state.handle_intent(&rows, UiIntent::Activate),
            Outcome::Activated("second")
        );
        assert_eq!(
            state.handle_intent(&rows, UiIntent::Cancel),
            Outcome::Cancelled
        );
    }

    fn rows() -> [ListRow<'static, &'static str>; 4] {
        [
            ListRow {
                id: "section",
                label: Line::from("Section"),
                leading: None,
                secondary: None,
                badge: None,
                shortcut: None,
                trailing: None,
                role: RowRole::Separator,
                enabled: true,
                loading: false,
            },
            ListRow {
                id: "disabled",
                label: Line::from("Disabled"),
                leading: None,
                secondary: None,
                badge: None,
                shortcut: None,
                trailing: None,
                role: RowRole::Item,
                enabled: false,
                loading: false,
            },
            ListRow {
                id: "first",
                label: Line::from("First"),
                leading: None,
                secondary: None,
                badge: None,
                shortcut: None,
                trailing: None,
                role: RowRole::Item,
                enabled: true,
                loading: false,
            },
            ListRow {
                id: "second",
                label: Line::from("Second"),
                leading: None,
                secondary: None,
                badge: None,
                shortcut: None,
                trailing: None,
                role: RowRole::Item,
                enabled: true,
                loading: false,
            },
        ]
    }

    #[test]
    fn keyboard_skips_non_items_and_returns_stable_ids() {
        let rows = rows();
        let mut state = ListState::new(None);
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            Outcome::Changed
        );
        assert_eq!(state.selected(), Some(&"first"));
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            Outcome::Changed
        );
        assert_eq!(state.selected(), Some(&"second"));
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Outcome::Activated("second")
        );
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Outcome::Cancelled
        );
    }

    #[test]
    fn render_reveals_selection_and_mouse_uses_painted_regions() {
        let rows = rows();
        let tokens = DesignTokens::default();
        let mut state = ListState::new(Some("second"));
        let area = Rect::new(4, 3, 12, 1);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);
        assert_eq!(state.offset(), 3);
        assert_eq!(state.regions().len(), 1);
        let position = Position::new(area.x, area.y);
        assert_eq!(state.hover(position), Some(&"second"));
        assert_eq!(state.click(position), Outcome::Activated("second"));
        // Quiet phosphor selection uses design-token gutter glyph.
        assert_eq!(buffer[(area.x, area.y)].symbol(), "▌");
    }

    #[test]
    fn trailing_cells_align_right_and_wide_labels_truncate_first() {
        let rows = [
            ListRow {
                id: "wide",
                label: Line::from("🧪🧪label"),
                leading: None,
                secondary: None,
                badge: None,
                shortcut: None,
                trailing: Some(Line::from("9 KiB")),
                role: RowRole::Item,
                enabled: true,
                loading: false,
            },
            ListRow {
                id: "short",
                label: Line::from("short"),
                leading: None,
                secondary: None,
                badge: None,
                shortcut: None,
                trailing: Some(Line::from("1 B")),
                role: RowRole::Item,
                enabled: true,
                loading: false,
            },
        ];
        let tokens = DesignTokens::default();
        let mut state = ListState::new(None);
        // Gutter (2) + content: badge right-aligned within content band.
        let area = Rect::new(0, 0, 14, 2);
        let mut buffer = Buffer::empty(area);

        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);

        // Right edge of full row holds trailing badge.
        assert_eq!(buffer[(9, 0)].symbol(), "9");
        assert_eq!(buffer[(13, 0)].symbol(), "B");
        assert_eq!(buffer[(11, 1)].symbol(), "1");
        assert_eq!(buffer[(13, 1)].symbol(), "B");
        // Primary starts after gutter and keeps wide graphemes intact.
        assert_eq!(buffer[(2, 0)].symbol(), "🧪");
    }

    #[test]
    fn narrow_trailing_cell_clips_only_at_grapheme_boundaries() {
        let mut row = ListRow::item("wide-trailing", Line::from("x"));
        row.trailing = Some(Line::from("🧪Z"));
        let rows = [row];
        let tokens = DesignTokens::default();
        let mut state = ListState::new(None);
        // Gutter 2 + content 3: badge "🧪Z" (3 cells) fits; grapheme-safe clip drops Z if tighter.
        let area = Rect::new(0, 0, 5, 1);
        let mut buffer = Buffer::empty(area);

        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);

        let text: String = (0..5)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        // Badge is right-aligned in content; wide emoji either fully present or absent — never half.
        let emoji_count = text.matches('🧪').count();
        assert!(emoji_count <= 1, "must not split wide grapheme: {text:?}");
        if emoji_count == 1 {
            assert!(
                !text.contains('Z'),
                "clip after emoji not mid-grapheme: {text:?}"
            );
        }
    }

    #[test]
    fn composed_row_anatomy_paints_leading_secondary_shortcut() {
        let mut row = ListRow::item("job", Line::from("Build"));
        row.leading = Some(Line::from("*"));
        row.secondary = Some(Line::from("meta"));
        row.badge = Some(Line::from("ok"));
        row.shortcut = Some("⌘B");
        let rows = [row];
        let tokens = DesignTokens::default();
        let mut state = ListState::new(None);
        let area = Rect::new(0, 0, 40, 1);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);
        let text: String = (0..40)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(text.contains("Build"), "{text:?}");
        assert!(text.contains('*'), "{text:?}");
        assert!(text.contains("meta"), "{text:?}");
        assert!(text.contains("ok"), "{text:?}");
        assert!(text.contains('⌘') || text.contains('B'), "{text:?}");
    }

    #[test]
    fn narrow_list_drops_shortcut_before_primary_identity() {
        let mut row = ListRow::item("job", Line::from("Identity"));
        row.shortcut = Some("⌘K");
        row.badge = Some(Line::from("99"));
        let rows = [row];
        let tokens = DesignTokens::default();
        let mut state = ListState::new(None);
        // Gutter 2 + content 4: optional chrome must drop before primary.
        let area = Rect::new(0, 0, 6, 1);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);
        let text: String = (0..6)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            text.contains('I') || text.contains("Id"),
            "primary survives: {text:?}"
        );
        assert!(!text.contains('⌘'), "shortcut drops first: {text:?}");
    }

    #[test]
    fn list_check_toggle_reports_id() {
        let rows = rows();
        let tokens = DesignTokens::default();
        let mut state = ListState::new(Some("first"));
        state.enable_multi_select();

        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
            Outcome::CheckToggled("first")
        );
        assert!(state.selection().unwrap().is_checked(&"first"));

        let area = Rect::new(0, 0, 20, 4);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(2, 2)].symbol(), "[");
        assert_eq!(buffer[(3, 2)].symbol(), "x");
        assert_eq!(
            state.click(Position::new(2, 3)),
            Outcome::CheckToggled("second")
        );
        assert_eq!(state.selection().unwrap().checked(), ["first", "second"]);

        state.selection_mut().unwrap().clear();
        assert!(state.selection().unwrap().checked().is_empty());
        state.disable_multi_select();
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
            Outcome::Ignored
        );
    }

    #[test]
    fn list_state_accessors_preserve_semantic_ownership() {
        let mut state = ListState::new(Some("first"));

        assert_eq!(state.selected(), Some(&"first"));
        assert_eq!(state.hovered(), None);
        assert!(state.is_focused());
        assert_eq!(state.offset(), 0);
        assert!(state.regions().is_empty());

        state.select(Some("second"));
        state.set_focused(false);
        state.enable_multi_select();
        assert!(state.selection_mut().unwrap().toggle(&"second"));

        assert_eq!(state.selected(), Some(&"second"));
        assert!(!state.is_focused());
        assert_eq!(state.selection().unwrap().checked(), ["second"]);
    }

    #[test]
    fn indexed_picker_navigation_wraps_keys_and_bounds_gestures() {
        let mut state = ListState::for_count(3);
        assert_eq!(state.selected(), Some(&0));
        assert!(state.cycle_index(3, -1));
        assert_eq!(state.selected(), Some(&2));
        assert!(state.cycle_index(3, 1));
        assert_eq!(state.selected(), Some(&0));
        assert!(state.move_index(3, 9));
        assert_eq!(state.selected(), Some(&2));
        assert!(!state.move_index(3, 9));
        assert_eq!(state.selected_item(&["a", "b", "c"]), Some(&"c"));

        state.reconcile_count(1);
        assert_eq!(state.selected(), Some(&0));
        state.reconcile_count(0);
        assert_eq!(state.selected(), None);
    }
}

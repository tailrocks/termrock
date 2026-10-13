//! Popup candidate menu with caller-owned ranking and stable IDs.
//!
//! TermRock owns selection, scroll, clamp/flip geometry relative to an anchor,
//! and keyboard/mouse routing. Callers own candidate text, ranking, filtering,
//! and commit policy (which token range to replace). The menu never parses
//! language or talks to a database.

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind},
    style::{Role, Theme},
    text::{display_cols, take_display_cols},
};

/// One borrowed completion candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletionCandidate<'a, Id> {
    /// Stable identity (caller-owned; used for selection and commit).
    pub id: Id,
    /// Primary label (Unicode display-width measured).
    pub label: &'a str,
    /// Optional trailing annotation (kind, signature fragment, etc.).
    pub kind: Option<&'a str>,
    /// Whether this candidate accepts selection and commit.
    pub enabled: bool,
}

impl<'a, Id> CompletionCandidate<'a, Id> {
    /// Creates an enabled candidate without a kind annotation.
    #[must_use]
    pub const fn new(id: Id, label: &'a str) -> Self {
        Self {
            id,
            label,
            kind: None,
            enabled: true,
        }
    }

    /// Adds a trailing kind annotation.
    #[must_use]
    pub const fn kind(mut self, kind: &'a str) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Marks the candidate disabled (visible but not selectable).
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Semantic outcomes from menu interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompletionMenuOutcome<Id> {
    /// Event not applicable to the menu.
    Ignored,
    /// Selected identity changed (keyboard or hover).
    SelectionChanged,
    /// Caller should commit the given candidate id.
    Committed(Id),
    /// Caller should dismiss the menu (Escape or cancel).
    Dismissed,
}

/// Runtime state for [`CompletionMenu`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionMenuState<Id> {
    selected: Option<Id>,
    hovered: Option<Id>,
    offset: usize,
    viewport_height: usize,
    /// Last painted menu rect (for hit testing).
    painted: Rect,
    /// Row hit regions: (id, rect).
    hits: Vec<(Id, Rect)>,
    open: bool,
}

impl<Id> Default for CompletionMenuState<Id> {
    fn default() -> Self {
        Self::new(None)
    }
}

impl<Id> CompletionMenuState<Id> {
    /// Creates state with an optional initial selection.
    #[must_use]
    pub const fn new(selected: Option<Id>) -> Self {
        Self {
            selected,
            hovered: None,
            offset: 0,
            viewport_height: 0,
            painted: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            hits: Vec::new(),
            open: true,
        }
    }

    /// Whether the menu is open (caller may force-close).
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Opens or closes the menu.
    pub fn set_open(&mut self, open: bool) {
        self.open = open;
        if !open {
            self.hovered = None;
            self.hits.clear();
        }
    }

    /// Selected candidate identity.
    #[must_use]
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// Replace the selected identity.
    pub fn select(&mut self, selected: Option<Id>) {
        self.selected = selected;
    }

    /// Painted menu geometry from the last render.
    #[must_use]
    pub const fn painted(&self) -> Rect {
        self.painted
    }

    /// Scroll offset (first visible candidate index).
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }
}

impl<Id: Clone + PartialEq> CompletionMenuState<Id> {
    /// Reconcile selection after the caller rebuilds the candidate list.
    ///
    /// Keeps the previous id when still present; otherwise selects the first
    /// enabled candidate (or none when empty).
    pub fn reconcile(&mut self, candidates: &[CompletionCandidate<'_, Id>]) {
        if let Some(selected) = self.selected.clone() {
            if candidates.iter().any(|c| c.id == selected && c.enabled) {
                self.ensure_visible(candidates);
                return;
            }
        }
        self.selected = candidates
            .iter()
            .find(|c| c.enabled)
            .map(|c| c.id.clone());
        self.offset = 0;
        self.ensure_visible(candidates);
    }

    fn ensure_visible(&mut self, candidates: &[CompletionCandidate<'_, Id>]) {
        let Some(selected) = self.selected.as_ref() else {
            return;
        };
        let Some(index) = candidates.iter().position(|c| &c.id == selected) else {
            return;
        };
        let height = self.viewport_height.max(1);
        if index < self.offset {
            self.offset = index;
        } else if index >= self.offset.saturating_add(height) {
            self.offset = index.saturating_add(1).saturating_sub(height);
        }
        let max_offset = candidates.len().saturating_sub(height);
        if self.offset > max_offset {
            self.offset = max_offset;
        }
    }

    /// Move selection by `delta` enabled candidates.
    pub fn move_by(
        &mut self,
        candidates: &[CompletionCandidate<'_, Id>],
        delta: isize,
    ) -> CompletionMenuOutcome<Id> {
        if candidates.is_empty() || delta == 0 {
            return CompletionMenuOutcome::Ignored;
        }
        let enabled: Vec<usize> = candidates
            .iter()
            .enumerate()
            .filter_map(|(i, c)| c.enabled.then_some(i))
            .collect();
        if enabled.is_empty() {
            return CompletionMenuOutcome::Ignored;
        }
        let current = self
            .selected
            .as_ref()
            .and_then(|id| candidates.iter().position(|c| &c.id == id))
            .and_then(|idx| enabled.iter().position(|&i| i == idx))
            .unwrap_or(0);
        let len = enabled.len() as isize;
        let next = (current as isize + delta).rem_euclid(len) as usize;
        let new_id = candidates[enabled[next]].id.clone();
        if self.selected.as_ref() == Some(&new_id) {
            return CompletionMenuOutcome::Ignored;
        }
        self.selected = Some(new_id);
        self.ensure_visible(candidates);
        CompletionMenuOutcome::SelectionChanged
    }

    /// Commit the current selection.
    pub fn commit(
        &mut self,
        candidates: &[CompletionCandidate<'_, Id>],
    ) -> CompletionMenuOutcome<Id> {
        let Some(id) = self.selected.clone() else {
            return CompletionMenuOutcome::Ignored;
        };
        if !candidates.iter().any(|c| c.id == id && c.enabled) {
            return CompletionMenuOutcome::Ignored;
        }
        CompletionMenuOutcome::Committed(id)
    }

    /// Route a key event. Commit/dismiss semantics are reported; callers map
    /// Enter/Tab/Escape as they prefer by forwarding those keys here.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        candidates: &[CompletionCandidate<'_, Id>],
    ) -> CompletionMenuOutcome<Id> {
        if !self.open || key.kind == KeyEventKind::Release {
            return CompletionMenuOutcome::Ignored;
        }
        match key.code {
            KeyCode::Esc => {
                self.open = false;
                CompletionMenuOutcome::Dismissed
            }
            KeyCode::Up => self.move_by(candidates, -1),
            KeyCode::Down => self.move_by(candidates, 1),
            KeyCode::PageUp => {
                let step = isize::try_from(self.viewport_height.max(1)).unwrap_or(1);
                self.move_by(candidates, -step)
            }
            KeyCode::PageDown => {
                let step = isize::try_from(self.viewport_height.max(1)).unwrap_or(1);
                self.move_by(candidates, step)
            }
            KeyCode::Home => {
                let first = candidates.iter().find(|c| c.enabled).map(|c| c.id.clone());
                if first.is_some() && first != self.selected {
                    self.selected = first;
                    self.offset = 0;
                    self.ensure_visible(candidates);
                    CompletionMenuOutcome::SelectionChanged
                } else {
                    CompletionMenuOutcome::Ignored
                }
            }
            KeyCode::End => {
                let last = candidates
                    .iter()
                    .rev()
                    .find(|c| c.enabled)
                    .map(|c| c.id.clone());
                if last.is_some() && last != self.selected {
                    self.selected = last;
                    self.ensure_visible(candidates);
                    CompletionMenuOutcome::SelectionChanged
                } else {
                    CompletionMenuOutcome::Ignored
                }
            }
            KeyCode::Enter | KeyCode::Tab => self.commit(candidates),
            _ => CompletionMenuOutcome::Ignored,
        }
    }

    /// Route a mouse event against painted geometry.
    pub fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        candidates: &[CompletionCandidate<'_, Id>],
    ) -> CompletionMenuOutcome<Id> {
        if !self.open {
            return CompletionMenuOutcome::Ignored;
        }
        match mouse.kind {
            MouseEventKind::ScrollUp if self.painted.contains(mouse.position) => {
                self.move_by(candidates, -1)
            }
            MouseEventKind::ScrollDown if self.painted.contains(mouse.position) => {
                self.move_by(candidates, 1)
            }
            MouseEventKind::Moved | MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(id) = self.hit_at(mouse.position) {
                    if candidates.iter().any(|c| c.id == id && c.enabled)
                        && self.hovered.as_ref() != Some(&id)
                    {
                        self.hovered = Some(id.clone());
                        if self.selected.as_ref() != Some(&id) {
                            self.selected = Some(id);
                            self.ensure_visible(candidates);
                            return CompletionMenuOutcome::SelectionChanged;
                        }
                    }
                }
                CompletionMenuOutcome::Ignored
            }
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left) => {
                if let Some(id) = self.hit_at(mouse.position) {
                    if candidates.iter().any(|c| c.id == id && c.enabled) {
                        self.selected = Some(id.clone());
                        return CompletionMenuOutcome::Committed(id);
                    }
                }
                // Click outside dismisses.
                if !self.painted.contains(mouse.position) && !self.painted.is_empty() {
                    self.open = false;
                    return CompletionMenuOutcome::Dismissed;
                }
                CompletionMenuOutcome::Ignored
            }
            _ => CompletionMenuOutcome::Ignored,
        }
    }

    fn hit_at(&self, position: Position) -> Option<Id> {
        self.hits
            .iter()
            .find(|(_, rect)| rect.contains(position))
            .map(|(id, _)| id.clone())
    }
}

/// Preferred popup size before clamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletionMenuSize {
    /// Preferred width in cells.
    pub width: u16,
    /// Preferred height in rows (candidates visible).
    pub height: u16,
}

impl Default for CompletionMenuSize {
    fn default() -> Self {
        Self {
            width: 32,
            height: 8,
        }
    }
}

/// Compute a menu rectangle that never covers the anchor cell and stays inside
/// `bounds`. Prefers below-right of the anchor; flips above or left when needed.
#[must_use]
pub fn place_completion_menu(bounds: Rect, anchor: Rect, preferred: CompletionMenuSize) -> Rect {
    if bounds.is_empty() || preferred.width == 0 || preferred.height == 0 {
        return Rect::default();
    }
    let width = preferred.width.min(bounds.width).max(1);
    let height = preferred.height.min(bounds.height).max(1);

    // Prefer below the anchor row.
    let below_y = anchor.y.saturating_add(1);
    let space_below = bounds
        .y
        .saturating_add(bounds.height)
        .saturating_sub(below_y);
    let above_y_end = anchor.y;
    let space_above = above_y_end.saturating_sub(bounds.y);

    let y = if space_below >= height {
        below_y
    } else if space_above >= height {
        above_y_end.saturating_sub(height)
    } else if space_below >= space_above {
        below_y.min(
            bounds
                .y
                .saturating_add(bounds.height)
                .saturating_sub(height),
        )
    } else {
        bounds.y
    };

    // Prefer aligned to anchor.x; flip left if overflowing right edge.
    let right_limit = bounds.x.saturating_add(bounds.width);
    let x = if anchor.x.saturating_add(width) <= right_limit {
        anchor.x.max(bounds.x)
    } else {
        right_limit.saturating_sub(width).max(bounds.x)
    };

    // Final clamp inside bounds.
    let x = x.clamp(bounds.x, right_limit.saturating_sub(width));
    let y = y.clamp(
        bounds.y,
        bounds
            .y
            .saturating_add(bounds.height)
            .saturating_sub(height),
    );
    let rect = Rect::new(x, y, width, height);

    // Never cover the anchor cell: if still overlapping, shift vertically.
    if rect_intersects(rect, anchor) {
        if anchor.y > bounds.y {
            let flipped = Rect::new(
                rect.x,
                anchor.y.saturating_sub(height).max(bounds.y),
                width,
                height,
            );
            if !rect_intersects(flipped, anchor) {
                return flipped;
            }
        }
        let pushed = Rect::new(
            rect.x,
            anchor.y.saturating_add(1).min(
                bounds
                    .y
                    .saturating_add(bounds.height)
                    .saturating_sub(height),
            ),
            width,
            height,
        );
        if !rect_intersects(pushed, anchor) {
            return pushed;
        }
    }
    rect
}

fn rect_intersects(a: Rect, b: Rect) -> bool {
    let a_x2 = a.x.saturating_add(a.width);
    let a_y2 = a.y.saturating_add(a.height);
    let b_x2 = b.x.saturating_add(b.width);
    let b_y2 = b.y.saturating_add(b.height);
    a.x < b_x2 && b.x < a_x2 && a.y < b_y2 && b.y < a_y2
}

/// Popup completion list widget.
pub struct CompletionMenu<'a, Id> {
    candidates: &'a [CompletionCandidate<'a, Id>],
    theme: &'a Theme,
    empty_message: &'a str,
    /// Bounds inside which the menu must stay (typically the editor area).
    bounds: Rect,
    /// Cursor / anchor cell that must not be covered.
    anchor: Rect,
    preferred: CompletionMenuSize,
}

impl<'a, Id> CompletionMenu<'a, Id> {
    /// Creates a menu over borrowed candidates.
    #[must_use]
    pub const fn new(
        candidates: &'a [CompletionCandidate<'a, Id>],
        theme: &'a Theme,
        bounds: Rect,
        anchor: Rect,
    ) -> Self {
        Self {
            candidates,
            theme,
            empty_message: "No matches",
            bounds,
            anchor,
            preferred: CompletionMenuSize {
                width: 32,
                height: 8,
            },
        }
    }

    /// Preferred popup size before clamp.
    #[must_use]
    pub const fn preferred_size(mut self, size: CompletionMenuSize) -> Self {
        self.preferred = size;
        self
    }

    /// Empty-list cue.
    #[must_use]
    pub const fn empty_message(mut self, message: &'a str) -> Self {
        self.empty_message = message;
        self
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &CompletionMenu<'_, Id> {
    type State = CompletionMenuState<Id>;

    fn render(self, _area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.hits.clear();
        if !state.open {
            state.painted = Rect::default();
            return;
        }
        let mut preferred = self.preferred;
        if self.candidates.is_empty() {
            preferred.height = preferred.height.min(1);
        } else {
            preferred.height = preferred
                .height
                .min(u16::try_from(self.candidates.len()).unwrap_or(u16::MAX).max(1));
        }
        // Fit width to longest label+kind with padding, capped by preferred.
        let content_width = self
            .candidates
            .iter()
            .map(|c| {
                let kind = c.kind.map(display_cols).unwrap_or(0);
                display_cols(c.label).saturating_add(if kind == 0 { 0 } else { kind + 2 })
            })
            .max()
            .unwrap_or(display_cols(self.empty_message))
            .saturating_add(2);
        preferred.width = preferred
            .width
            .max(u16::try_from(content_width).unwrap_or(u16::MAX).min(preferred.width.max(12)));

        let menu = place_completion_menu(self.bounds, self.anchor, preferred);
        state.painted = menu;
        if menu.is_empty() {
            return;
        }
        state.viewport_height = usize::from(menu.height);
        state.reconcile(self.candidates);

        // Clear and border-fill background.
        let bg = self.theme.style(Role::Surface);
        let border = self.theme.style(Role::BorderFocused);
        for y in menu.y..menu.y.saturating_add(menu.height) {
            for x in menu.x..menu.x.saturating_add(menu.width) {
                if let Some(cell) = buffer.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(bg);
                }
            }
        }
        // Top/bottom edge markers (non-color: use box-drawing if width allows).
        if menu.width >= 2 && menu.height >= 1 {
            if let Some(cell) = buffer.cell_mut((menu.x, menu.y)) {
                cell.set_symbol("┌");
                cell.set_style(border);
            }
            if let Some(cell) = buffer.cell_mut((menu.x.saturating_add(menu.width).saturating_sub(1), menu.y))
            {
                cell.set_symbol("┐");
                cell.set_style(border);
            }
        }

        if self.candidates.is_empty() {
            let text = take_display_cols(self.empty_message, usize::from(menu.width.saturating_sub(2)));
            buffer.set_stringn(
                menu.x.saturating_add(1),
                menu.y,
                text,
                usize::from(menu.width.saturating_sub(2)),
                self.theme.style(Role::TextMuted),
            );
            return;
        }

        let max_offset = self
            .candidates
            .len()
            .saturating_sub(usize::from(menu.height));
        if state.offset > max_offset {
            state.offset = max_offset;
        }
        let end = (state.offset + usize::from(menu.height)).min(self.candidates.len());
        for (row, candidate) in self.candidates[state.offset..end].iter().enumerate() {
            let y = menu.y.saturating_add(u16::try_from(row).unwrap_or(u16::MAX));
            let row_rect = Rect::new(menu.x, y, menu.width, 1);
            state.hits.push((candidate.id.clone(), row_rect));

            let selected = state.selected.as_ref() == Some(&candidate.id);
            let hovered = state.hovered.as_ref() == Some(&candidate.id);
            let style = row_style(self.theme, candidate.enabled, selected, hovered);

            let kind_cols = candidate.kind.map(display_cols).unwrap_or(0);
            let kind_budget = if kind_cols == 0 {
                0
            } else {
                kind_cols.saturating_add(1)
            };
            let label_budget = usize::from(menu.width)
                .saturating_sub(2)
                .saturating_sub(kind_budget);
            let label = take_display_cols(candidate.label, label_budget);
            buffer.set_stringn(
                menu.x.saturating_add(1),
                y,
                label,
                label_budget,
                style,
            );
            if let Some(kind) = candidate.kind {
                let kind_text = take_display_cols(kind, kind_cols.min(usize::from(menu.width) / 2));
                let kind_x = menu
                    .x
                    .saturating_add(menu.width)
                    .saturating_sub(u16::try_from(display_cols(&kind_text) + 1).unwrap_or(1));
                buffer.set_stringn(
                    kind_x,
                    y,
                    kind_text,
                    usize::from(menu.width.saturating_sub(2)),
                    if candidate.enabled {
                        self.theme.style(Role::TextMuted)
                    } else {
                        style
                    },
                );
            }
            // Non-color selection marker in the left gutter.
            if selected {
                if let Some(cell) = buffer.cell_mut((menu.x, y)) {
                    cell.set_symbol("›");
                    cell.set_style(style);
                }
            }
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for CompletionMenu<'_, Id> {
    type State = CompletionMenuState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}

fn row_style(theme: &Theme, enabled: bool, selected: bool, hovered: bool) -> Style {
    if !enabled {
        theme.style(Role::TextDisabled)
    } else if selected {
        theme.style(Role::Selection)
    } else if hovered {
        theme.style(Role::Focus)
    } else {
        theme.style(Role::Text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;
    use ratatui_core::layout::Rect;

    fn candidates(ids: &[&'static str]) -> Vec<CompletionCandidate<'static, &'static str>> {
        ids.iter()
            .map(|id| CompletionCandidate::new(*id, *id))
            .collect()
    }

    #[test]
    fn place_prefers_below_anchor_without_covering() {
        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(10, 5, 1, 1);
        let menu = place_completion_menu(
            bounds,
            anchor,
            CompletionMenuSize {
                width: 20,
                height: 6,
            },
        );
        assert_eq!(menu.y, 6, "below anchor");
        assert!(!rect_intersects(menu, anchor));
        assert!(menu.x >= bounds.x);
        assert!(menu.x + menu.width <= bounds.x + bounds.width);
    }

    #[test]
    fn place_flips_above_when_bottom_edge() {
        let bounds = Rect::new(0, 0, 80, 20);
        let anchor = Rect::new(10, 18, 1, 1);
        let menu = place_completion_menu(
            bounds,
            anchor,
            CompletionMenuSize {
                width: 20,
                height: 6,
            },
        );
        assert!(menu.y + menu.height <= anchor.y, "above anchor: {menu:?}");
        assert!(!rect_intersects(menu, anchor));
    }

    #[test]
    fn place_clamps_right_edge() {
        let bounds = Rect::new(0, 0, 40, 20);
        let anchor = Rect::new(35, 2, 1, 1);
        let menu = place_completion_menu(
            bounds,
            anchor,
            CompletionMenuSize {
                width: 20,
                height: 4,
            },
        );
        assert!(menu.x + menu.width <= 40);
        assert!(!rect_intersects(menu, anchor));
    }

    #[test]
    fn keyboard_moves_commits_and_dismisses() {
        let items = candidates(&["alpha", "beta", "gamma"]);
        let mut state = CompletionMenuState::new(Some("alpha"));
        state.viewport_height = 3;
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                &items
            ),
            CompletionMenuOutcome::SelectionChanged
        );
        assert_eq!(state.selected().copied(), Some("beta"));
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &items
            ),
            CompletionMenuOutcome::Committed("beta")
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &items),
            CompletionMenuOutcome::Dismissed
        );
        assert!(!state.is_open());
    }

    #[test]
    fn reconcile_keeps_id_then_falls_back() {
        let mut state = CompletionMenuState::new(Some("beta"));
        state.reconcile(&candidates(&["alpha", "beta", "gamma"]));
        assert_eq!(state.selected().copied(), Some("beta"));
        state.reconcile(&candidates(&["alpha", "gamma"]));
        assert_eq!(state.selected().copied(), Some("alpha"));
        state.reconcile(&[]);
        assert_eq!(state.selected().copied(), None);
    }

    #[test]
    fn mouse_click_commits_selected_hit() {
        let items = candidates(&["one", "two"]);
        let mut state = CompletionMenuState::new(Some("one"));
        state.open = true;
        state.painted = Rect::new(0, 0, 20, 2);
        state.hits = vec![
            ("one", Rect::new(0, 0, 20, 1)),
            ("two", Rect::new(0, 1, 20, 1)),
        ];
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position { x: 2, y: 1 },
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            state.handle_mouse(event, &items),
            CompletionMenuOutcome::Committed("two")
        );
    }
}

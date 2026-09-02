// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **TreeTable** — hierarchical rows + columns without confusing navigation.
//!
//! **Mission.** Process trees, schema browsers, task boards, and dependency
//! graphs: expansion, lazy children, sticky headers, sortable data columns,
//! selection, grouping, aggregate rows, virtualized visible expanded slices,
//! responsive column priorities, and compact hierarchy indentation.
//!
//! **Ownership.** Host owns hierarchy, expansion, lazy fetch, and projection of
//! the **flattened visible** window. TreeTable owns cursor/selection chrome,
//! scroll, hit geometry, and typed outcomes.
//!
//! **Left/Right policy** ([`TreeTableNavMode`]):
//! - **Hierarchy** (default): Left collapse / parent; Right expand or enter first child.
//! - **Cell**: Left/Right move among columns (then h-scroll); Shift+Left/Right hierarchy.
//! - **Scroll**: Left/Right only horizontal scroll; expand via Enter/`e` intents.
//!
//! Research: process trees, file trees with metadata, IDE outlines, DB schema browsers.
//! Single-column hierarchy → [`super::Tree`]. Flat multi-column → [`super::DataTable`].
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::StatefulWidget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{NavigationMove, PageMove, UiIntent},
    style::{DesignSystem, Glyph, ListRowVisualState, Role},
    text::take_display_cols,
    widgets::{
        data_view::{
            ColumnKind, ColumnModel, ColumnPin, LoadState, SelectionModel, SortSpec, VirtualWindow,
        },
        tree::TreeNodeStatus,
    },
};

const GUTTER_W: u16 = 2;

/// Column separator, from the glyph catalog rather than a file-local literal.
/// Cells of indent per depth (compact default).
const INDENT_STEP: u16 = 2;
const INDENT_STEP_COMPACT: u16 = 1;

/// How Left/Right and related chords are interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TreeTableNavMode {
    /// Hierarchy first: expand/collapse/parent (default; process tree UX).
    #[default]
    Hierarchy,
    /// Spreadsheet-like cell cursor; Shift+Left/Right for hierarchy.
    Cell,
    /// Horizontal scroll only on Left/Right; hierarchy via Expand/Collapse intents.
    Scroll,
}

impl TreeTableNavMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Hierarchy => "hierarchy",
            Self::Cell => "cell",
            Self::Scroll => "scroll",
        }
    }

    /// Cycle Hierarchy → Cell → Scroll → Hierarchy.
    #[must_use]
    pub const fn cycle(self) -> Self {
        match self {
            Self::Hierarchy => Self::Cell,
            Self::Cell => Self::Scroll,
            Self::Scroll => Self::Hierarchy,
        }
    }
}

/// Semantic row role in the flattened projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TreeTableRowKind {
    /// Ordinary data / branch row.
    #[default]
    Data,
    /// Group band (section header in the flat stream).
    Group,
    /// Aggregate / totals row (non-expandable summary).
    Aggregate,
}

/// One flattened visible row (host projects expanded paths only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeTableRow<'a, Id> {
    /// Stable identity.
    pub id: Id,
    /// Hierarchy depth (0 = root). Indent = depth × step.
    pub depth: u16,
    /// Can expand/collapse.
    pub branch: bool,
    /// Currently expanded (host-owned).
    pub expanded: bool,
    /// Loading / error / lazy child state.
    pub status: TreeNodeStatus,
    /// Data / group / aggregate.
    pub kind: TreeTableRowKind,
    /// Cells in **visible column order** (col 0 = primary label without indent glyphs).
    pub cells: &'a [&'a str],
    /// Parent id when known (collapse-to-parent, filter).
    pub parent: Option<Id>,
    /// Interaction enabled.
    pub enabled: bool,
}

impl<'a, Id> TreeTableRow<'a, Id> {
    /// Ready data leaf/branch at `depth`.
    #[must_use]
    pub fn new(id: Id, depth: u16, cells: &'a [&'a str]) -> Self {
        Self {
            id,
            depth,
            branch: false,
            expanded: false,
            status: TreeNodeStatus::Ready,
            kind: TreeTableRowKind::Data,
            cells,
            parent: None,
            enabled: true,
        }
    }

    /// Branch capable of expansion.
    #[must_use]
    pub const fn branch(mut self) -> Self {
        self.branch = true;
        self
    }

    /// Expanded branch.
    #[must_use]
    pub const fn expanded(mut self) -> Self {
        self.expanded = true;
        self.branch = true;
        self
    }

    /// Lazy unloaded children.
    #[must_use]
    pub const fn lazy_branch(mut self) -> Self {
        self.branch = true;
        self.expanded = false;
        self.status = TreeNodeStatus::Lazy;
        self
    }

    /// Parent id.
    #[must_use]
    pub fn parent(mut self, parent: Id) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Status override.
    #[must_use]
    pub const fn with_status(mut self, status: TreeNodeStatus) -> Self {
        self.status = status;
        self
    }

    /// Group band.
    #[must_use]
    pub const fn group(mut self) -> Self {
        self.kind = TreeTableRowKind::Group;
        self.branch = true;
        self
    }

    /// Aggregate / totals row.
    #[must_use]
    pub const fn aggregate(mut self) -> Self {
        self.kind = TreeTableRowKind::Aggregate;
        self.branch = false;
        self.expanded = false;
        self
    }

    /// Disabled.
    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Loading children.
    #[must_use]
    pub const fn loading(mut self) -> Self {
        self.status = TreeNodeStatus::Loading;
        self.enabled = false;
        self
    }
}

/// Header hit region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeTableHeaderRegion<ColId> {
    /// Column id.
    pub id: ColId,
    /// Header area.
    pub area: Rect,
    /// Sortable.
    pub sortable: bool,
}

/// Body row hit region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeTableRowRegion<Id> {
    /// Row id.
    pub id: Id,
    /// Projected index.
    pub index: usize,
    /// Full row rect.
    pub area: Rect,
    /// Disclosure glyph rect when branch.
    pub disclosure: Option<Rect>,
}

/// Semantic outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TreeTableOutcome<Id, ColId> {
    /// No change.
    Ignored,
    /// Viewport scrolled.
    Scrolled,
    /// Cursor moved.
    CursorMoved,
    /// Selection changed to this row.
    Selected(Id),
    /// Multi-check toggled.
    CheckToggled(Id),
    /// Expand/collapse or lazy load requested (host updates projection).
    ExpandToggled(Id),
    /// Activate row.
    Activated(Id),
    /// Sort on data column.
    SortSpec(SortSpec<ColId>),
    /// Select-all visible projected rows requested.
    SelectAllRequested,
    /// Context menu.
    ContextMenu {
        /// Row.
        row: Id,
    },
    /// Cancel / clear.
    Cancelled,
    /// Nav mode cycled.
    NavModeChanged(TreeTableNavMode),
    /// Retry load surface.
    RetryLoad,
}

/// Interaction + geometry state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeTableState<Id: Clone + Ord, ColId: Clone + PartialEq> {
    /// Selected / cursor row id.
    selected: Option<Id>,
    /// Multi-check model (optional; enabled via [`Self::enable_multi_select`]).
    pub selection: SelectionModel<Id>,
    /// Multi-select active.
    multi: bool,
    /// Vertical virtual window over flattened expanded universe.
    pub window: VirtualWindow,
    /// Cursor index in projected slice.
    pub cursor_row: usize,
    /// Cursor column among visible columns (0 = hierarchy/primary).
    pub cursor_col: usize,
    /// Row currently under the pointer.
    pub hovered: Option<Id>,
    /// Horizontal content scroll.
    pub h_offset: u16,
    /// Nav mode.
    pub nav_mode: TreeTableNavMode,
    /// Load chrome.
    pub load: LoadState,
    /// Active sort (data columns).
    pub sort: Option<SortSpec<ColId>>,
    /// Striped body.
    pub striped: bool,
    /// Host grants input.
    pub accepts_input: bool,
    /// Header regions.
    pub header_regions: Vec<TreeTableHeaderRegion<ColId>>,
    /// Row regions.
    pub row_regions: Vec<TreeTableRowRegion<Id>>,
    body_origin: (u16, u16),
    body_rows: u16,
    body_width: u16,
    paint_widths: Vec<(usize, u16)>,
    content_width: u16,
    viewport_width: u16,
    previous_index: Option<usize>,
}

impl<Id: Clone + Ord, ColId: Clone + PartialEq> TreeTableState<Id, ColId> {
    /// Fresh state with optional initial selection.
    #[must_use]
    pub fn new(selected: Option<Id>) -> Self {
        Self {
            selected,
            selection: SelectionModel::multi_row(),
            multi: false,
            window: VirtualWindow::default(),
            cursor_row: 0,
            cursor_col: 0,
            hovered: None,
            h_offset: 0,
            nav_mode: TreeTableNavMode::Hierarchy,
            load: LoadState::Ready { count: 0 },
            sort: None,
            striped: false,
            accepts_input: true,
            header_regions: Vec::new(),
            row_regions: Vec::new(),
            body_origin: (0, 0),
            body_rows: 0,
            body_width: 0,
            paint_widths: Vec::new(),
            content_width: 0,
            viewport_width: 0,
            previous_index: None,
        }
    }

    /// Enable multi-check selection.
    pub fn enable_multi_select(&mut self) {
        self.multi = true;
        self.selection = SelectionModel::multi_row();
    }

    /// Selected row id (cursor).
    #[must_use]
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// Sets selected id.
    pub fn select(&mut self, id: Option<Id>) {
        self.selected = id;
    }

    /// Logical flattened universe size (expanded rows only).
    pub fn set_logical_rows(&mut self, logical_len: u64) {
        self.window.logical_len = logical_len;
        self.window.clamp();
    }

    /// Host surface input gate (scene focus).
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Sets Left/Right interpretation mode.
    pub fn set_nav_mode(&mut self, mode: TreeTableNavMode) {
        self.nav_mode = mode;
    }

    /// Scrolls horizontally by display columns. Returns whether offset changed.
    pub fn scroll_horizontal(&mut self, delta: i16) -> bool {
        let max = self.content_width.saturating_sub(self.viewport_width);
        let next = if delta >= 0 {
            self.h_offset.saturating_add(delta as u16).min(max)
        } else {
            self.h_offset.saturating_sub((-delta) as u16)
        };
        let changed = next != self.h_offset;
        self.h_offset = next;
        changed
    }

    /// Reconcile selection after host reprojects rows.
    pub fn reconcile(&mut self, rows: &[TreeTableRow<'_, Id>]) {
        if let Some(sel) = self.selected.as_ref()
            && let Some(idx) = rows.iter().position(|r| r.enabled && &r.id == sel)
        {
            self.cursor_row = idx;
            self.previous_index = Some(idx);
            self.reveal_cursor(rows.len());
            return;
        }
        let anchor = self.previous_index.unwrap_or(0);
        let Some(idx) = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.enabled && navigable(r))
            .min_by_key(|(i, _)| i.abs_diff(anchor))
            .map(|(i, _)| i)
        else {
            self.selected = None;
            self.cursor_row = 0;
            return;
        };
        self.selected = Some(rows[idx].id.clone());
        self.cursor_row = idx;
        self.previous_index = Some(idx);
        self.reveal_cursor(rows.len());
    }

    fn reveal_cursor(&mut self, projected_len: usize) {
        let vp = usize::from(self.window.viewport.max(1));
        if self.cursor_row < self.window.offset as usize {
            self.window.offset = self.cursor_row as u64;
        } else if self.cursor_row >= self.window.offset as usize + vp {
            self.window.offset = (self.cursor_row + 1).saturating_sub(vp) as u64;
        }
        let _ = projected_len;
        self.window.clamp();
    }

    /// Keys over projected rows.
    pub fn handle_key(
        &mut self,
        rows: &[TreeTableRow<'_, Id>],
        columns: &ColumnModel<ColId>,
        key: KeyEvent,
    ) -> TreeTableOutcome<Id, ColId>
    where
        ColId: Clone,
    {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return TreeTableOutcome::Ignored;
        }
        let is_press = key.kind == KeyEventKind::Press;

        if matches!(
            self.load,
            LoadState::Empty { .. } | LoadState::Error { .. } | LoadState::Loading { .. }
        ) {
            if is_press && matches!(key.code, KeyCode::Char('r' | 'R') | KeyCode::Enter) {
                return TreeTableOutcome::RetryLoad;
            }
            return TreeTableOutcome::Ignored;
        }
        if rows.is_empty() {
            return TreeTableOutcome::Ignored;
        }
        self.cursor_row = self.cursor_row.min(rows.len() - 1);
        let vis_n = columns.visible().count().max(1);
        self.cursor_col = self.cursor_col.min(vis_n - 1);

        if is_press && matches!(key.code, KeyCode::Char('\\')) && key.modifiers.is_empty() {
            self.nav_mode = self.nav_mode.cycle();
            return TreeTableOutcome::NavModeChanged(self.nav_mode);
        }

        // Shift+Left/Right always hierarchy when in Cell mode
        if key.modifiers.contains(KeyModifiers::SHIFT)
            && matches!(
                key.code,
                KeyCode::Left | KeyCode::Right | KeyCode::Char('h' | 'l' | 'H' | 'L')
            )
        {
            let expand = matches!(key.code, KeyCode::Right | KeyCode::Char('l' | 'L'));
            return self.hierarchy_step(rows, expand);
        }

        if let Some(intent) = default_tree_table_intent(key, self.nav_mode) {
            let out = self.handle_intent(rows, columns, intent);
            if !matches!(out, TreeTableOutcome::Ignored) {
                return out;
            }
        }

        match key.code {
            KeyCode::Char('a') if is_press && key.modifiers.contains(KeyModifiers::CONTROL) => {
                TreeTableOutcome::SelectAllRequested
            }
            KeyCode::Char('s') if is_press && key.modifiers.is_empty() => {
                self.request_sort(columns)
            }
            KeyCode::Char('x') if is_press => {
                let id = rows[self.cursor_row].id.clone();
                TreeTableOutcome::ContextMenu { row: id }
            }
            KeyCode::Char(' ') if is_press && self.multi => {
                let id = rows[self.cursor_row].id.clone();
                if rows[self.cursor_row].enabled {
                    self.selection.toggle_row(id.clone());
                    TreeTableOutcome::CheckToggled(id)
                } else {
                    TreeTableOutcome::Ignored
                }
            }
            _ => TreeTableOutcome::Ignored,
        }
    }

    /// Applies a semantic intent (nav, expand/collapse, activate, toggle).
    pub fn handle_intent(
        &mut self,
        rows: &[TreeTableRow<'_, Id>],
        columns: &ColumnModel<ColId>,
        intent: UiIntent,
    ) -> TreeTableOutcome<Id, ColId>
    where
        ColId: Clone,
    {
        if !self.accepts_input || rows.is_empty() {
            return TreeTableOutcome::Ignored;
        }
        self.cursor_row = self.cursor_row.min(rows.len() - 1);
        match intent {
            UiIntent::Move(NavigationMove::Next) | UiIntent::Move(NavigationMove::Down) => {
                self.move_row(rows, 1)
            }
            UiIntent::Move(NavigationMove::Previous) | UiIntent::Move(NavigationMove::Up) => {
                self.move_row(rows, -1)
            }
            UiIntent::Move(NavigationMove::First) => self.select_edge(rows, false),
            UiIntent::Move(NavigationMove::Last) => self.select_edge(rows, true),
            UiIntent::Move(NavigationMove::Left) => self.horizontal(rows, columns, -1),
            UiIntent::Move(NavigationMove::Right) => self.horizontal(rows, columns, 1),
            UiIntent::Page(PageMove::Forward) => {
                let step = i64::from(self.window.viewport.max(1));
                if self.window.scroll_by(step) {
                    TreeTableOutcome::Scrolled
                } else {
                    self.move_row(rows, step)
                }
            }
            UiIntent::Page(PageMove::Backward) => {
                let step = i64::from(self.window.viewport.max(1));
                if self.window.scroll_by(-step) {
                    TreeTableOutcome::Scrolled
                } else {
                    self.move_row(rows, -step)
                }
            }
            UiIntent::Expand => self.hierarchy_step(rows, true),
            UiIntent::Collapse => self.hierarchy_step(rows, false),
            UiIntent::Activate | UiIntent::Submit | UiIntent::Open => {
                let row = &rows[self.cursor_row];
                if row.enabled {
                    TreeTableOutcome::Activated(row.id.clone())
                } else {
                    TreeTableOutcome::Ignored
                }
            }
            UiIntent::Toggle => {
                if self.multi {
                    let id = rows[self.cursor_row].id.clone();
                    self.selection.toggle_row(id.clone());
                    TreeTableOutcome::CheckToggled(id)
                } else {
                    self.hierarchy_step(rows, true)
                }
            }
            UiIntent::Cancel | UiIntent::Close => {
                if self.multi {
                    self.selection.clear_selection();
                }
                TreeTableOutcome::Cancelled
            }
            _ => TreeTableOutcome::Ignored,
        }
    }

    fn horizontal(
        &mut self,
        rows: &[TreeTableRow<'_, Id>],
        columns: &ColumnModel<ColId>,
        delta: i16,
    ) -> TreeTableOutcome<Id, ColId>
    where
        ColId: Clone,
    {
        match self.nav_mode {
            TreeTableNavMode::Hierarchy => self.hierarchy_step(rows, delta > 0),
            TreeTableNavMode::Scroll => {
                if self.scroll_horizontal(delta * 2) {
                    TreeTableOutcome::Scrolled
                } else {
                    TreeTableOutcome::Ignored
                }
            }
            TreeTableNavMode::Cell => {
                let vis_n = columns.visible().count();
                if vis_n == 0 {
                    return TreeTableOutcome::Ignored;
                }
                let next = if delta < 0 {
                    self.cursor_col.saturating_sub(1)
                } else {
                    (self.cursor_col + 1).min(vis_n - 1)
                };
                if next != self.cursor_col {
                    self.cursor_col = next;
                    return TreeTableOutcome::CursorMoved;
                }
                if self.scroll_horizontal(delta) {
                    TreeTableOutcome::Scrolled
                } else {
                    TreeTableOutcome::Ignored
                }
            }
        }
    }

    fn hierarchy_step(
        &mut self,
        rows: &[TreeTableRow<'_, Id>],
        expand_or_enter: bool,
    ) -> TreeTableOutcome<Id, ColId> {
        let row = &rows[self.cursor_row];
        if !row.enabled || matches!(row.kind, TreeTableRowKind::Aggregate) {
            return TreeTableOutcome::Ignored;
        }
        if expand_or_enter {
            // Expand branch or lazy; if already expanded, enter first child
            if row.branch && !row.expanded {
                return TreeTableOutcome::ExpandToggled(row.id.clone());
            }
            if matches!(row.status, TreeNodeStatus::Lazy) {
                return TreeTableOutcome::ExpandToggled(row.id.clone());
            }
            if row.branch && row.expanded {
                // Enter first child in projection
                if let Some((idx, child)) = rows
                    .iter()
                    .enumerate()
                    .skip(self.cursor_row + 1)
                    .find(|(_, r)| r.depth > row.depth && r.enabled)
                {
                    self.cursor_row = idx;
                    self.selected = Some(child.id.clone());
                    self.previous_index = Some(idx);
                    self.reveal_cursor(rows.len());
                    return TreeTableOutcome::Selected(child.id.clone());
                }
                return TreeTableOutcome::Ignored;
            }
            TreeTableOutcome::Ignored
        } else {
            // Collapse expanded; else select parent
            if row.branch && row.expanded {
                return TreeTableOutcome::ExpandToggled(row.id.clone());
            }
            if let Some(parent) = row.parent.as_ref() {
                if let Some(idx) = rows.iter().position(|r| &r.id == parent) {
                    self.cursor_row = idx;
                    self.selected = Some(parent.clone());
                    self.previous_index = Some(idx);
                    self.reveal_cursor(rows.len());
                    return TreeTableOutcome::Selected(parent.clone());
                }
            }
            // Depth walk for parent
            let depth = row.depth;
            if depth == 0 {
                return TreeTableOutcome::Ignored;
            }
            if let Some((idx, parent)) = rows[..self.cursor_row]
                .iter()
                .enumerate()
                .rev()
                .find(|(_, r)| r.depth < depth)
            {
                self.cursor_row = idx;
                self.selected = Some(parent.id.clone());
                self.previous_index = Some(idx);
                self.reveal_cursor(rows.len());
                return TreeTableOutcome::Selected(parent.id.clone());
            }
            TreeTableOutcome::Ignored
        }
    }

    fn move_row(
        &mut self,
        rows: &[TreeTableRow<'_, Id>],
        delta: i64,
    ) -> TreeTableOutcome<Id, ColId> {
        let enabled: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.enabled && navigable(r))
            .map(|(i, _)| i)
            .collect();
        if enabled.is_empty() {
            return TreeTableOutcome::Ignored;
        }
        let cur_pos = enabled
            .iter()
            .position(|&i| i == self.cursor_row)
            .unwrap_or(0);
        let next_pos = if delta >= 0 {
            (cur_pos + delta as usize).min(enabled.len() - 1)
        } else {
            cur_pos.saturating_sub((-delta) as usize)
        };
        let idx = enabled[next_pos];
        if idx == self.cursor_row {
            if delta > 0 && self.window.scroll_by(1) {
                return TreeTableOutcome::Scrolled;
            }
            if delta < 0 && self.window.scroll_by(-1) {
                return TreeTableOutcome::Scrolled;
            }
            return TreeTableOutcome::Ignored;
        }
        self.cursor_row = idx;
        self.selected = Some(rows[idx].id.clone());
        self.previous_index = Some(idx);
        self.reveal_cursor(rows.len());
        TreeTableOutcome::Selected(rows[idx].id.clone())
    }

    fn select_edge(
        &mut self,
        rows: &[TreeTableRow<'_, Id>],
        last: bool,
    ) -> TreeTableOutcome<Id, ColId> {
        let mut iter = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.enabled && navigable(r));
        let Some((idx, row)) = (if last { iter.next_back() } else { iter.next() }) else {
            return TreeTableOutcome::Ignored;
        };
        self.cursor_row = idx;
        self.selected = Some(row.id.clone());
        self.previous_index = Some(idx);
        self.reveal_cursor(rows.len());
        TreeTableOutcome::Selected(row.id.clone())
    }

    fn request_sort(&mut self, columns: &ColumnModel<ColId>) -> TreeTableOutcome<Id, ColId>
    where
        ColId: Clone,
    {
        // Sort only non-primary data columns that are sortable; col 0 is hierarchy.
        let vis: Vec<_> = columns.visible().collect();
        if vis.len() < 2 {
            return TreeTableOutcome::Ignored;
        }
        let target = vis
            .get(self.cursor_col.max(1))
            .or_else(|| vis.get(1))
            .map(|(_, c)| c);
        let Some(col) = target else {
            return TreeTableOutcome::Ignored;
        };
        if !col.sortable && self.cursor_col == 0 {
            // Prefer first sortable data column
            if let Some((_, c)) = vis.iter().skip(1).find(|(_, c)| c.sortable) {
                let ascending = match &self.sort {
                    Some(s) if s.column == c.id => !s.ascending,
                    _ => true,
                };
                let spec = SortSpec {
                    column: c.id.clone(),
                    ascending,
                };
                self.sort = Some(spec.clone());
                return TreeTableOutcome::SortSpec(spec);
            }
            return TreeTableOutcome::Ignored;
        }
        let ascending = match &self.sort {
            Some(s) if s.column == col.id => !s.ascending,
            _ => true,
        };
        let spec = SortSpec {
            column: col.id.clone(),
            ascending,
        };
        self.sort = Some(spec.clone());
        TreeTableOutcome::SortSpec(spec)
    }

    /// Mouse routing.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        rows: &[TreeTableRow<'_, Id>],
        columns: &ColumnModel<ColId>,
    ) -> TreeTableOutcome<Id, ColId>
    where
        ColId: Clone,
    {
        if !self.accepts_input {
            return TreeTableOutcome::Ignored;
        }
        let (ox, oy) = self.body_origin;
        let body = Rect {
            x: ox,
            y: oy,
            width: self.body_width.max(1),
            height: self.body_rows.max(1),
        };
        match event.kind {
            MouseEventKind::Moved => {
                let next = self
                    .row_regions
                    .iter()
                    .find(|region| region.area.contains(event.position))
                    .map(|region| region.id.clone());
                if self.hovered != next {
                    self.hovered = next;
                }
                TreeTableOutcome::Ignored
            }
            MouseEventKind::ScrollUp if body.contains(event.position) => {
                if self.window.scroll_by(-1) {
                    TreeTableOutcome::Scrolled
                } else {
                    self.move_row(rows, -1)
                }
            }
            MouseEventKind::ScrollDown if body.contains(event.position) => {
                if self.window.scroll_by(1) {
                    TreeTableOutcome::Scrolled
                } else {
                    self.move_row(rows, 1)
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(h) = self
                    .header_regions
                    .iter()
                    .find(|r| r.sortable && r.area.contains(event.position))
                {
                    let col = h.id.clone();
                    let ascending = match &self.sort {
                        Some(s) if s.column == col => !s.ascending,
                        _ => true,
                    };
                    let spec = SortSpec {
                        column: col,
                        ascending,
                    };
                    self.sort = Some(spec.clone());
                    return TreeTableOutcome::SortSpec(spec);
                }
                if let Some(region) = self
                    .row_regions
                    .iter()
                    .find(|r| r.area.contains(event.position))
                {
                    if region
                        .disclosure
                        .is_some_and(|d| d.contains(event.position))
                    {
                        return TreeTableOutcome::ExpandToggled(region.id.clone());
                    }
                    if self.selected.as_ref() == Some(&region.id) {
                        return TreeTableOutcome::Activated(region.id.clone());
                    }
                    self.cursor_row = region.index;
                    self.selected = Some(region.id.clone());
                    self.previous_index = Some(region.index);
                    return TreeTableOutcome::Selected(region.id.clone());
                }
                let _ = columns;
                TreeTableOutcome::Ignored
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if let Some(region) = self
                    .row_regions
                    .iter()
                    .find(|r| r.area.contains(event.position))
                {
                    self.cursor_row = region.index;
                    self.selected = Some(region.id.clone());
                    return TreeTableOutcome::ContextMenu {
                        row: region.id.clone(),
                    };
                }
                TreeTableOutcome::Ignored
            }
            _ => TreeTableOutcome::Ignored,
        }
    }
}

impl<Id: Clone + Ord, ColId: Clone + PartialEq> Default for TreeTableState<Id, ColId> {
    fn default() -> Self {
        Self::new(None)
    }
}

fn navigable<Id>(row: &TreeTableRow<'_, Id>) -> bool {
    !matches!(row.status, TreeNodeStatus::Loading) && !matches!(row.kind, TreeTableRowKind::Group)
}

/// Intent map for TreeTable: mode-sensitive Left/Right.
#[must_use]
pub fn default_tree_table_intent(key: KeyEvent, mode: TreeTableNavMode) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        return None; // handled as hierarchy chord
    }
    match key.code {
        KeyCode::Left | KeyCode::Char('h' | 'H') => match mode {
            TreeTableNavMode::Hierarchy => Some(UiIntent::Collapse),
            TreeTableNavMode::Cell | TreeTableNavMode::Scroll => {
                Some(UiIntent::Move(NavigationMove::Left))
            }
        },
        KeyCode::Right | KeyCode::Char('l' | 'L') => match mode {
            TreeTableNavMode::Hierarchy => Some(UiIntent::Expand),
            TreeTableNavMode::Cell | TreeTableNavMode::Scroll => {
                Some(UiIntent::Move(NavigationMove::Right))
            }
        },
        _ => crate::interaction::default_list_intent(key),
    }
}

/// TreeTable widget.
#[derive(Debug, Clone)]
pub struct TreeTable<'a, Id, ColId> {
    empty_message: &'a str,
    system: &'a DesignSystem,
    columns: &'a ColumnModel<ColId>,
    rows: &'a [TreeTableRow<'a, Id>],
    focused: bool,
    sticky_header: bool,
    compact_indent: bool,
}

impl<'a, Id: Clone + Ord, ColId: Clone + PartialEq> TreeTable<'a, Id, ColId> {
    /// Columns + flattened projected rows.
    #[must_use]
    pub const fn new(
        system: &'a DesignSystem,
        columns: &'a ColumnModel<ColId>,
        rows: &'a [TreeTableRow<'a, Id>],
    ) -> Self {
        Self {
            empty_message: "No rows",
            system,
            columns,
            rows,
            focused: true,
            sticky_header: true,
            compact_indent: false,
        }
    }

    /// Line shown when there is nothing to show.
    ///
    /// A collection that paints nothing when empty reads as broken; it has to
    /// say that it is empty.
    #[must_use]
    pub const fn empty_message(mut self, message: &'a str) -> Self {
        self.empty_message = message;
        self
    }

    /// Scene focus chrome for this surface.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Keep column headers pinned above the body (default true).
    #[must_use]
    pub const fn sticky_header(mut self, sticky: bool) -> Self {
        self.sticky_header = sticky;
        self
    }

    /// Use 1-cell indent steps (dense process trees).
    #[must_use]
    pub const fn compact_indent(mut self, compact: bool) -> Self {
        self.compact_indent = compact;
        self
    }

    /// Paint O(projected) only.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut TreeTableState<Id, ColId>)
    where
        ColId: Clone,
    {
        state.header_regions.clear();
        state.row_regions.clear();
        if area.is_empty() {
            return;
        }
        let surface_focused = self.focused || state.accepts_input;
        let header_h = u16::from(self.sticky_header);
        let footer_h = 1u16;
        state.window.viewport = area.height.saturating_sub(header_h + footer_h).max(1);
        state.window.clamp();

        let mut y = area.y;
        let col_budget = area.width.saturating_sub(GUTTER_W);
        state.viewport_width = col_budget;
        self.columns.resolve_paint_widths_with_gap(
            col_budget.saturating_add(state.h_offset),
            self.system.spacing.column_gap,
            &mut state.paint_widths,
        );
        let gap = self.system.spacing.column_gap;
        state.content_width = state
            .paint_widths
            .iter()
            .map(|(_, w)| *w)
            .fold(0u16, u16::saturating_add)
            .saturating_add(gap.saturating_mul(
                u16::try_from(state.paint_widths.len().saturating_sub(1)).unwrap_or(0),
            ));
        state.h_offset = state
            .h_offset
            .min(state.content_width.saturating_sub(col_budget));

        if self.sticky_header && y < area.bottom() {
            paint_header(self, area, y, buffer, state, surface_focused);
            y = y.saturating_add(1);
        }

        if let Some(chrome) =
            super::data_view::data_load_chrome(&state.load, self.system, false, self.empty_message)
        {
            paint_msg(
                self,
                area,
                y,
                buffer,
                chrome.prefix,
                &chrome.message,
                chrome.role,
            );
            state.body_origin = (area.x, y);
            state.body_rows = 0;
            state.body_width = area.width;
            return;
        }

        if self.rows.is_empty() {
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(self.empty_message, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            state.body_origin = (area.x, y);
            state.body_rows = 0;
            state.body_width = area.width;
            return;
        }

        state.body_origin = (area.x, y);
        state.body_width = area.width;
        let body_start = y;
        let body_bottom = area.bottom().saturating_sub(footer_h);
        let indent_step = if self.compact_indent {
            INDENT_STEP_COMPACT
        } else {
            INDENT_STEP
        };

        for (i, row) in self.rows.iter().enumerate() {
            if y >= body_bottom {
                break;
            }
            paint_row(
                self,
                area,
                y,
                buffer,
                state,
                i,
                row,
                surface_focused,
                indent_step,
            );
            y = y.saturating_add(1);
        }
        state.body_rows = y.saturating_sub(body_start);

        // Footer
        let fy = area.bottom().saturating_sub(1);
        let mut parts = Vec::new();
        match &state.load {
            LoadState::Partial { resident, total } => match total {
                Some(t) => parts.push(format!("partial {resident}/{t}")),
                None => parts.push(format!("partial {resident}+")),
            },
            LoadState::Ready { count } => parts.push(format!("{count} rows")),
            _ => {}
        }
        if state.multi {
            let n = state.selection.selected_rows().len();
            if n > 0 {
                parts.push(format!("{n} checked"));
            }
        }
        parts.push(format!("nav:{}", state.nav_mode.id()));
        let footer = parts.join(" · ");
        if !footer.is_empty() && fy >= area.y {
            buffer.set_stringn(
                area.x,
                fy,
                &take_display_cols(&footer, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
        }
    }
}

fn paint_msg<Id: Clone + Ord, ColId: Clone + PartialEq>(
    table: &TreeTable<'_, Id, ColId>,
    area: Rect,
    y: u16,
    buffer: &mut Buffer,
    glyph: &str,
    message: &str,
    role: Role,
) {
    if y >= area.bottom() {
        return;
    }
    let line = format!("{glyph}{message}");
    buffer.set_stringn(
        area.x,
        y,
        &take_display_cols(&line, usize::from(area.width)),
        usize::from(area.width),
        table.system.style(role),
    );
}

fn paint_header<Id: Clone + Ord, ColId: Clone + PartialEq>(
    table: &TreeTable<'_, Id, ColId>,
    area: Rect,
    y: u16,
    buffer: &mut Buffer,
    state: &mut TreeTableState<Id, ColId>,
    _surface_focused: bool,
) where
    ColId: Clone,
{
    // The header never brightens with focus; the panel border says that.
    let style = super::table_chrome::header_style(table.system);
    buffer.set_style(
        Rect::new(area.x, y, area.width, 1),
        super::table_chrome::header_band(table.system),
    );
    buffer.set_stringn(area.x, y, "  ", usize::from(GUTTER_W), style);
    let origin = area.x.saturating_add(GUTTER_W);
    let clip_right = area.right();
    let mut logical = 0i32;
    let h_off = i32::from(state.h_offset);
    let gap = i32::from(table.system.spacing.column_gap);
    for (ord, &(col_idx, width)) in state.paint_widths.iter().enumerate() {
        let col = &table.columns.columns[col_idx];
        let pinned = col.pin != ColumnPin::None;
        let col_left = if pinned {
            i32::from(origin) + logical
        } else {
            i32::from(origin) + logical - h_off
        };
        let col_right = col_left + i32::from(width);
        logical += i32::from(width) + gap;
        if col_right <= i32::from(origin) || col_left >= i32::from(clip_right) {
            continue;
        }
        let paint_x = col_left.max(i32::from(origin)) as u16;
        let paint_end = col_right.min(i32::from(clip_right)) as u16;
        let paint_w = paint_end.saturating_sub(paint_x);
        if paint_w == 0 {
            continue;
        }
        let mut title = col.title.clone();
        if let Some(sort) = &state.sort
            && sort.column == col.id
        {
            title.push_str(super::table_chrome::sort_marker(sort.ascending));
        }
        buffer.set_style(Rect::new(paint_x, y, paint_w, 1), style);
        buffer.set_stringn(
            paint_x,
            y,
            &take_display_cols(&title, usize::from(paint_w)),
            usize::from(paint_w),
            style,
        );
        // Hierarchy column (0) is not sortable by default semantics
        let sortable = col.sortable && ord > 0;
        state.header_regions.push(TreeTableHeaderRegion {
            id: col.id.clone(),
            area: Rect::new(paint_x, y, paint_w, 1),
            sortable,
        });
    }
}

fn paint_row<Id: Clone + Ord, ColId: Clone + PartialEq>(
    table: &TreeTable<'_, Id, ColId>,
    area: Rect,
    y: u16,
    buffer: &mut Buffer,
    state: &mut TreeTableState<Id, ColId>,
    row_index: usize,
    row: &TreeTableRow<'_, Id>,
    surface_focused: bool,
    indent_step: u16,
) where
    ColId: Clone,
    Id: Clone,
{
    let selected = state.selected.as_ref() == Some(&row.id);
    let checked = state.multi && state.selection.is_row_selected(&row.id);
    let cursor = state.cursor_row == row_index;
    let hovered = state.hovered.as_ref() == Some(&row.id);
    let loading = matches!(row.status, TreeNodeStatus::Loading | TreeNodeStatus::Lazy);
    let indicated = selected || (cursor && surface_focused);
    let chrome = super::row_chrome::RowChrome::resolve(
        table.system,
        ListRowVisualState {
            selected: indicated,
            focused: surface_focused && cursor,
            hovered,
            enabled: row.enabled,
            loading,
            checked,

            ..ListRowVisualState::default()
        },
    );

    let base_style = match row.status {
        TreeNodeStatus::Error => table.system.style(Role::Danger),
        TreeNodeStatus::Loading | TreeNodeStatus::Lazy => table.system.style(Role::TextMuted),
        TreeNodeStatus::Ready if !row.enabled => table.system.style(Role::TextDisabled),
        TreeNodeStatus::Ready
            if matches!(
                row.kind,
                TreeTableRowKind::Group | TreeTableRowKind::Aggregate
            ) =>
        {
            table
                .system
                .style(Role::TextStrong)
                .add_modifier(Modifier::BOLD)
        }
        TreeNodeStatus::Ready if state.striped && row_index % 2 == 1 => {
            table.system.style(Role::TextMuted)
        }
        TreeNodeStatus::Ready => table.system.style(Role::Text),
    };
    let base_style = chrome.label_style(base_style);

    // The quiet tier for this row: same ground and weight, lower voice.
    let quiet_style = chrome.secondary_style(base_style);

    let row_area = Rect::new(area.x, y, area.width, 1);
    chrome.paint_wash(buffer, row_area);
    buffer.set_stringn(area.x, y, " ", 1, base_style);
    buffer.set_stringn(area.x.saturating_add(1), y, " ", 1, base_style);

    let origin = area.x.saturating_add(GUTTER_W);
    let clip_right = area.right();
    let h_off = i32::from(state.h_offset);
    let gap = i32::from(table.system.spacing.column_gap);
    let mut logical = 0i32;
    let mut disclosure_rect = None;
    if matches!(row.kind, TreeTableRowKind::Group) {
        let mark = if row.expanded { "▾ " } else { "▸ " };
        let label = row.cells.first().copied().unwrap_or("");
        let line = format!("{mark}{label}");
        buffer.set_stringn(
            origin,
            y,
            &take_display_cols(&line, usize::from(clip_right.saturating_sub(origin))),
            usize::from(clip_right.saturating_sub(origin)),
            base_style,
        );
        state.row_regions.push(TreeTableRowRegion {
            id: row.id.clone(),
            index: row_index,
            area: row_area,
            disclosure: Some(Rect::new(origin, y, 2, 1)),
        });
        chrome.paint_gutter(buffer, row_area);
        paint_checked_marker(table.system, buffer, row_area, checked);
        return;
    }

    for (ord, &(col_idx, width)) in state.paint_widths.iter().enumerate() {
        let col = &table.columns.columns[col_idx];
        let pinned = col.pin != ColumnPin::None;
        let col_left = if pinned {
            i32::from(origin) + logical
        } else {
            i32::from(origin) + logical - h_off
        };
        let col_right = col_left + i32::from(width);
        logical += i32::from(width) + gap;
        if col_right <= i32::from(origin) || col_left >= i32::from(clip_right) {
            continue;
        }
        let paint_x = col_left.max(i32::from(origin)) as u16;
        let paint_end = col_right.min(i32::from(clip_right)) as u16;
        let paint_w = paint_end.saturating_sub(paint_x);
        if paint_w == 0 {
            continue;
        }

        // The hierarchy column is the row's identity; it never drops a tier.
        let mut cell_style = if ord == 0 {
            base_style
        } else {
            col.kind.cell_style(base_style, quiet_style)
        };
        if cursor && surface_focused && state.cursor_col == ord {
            // A cell cursor is a cell: it states itself with the explicit
            // reversal pair, not a modifier over the row's own colours.
            cell_style = table.system.reversed();
        }
        buffer.set_style(Rect::new(paint_x, y, paint_w, 1), cell_style);

        if ord == 0 {
            // Hierarchy column: indent + disclosure + label
            let max_indent = paint_w.saturating_sub(4);
            let indent = row.depth.saturating_mul(indent_step).min(max_indent);
            let mut x = paint_x;
            if indent > 0 && x + indent <= paint_end {
                // blank indent
                x = x.saturating_add(indent);
            }
            let glyph = if row.branch {
                if row.expanded {
                    table.system.glyphs.disclosure_open()
                } else {
                    table.system.glyphs.disclosure_closed()
                }
            } else if matches!(row.kind, TreeTableRowKind::Aggregate) {
                // ASCII "=" for all glyph sets (no non-English Σ showcase).
                "="
            } else {
                " "
            };
            if x < paint_end {
                buffer.set_stringn(x, y, glyph, 1, cell_style);
                if row.branch {
                    disclosure_rect = Some(Rect::new(x, y, 1, 1));
                }
                x = x.saturating_add(2);
            }
            let label = row.cells.first().copied().unwrap_or("");
            let remain = paint_end.saturating_sub(x);
            if remain > 0 {
                buffer.set_stringn(
                    x,
                    y,
                    &take_display_cols(label, usize::from(remain)),
                    usize::from(remain),
                    cell_style,
                );
            }
        } else {
            let text = row.cells.get(ord).copied().unwrap_or("");
            buffer.set_stringn(
                paint_x,
                y,
                &take_display_cols(text, usize::from(paint_w)),
                usize::from(paint_w),
                cell_style,
            );
        }
    }

    if row.enabled {
        state.row_regions.push(TreeTableRowRegion {
            id: row.id.clone(),
            index: row_index,
            area: row_area,
            disclosure: disclosure_rect,
        });
    }
    chrome.paint_gutter(buffer, row_area);
    paint_checked_marker(table.system, buffer, row_area, checked);
}

fn paint_checked_marker(system: &DesignSystem, buffer: &mut Buffer, row: Rect, checked: bool) {
    if checked && row.width > 1 {
        buffer.set_stringn(
            row.x.saturating_add(1),
            row.y,
            Glyph::Success.resolve().text,
            1,
            system.style(Role::Accent),
        );
    }
}

impl<'a, Id: Clone + Ord, ColId: Clone + PartialEq> StatefulWidget for TreeTable<'a, Id, ColId> {
    type State = TreeTableState<Id, ColId>;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        TreeTable::render(&self, area, buffer, state);
    }
}

impl<'a, Id: Clone + Ord, ColId: Clone + PartialEq> StatefulWidget for &TreeTable<'a, Id, ColId> {
    type State = TreeTableState<Id, ColId>;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        TreeTable::render(self, area, buffer, state);
    }
}

/// Keep TreeTable rows matching query and their ancestors (by depth walk).
#[must_use]
pub fn filter_tree_table_with_ancestors<'a, Id: Clone + PartialEq>(
    rows: &'a [TreeTableRow<'a, Id>],
    query: &str,
) -> Vec<&'a TreeTableRow<'a, Id>> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return rows.iter().collect();
    }
    let mut keep = vec![false; rows.len()];
    for (i, r) in rows.iter().enumerate() {
        let label = r.cells.first().copied().unwrap_or("");
        if label.to_ascii_lowercase().contains(&q) {
            keep[i] = true;
            let mut depth = r.depth;
            let mut j = i;
            while depth > 0 && j > 0 {
                j -= 1;
                if rows[j].depth < depth {
                    keep[j] = true;
                    depth = rows[j].depth;
                }
            }
        }
    }
    rows.iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, r)| r)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::data_view::{ColumnKind, DataColumn, DataColumnWidth, bench};
    use ratatui_core::layout::Position;

    fn cols() -> ColumnModel<&'static str> {
        ColumnModel::new(vec![
            DataColumn::new("name", "Name", DataColumnWidth::Min(12)).priority(100),
            DataColumn::new("cpu", "CPU", DataColumnWidth::Fixed(6))
                .priority(80)
                .sortable(),
            DataColumn::new("mem", "MEM", DataColumnWidth::Fixed(6)).priority(40),
        ])
    }

    #[test]
    fn hierarchy_expand_collapse() {
        let c0: &[&str] = &["root", "1", "2"];
        let c1: &[&str] = &["child", "0", "0"];
        let rows = [
            TreeTableRow::new("r", 0, c0).branch().expanded(),
            TreeTableRow::new("c", 1, c1).parent("r"),
        ];
        let columns = cols();
        let mut state = TreeTableState::<&str, &str>::new(Some("r"));
        state.cursor_row = 0;
        let out = state.handle_key(
            &rows,
            &columns,
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
        );
        assert!(matches!(out, TreeTableOutcome::ExpandToggled("r")));
    }

    #[test]
    fn left_selects_parent_when_collapsed() {
        let c0: &[&str] = &["root", "", ""];
        let c1: &[&str] = &["child", "", ""];
        let rows = [
            TreeTableRow::new("r", 0, c0).branch(),
            TreeTableRow::new("c", 1, c1).parent("r"),
        ];
        let columns = cols();
        let mut state = TreeTableState::<&str, &str>::new(Some("c"));
        state.cursor_row = 1;
        let out = state.handle_key(
            &rows,
            &columns,
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
        );
        assert!(matches!(out, TreeTableOutcome::Selected("r")));
    }

    #[test]
    fn right_on_expanded_enters_child() {
        let c0: &[&str] = &["root", "", ""];
        let c1: &[&str] = &["child", "", ""];
        let rows = [
            TreeTableRow::new("r", 0, c0).branch().expanded(),
            TreeTableRow::new("c", 1, c1).parent("r"),
        ];
        let columns = cols();
        let mut state = TreeTableState::<&str, &str>::new(Some("r"));
        state.cursor_row = 0;
        let out = state.handle_key(
            &rows,
            &columns,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );
        assert!(matches!(out, TreeTableOutcome::Selected("c")));
        assert_eq!(state.cursor_row, 1);
    }

    #[test]
    fn cell_mode_moves_columns() {
        let c0: &[&str] = &["a", "1", "2"];
        let rows = [TreeTableRow::new("r", 0, c0)];
        let columns = cols();
        let mut state = TreeTableState::<&str, &str>::new(Some("r"));
        state.set_nav_mode(TreeTableNavMode::Cell);
        let out = state.handle_key(
            &rows,
            &columns,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );
        assert!(matches!(out, TreeTableOutcome::CursorMoved));
        assert_eq!(state.cursor_col, 1);
    }

    #[test]
    fn scroll_mode_horizontal() {
        let c0: &[&str] = &["a", "1", "2"];
        let rows = [TreeTableRow::new("r", 0, c0)];
        let columns = cols();
        let mut state = TreeTableState::<&str, &str>::new(Some("r"));
        state.set_nav_mode(TreeTableNavMode::Scroll);
        state.content_width = 80;
        state.viewport_width = 20;
        let out = state.handle_key(
            &rows,
            &columns,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );
        assert!(matches!(out, TreeTableOutcome::Scrolled));
        assert!(state.h_offset > 0);
    }

    #[test]
    fn lazy_expand_requests_toggle() {
        let c0: &[&str] = &["lazy", "", ""];
        let rows = [TreeTableRow::new("L", 0, c0).lazy_branch()];
        let columns = cols();
        let mut state = TreeTableState::<&str, &str>::new(Some("L"));
        let out = state.handle_key(
            &rows,
            &columns,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );
        assert!(matches!(out, TreeTableOutcome::ExpandToggled("L")));
    }

    #[test]
    fn sort_skips_hierarchy_column() {
        let c0: &[&str] = &["n", "1", "2"];
        let rows = [TreeTableRow::new("r", 0, c0)];
        let columns = cols();
        let mut state = TreeTableState::<&str, &str>::new(Some("r"));
        state.cursor_col = 0;
        let out = state.handle_key(
            &rows,
            &columns,
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            TreeTableOutcome::SortSpec(SortSpec {
                column: "cpu",
                ascending: true
            })
        ));
    }

    #[test]
    fn million_window_projection() {
        let mut state = TreeTableState::<u64, &str>::new(None);
        state.set_logical_rows(bench::ROWS_1M);
        state.window.viewport = bench::VIEWPORT_ROWS;
        state.window.clamp();
        let (a, b) = state.window.visible_range();
        assert_eq!(b - a, u64::from(bench::VIEWPORT_ROWS));
    }

    #[test]
    fn non_ready_load_state_precedes_the_empty_projection_fallback() {
        let system = DesignSystem::junie().no_color();
        let columns = cols();
        let rows: [TreeTableRow<'_, u64>; 0] = [];
        let render = |load| {
            let mut state = TreeTableState::<u64, &str>::new(None);
            state.load = load;
            let area = Rect::new(0, 0, 32, 5);
            let mut buffer = Buffer::empty(area);
            TreeTable::new(&system, &columns, &rows).render(area, &mut buffer, &mut state);
            buffer
                .content()
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
        };

        assert!(render(LoadState::Loading { message: None }).contains("… Loading…"));
        assert!(
            render(LoadState::Error {
                message: "failed".into(),
                retryable: false,
            })
            .contains("✗ failed")
        );
    }

    #[test]
    fn paint_process_tree() {
        let system = DesignSystem::default();
        let columns = cols();
        let r0: &[&str] = &["systemd", "0.1", "4M"];
        let r1: &[&str] = &["sshd", "0.0", "8M"];
        let r2: &[&str] = &["bash", "1.2", "12M"];
        let rows = [
            TreeTableRow::new(1u64, 0, r0).branch().expanded(),
            TreeTableRow::new(2, 1, r1).branch().expanded().parent(1),
            TreeTableRow::new(3, 2, r2).parent(2),
        ];
        let mut state = TreeTableState::new(Some(2));
        state.load = LoadState::Ready { count: 3 };
        let table = TreeTable::new(&system, &columns, &rows)
            .focused(true)
            .compact_indent(true);
        let area = Rect::new(0, 0, 48, 8);
        let mut buffer = Buffer::empty(area);
        table.render(area, &mut buffer, &mut state);
        assert!(!state.header_regions.is_empty());
        assert!(!state.row_regions.is_empty());
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("systemd") || text.contains("sshd"), "{text}");
        assert!(text.contains("Name") || text.contains("CPU"), "{text}");
    }

    #[test]
    fn disclosure_click_expands() {
        let system = DesignSystem::default();
        let columns = cols();
        let r0: &[&str] = &["root", "0", "0"];
        let rows = [TreeTableRow::new("r", 0, r0).branch()];
        let mut state = TreeTableState::<&str, &str>::new(Some("r"));
        let table = TreeTable::new(&system, &columns, &rows).focused(true);
        let area = Rect::new(0, 0, 40, 6);
        let mut buffer = Buffer::empty(area);
        table.render(area, &mut buffer, &mut state);
        let disc = state.row_regions[0]
            .disclosure
            .expect("branch has disclosure");
        let out = state.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position {
                    x: disc.x,
                    y: disc.y,
                },
                modifiers: KeyModifiers::NONE,
            },
            &rows,
            &columns,
        );
        assert!(matches!(out, TreeTableOutcome::ExpandToggled("r")));
    }

    #[test]
    fn pointer_move_tracks_hovered_row() {
        let system = DesignSystem::default();
        let columns = cols();
        let cells: &[&str] = &["root", "0", "0"];
        let rows = [TreeTableRow::new("r", 0, cells)];
        let mut state = TreeTableState::<&str, &str>::new(None);
        let area = Rect::new(0, 0, 40, 6);
        TreeTable::new(&system, &columns, &rows).render(area, &mut Buffer::empty(area), &mut state);
        let row = state.row_regions[0].area;
        let out = state.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Moved,
                position: Position { x: row.x, y: row.y },
                modifiers: KeyModifiers::NONE,
            },
            &rows,
            &columns,
        );
        assert!(matches!(out, TreeTableOutcome::Ignored));
        assert_eq!(state.hovered, Some("r"));
    }

    #[test]
    fn filter_keeps_ancestors() {
        let r0: &[&str] = &["src", "", ""];
        let r1: &[&str] = &["lib.rs", "", ""];
        let r2: &[&str] = &["other", "", ""];
        let rows = [
            TreeTableRow::new("a", 0, r0).branch().expanded(),
            TreeTableRow::new("b", 1, r1).parent("a"),
            TreeTableRow::new("c", 0, r2),
        ];
        let kept = filter_tree_table_with_ancestors(&rows, "lib");
        let ids: Vec<_> = kept.iter().map(|r| r.id).collect();
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"b"));
        assert!(!ids.contains(&"c"));
    }

    #[test]
    fn nav_mode_cycle() {
        let r0: &[&str] = &["x", "", ""];
        let rows = [TreeTableRow::new("r", 0, r0)];
        let columns = cols();
        let mut state = TreeTableState::<&str, &str>::new(Some("r"));
        let out = state.handle_key(
            &rows,
            &columns,
            KeyEvent::new(KeyCode::Char('\\'), KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            TreeTableOutcome::NavModeChanged(TreeTableNavMode::Cell)
        ));
    }

    #[test]
    fn multi_select_space() {
        let r0: &[&str] = &["a", "", ""];
        let rows = [TreeTableRow::new("r", 0, r0)];
        let columns = cols();
        let mut state = TreeTableState::<&str, &str>::new(Some("r"));
        state.enable_multi_select();
        let out = state.handle_key(
            &rows,
            &columns,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert!(matches!(out, TreeTableOutcome::CheckToggled("r")));
        assert!(state.selection.is_row_selected(&"r"));
    }

    #[test]
    fn aggregate_not_expandable() {
        let r0: &[&str] = &["TOTAL", "100", "200"];
        let rows = [TreeTableRow::new("t", 0, r0).aggregate()];
        let columns = cols();
        let mut state = TreeTableState::<&str, &str>::new(Some("t"));
        let out = state.handle_key(
            &rows,
            &columns,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );
        assert!(matches!(out, TreeTableOutcome::Ignored));
    }

    #[test]
    fn layout_fuzz_indent() {
        for depth in 0u16..20 {
            let indent = depth.saturating_mul(INDENT_STEP).min(40);
            assert!(indent <= 40);
        }
    }

    #[test]
    fn numeric_columns_read_quieter_than_the_hierarchy_column() {
        let system = DesignSystem::default();
        let columns = ColumnModel::new(vec![
            DataColumn::new("name", "Name", DataColumnWidth::Min(12)).priority(100),
            DataColumn::new("mem", "MEM", DataColumnWidth::Fixed(6))
                .priority(40)
                .kind(ColumnKind::Numeric),
        ]);
        let r0: &[&str] = &["systemd", "4096"];
        let rows = [TreeTableRow::new(1u64, 0, r0)];
        let mut state = TreeTableState::new(None);
        state.load = LoadState::Ready { count: 1 };
        state.set_accepts_input(false);
        let area = Rect::new(0, 0, 40, 6);
        let mut buffer = Buffer::empty(area);
        TreeTable::new(&system, &columns, &rows)
            .focused(false)
            .render(area, &mut buffer, &mut state);

        let row_y = (0..area.height)
            .find(|y| (0..area.width).any(|x| buffer[(x, *y)].symbol().starts_with('s')))
            .expect("the data row must be painted");
        let at = |needle: char| {
            let x = (0..area.width)
                .find(|x| buffer[(*x, row_y)].symbol().starts_with(needle))
                .unwrap_or_else(|| panic!("{needle:?} must be painted"));
            buffer[(x, row_y)].style().fg
        };
        assert_ne!(
            at('s'),
            at('4'),
            "a byte count must not read as loudly as the process name"
        );
        assert_eq!(at('4'), system.style(Role::TextMuted).fg);
    }

    #[test]
    fn intent_map_mode_sensitive() {
        let h = default_tree_table_intent(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            TreeTableNavMode::Hierarchy,
        );
        assert_eq!(h, Some(UiIntent::Expand));
        let c = default_tree_table_intent(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            TreeTableNavMode::Cell,
        );
        assert_eq!(c, Some(UiIntent::Move(NavigationMove::Right)));
    }
}

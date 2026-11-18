use std::cell::{Ref, RefCell};
use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};

use crate::core::edit::{EditEntry, EditKind, FieldKeys, TextCmd};
use crate::core::event::{Key, Outcome};
use crate::core::focus::request_focus_next;
use crate::core::hit::Axis;
use crate::core::id::WidgetId;
use crate::core::keymap::{Actions, KeyBinding, KeyMap, Nav, NavKeys, lookup};
use crate::core::scroll::{HScroll, ScrollPane, ScrollState, Scrollable};
use crate::theme::SyntaxTone;
use crate::ui::ctx::{RenderCtx, fill};
use crate::ui::edit_menu::{
    EditCaps, EditMenu, EditMenuHost, edit_menu_key, edit_menu_pointer, edit_menu_route,
    point_editable,
};
use crate::ui::text::{find_ranges, width};
use crate::widgets::edit::{EditField, Editable, LineLook};
use crate::widgets::field_common::{EditAction, PointerField};
use crate::widgets::scrollbar;

mod index;
use crate::core::hints::{HasHints, HintCx};
use crate::widgets::keyhint::Hint;
use index::{
    BracketMap, DiagnosticIndex, ProviderKey, RunIndex, SegmentSnapshot, SyntaxCache,
    next_generation,
};

pub type Highlighter = fn(&str) -> Vec<(Range<usize>, SyntaxTone)>;
pub type Segmenter = fn(&str) -> Vec<Range<usize>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub range: Range<usize>,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct FindState {
    pub needle: EditField,

    pub matches: Vec<Range<usize>>,
    pub current: usize,
    pub editing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorAction {
    Changed,
    CursorMoved,

    Committed,

    Leave { backward: bool },
}

#[derive(Debug, Clone)]
pub struct CodeEditor {
    pub id: WidgetId,

    pub field: EditField,
    pub editing: bool,
    pub read_only: bool,

    pub scroll: ScrollPane,
    pub hscroll: usize,
    pub indent: usize,
    highlighter: Option<Highlighter>,
    segmenter: Option<Segmenter>,
    diagnostics: Vec<Diagnostic>,

    pub running: Option<Range<usize>>,
    find: Option<FindState>,
    pub placeholder: String,

    pub tab_leaves: bool,
    pub area: Rect,
    text_area: Rect,
    gutter_w: u16,

    highlighter_generation: u64,
    segmenter_generation: u64,
    diagnostics_revision: u64,
    find_revision: u64,
    find_scanned: Option<(u64, String)>,
    syntax: Option<SyntaxCache>,
    diagnostic_index: Option<DiagnosticIndex>,
    find_runs: Option<(u64, u64, RunIndex)>,
    brackets: Option<BracketMap>,
    segment_cache: RefCell<Option<SegmentSnapshot>>,

    rendered_state: Option<(usize, u64, bool)>,

    pub scrollbar_gap: u16,

    pub horizontal: Option<HScroll>,

    pub nav: NavKeys,

    pub keymap: KeyMap<CodeCmd>,

    pub edit_entry: Option<EditEntry>,
    entry: EditEntry,

    pub esc_reverts: bool,

    pub field_keys: Option<FieldKeys>,

    snapshot: Option<String>,
    focus_seen: bool,

    pub edit_menu: EditMenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeCmd {
    Edit,

    Append,

    PrevBlock,

    NextBlock,

    Find,

    NextMatch,

    PrevMatch,
}

impl Actions for CodeCmd {
    fn label(self) -> &'static str {
        match self {
            CodeCmd::Edit => "Edit",
            CodeCmd::Append => "Edit at end",
            CodeCmd::PrevBlock => "Previous block",
            CodeCmd::NextBlock => "Next block",
            CodeCmd::Find => "Find",
            CodeCmd::NextMatch => "Next match",
            CodeCmd::PrevMatch => "Previous match",
        }
    }

    fn default_keymap() -> KeyMap<Self> {
        vec![
            (KeyBinding::key(KeyCode::Enter), CodeCmd::Edit),
            (KeyBinding::char('i'), CodeCmd::Edit),
            (KeyBinding::char('a'), CodeCmd::Append),
            (KeyBinding::char('{'), CodeCmd::PrevBlock),
            (KeyBinding::char('}'), CodeCmd::NextBlock),
            (KeyBinding::char('/'), CodeCmd::Find),
            (KeyBinding::char('n'), CodeCmd::NextMatch),
            (KeyBinding::char('N'), CodeCmd::PrevMatch),
        ]
    }
}

impl CodeEditor {
    pub fn new(id: WidgetId, text: &str) -> Self {
        let mut field = EditField::multi(text);
        field.buffer.move_doc_start(false);
        Self {
            id,
            field,
            editing: false,
            read_only: false,
            scroll: ScrollPane::default(),
            hscroll: 0,
            indent: 2,
            highlighter: None,
            segmenter: None,
            diagnostics: vec![],
            running: None,
            find: None,
            placeholder: String::new(),
            tab_leaves: false,
            area: Rect::ZERO,
            text_area: Rect::ZERO,
            gutter_w: 0,
            highlighter_generation: 0,
            segmenter_generation: 0,
            diagnostics_revision: 0,
            find_revision: 0,
            find_scanned: None,
            syntax: None,
            diagnostic_index: None,
            find_runs: None,
            brackets: None,
            segment_cache: RefCell::new(None),
            rendered_state: None,
            scrollbar_gap: scrollbar::GAP,
            horizontal: None,
            nav: NavKeys::default(),
            keymap: CodeCmd::default_keymap(),
            edit_entry: None,
            entry: crate::theme::Theme::DEFAULT_EDIT_ENTRY,
            esc_reverts: false,
            field_keys: None,
            snapshot: None,
            focus_seen: false,
            edit_menu: EditMenu::default(),
        }
    }

    pub fn context_menu(mut self, on: bool) -> Self {
        self.edit_menu.enabled = on;
        self
    }

    pub fn edit_entry(mut self, entry: EditEntry) -> Self {
        self.edit_entry = Some(entry);
        self.entry = entry;
        self
    }

    pub fn esc_reverts(mut self, on: bool) -> Self {
        self.esc_reverts = on;
        self
    }

    pub fn tab_leaves(mut self, on: bool) -> Self {
        self.tab_leaves = on;
        self
    }

    pub fn text_keymap(
        mut self,
        keymap: crate::core::keymap::KeyMap<crate::core::edit::TextCmd>,
    ) -> Self {
        self.field.text_keymap = Some(keymap);
        self
    }

    fn edit_action(&self, key: &Key) -> EditAction {
        self.field.action(key)
    }

    pub fn field_keys(mut self, keys: FieldKeys) -> Self {
        self.field_keys = Some(keys);
        self.field.keys = keys;
        self
    }

    pub fn entry(&self) -> EditEntry {
        self.edit_entry.unwrap_or(self.entry)
    }

    pub fn hints(&self) -> Vec<crate::widgets::keyhint::Hint> {
        if let Some(m) = self.edit_menu.menu() {
            return crate::core::hints::HasHints::hints(m, &crate::core::hints::HintCx::new(None));
        }
        use crate::widgets::keyhint::{EDIT_CODE, hint};
        let esc = if self.esc_reverts { "Cancel" } else { "Done" };
        if self.find.as_ref().is_some_and(|f| f.editing) {
            vec![hint("Enter", "Find"), hint("Esc", "Close")]
        } else if self.editing && self.tab_leaves {
            vec![hint("Tab", "Next field"), hint("Esc", esc)]
        } else if self.editing && !self.esc_reverts {
            EDIT_CODE.to_vec()
        } else if self.editing {
            vec![hint("Tab", "Indent"), hint("F6", "Leave"), hint("Esc", esc)]
        } else if self.read_only {
            vec![hint("Tab", "Next")]
        } else {
            vec![crate::widgets::widget_hints::EDIT, hint("Tab", "Next")]
        }
    }

    pub fn wants_text_input(&self) -> bool {
        self.find.as_ref().is_some_and(|f| f.editing) || (self.editing && !self.read_only)
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn keymap(mut self, keymap: KeyMap<CodeCmd>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn scrollbar_gap(mut self, gap: u16) -> Self {
        self.scrollbar_gap = gap;
        self
    }

    pub fn horizontal(mut self, policy: HScroll) -> Self {
        self.horizontal = Some(policy);
        self
    }

    pub fn hbar(&self) -> &ScrollState {
        &self.scroll.h
    }

    fn scroll_h(&mut self, cells: isize) -> bool {
        let before = self.hscroll;
        let max = if self.scroll.h.viewport_len > 0 {
            self.scroll.h.max_offset()
        } else {
            usize::MAX
        };
        self.hscroll = self.hscroll.saturating_add_signed(cells).min(max);
        self.scroll.h.offset = self.hscroll;
        self.hscroll != before
    }

    pub fn highlighter(mut self, h: Highlighter) -> Self {
        self.set_highlighter(Some(h));
        self
    }
    pub fn segmenter(mut self, s: Segmenter) -> Self {
        self.set_segmenter(Some(s));
        self
    }

    pub fn set_highlighter(&mut self, value: Option<Highlighter>) {
        self.highlighter = value;
        self.invalidate_highlighter();
    }

    pub fn invalidate_highlighter(&mut self) {
        self.highlighter_generation = next_generation(self.highlighter_generation);
        self.syntax = None;
    }

    pub fn highlighter_fn(&self) -> Option<Highlighter> {
        self.highlighter
    }

    pub fn set_segmenter(&mut self, value: Option<Segmenter>) {
        self.segmenter = value;
        self.invalidate_segmenter();
    }

    pub fn invalidate_segmenter(&mut self) {
        self.segmenter_generation = next_generation(self.segmenter_generation);
        *self.segment_cache.get_mut() = None;
    }

    pub fn segmenter_fn(&self) -> Option<Segmenter> {
        self.segmenter
    }

    pub fn set_diagnostics(&mut self, value: Vec<Diagnostic>) {
        self.diagnostics = value;
        self.diagnostics_revision = next_generation(self.diagnostics_revision);
        self.diagnostic_index = None;
    }

    pub fn push_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
        self.diagnostics_revision = next_generation(self.diagnostics_revision);
        self.diagnostic_index = None;
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn find_state(&self) -> Option<&FindState> {
        self.find.as_ref()
    }

    pub fn set_find_query(&mut self, text: &str) {
        if self.find.is_none() {
            self.open_find();
            if let Some(f) = self.find.as_mut() {
                f.editing = false;
            }
        }
        if let Some(f) = self.find.as_mut() {
            f.needle.set_text(text);
        }
        self.refind();
    }

    pub fn set_find_current(&mut self, index: usize) {
        if let Some(f) = self.find.as_mut() {
            f.current = index.min(f.matches.len().saturating_sub(1));
        }
    }
    pub fn read_only(mut self, ro: bool) -> Self {
        self.read_only = ro;
        self
    }
    pub fn placeholder(mut self, p: &str) -> Self {
        self.placeholder = p.to_owned();
        self
    }

    pub fn text(&self) -> &str {
        self.field.buffer.text()
    }

    pub fn set_text(&mut self, text: &str) {
        self.field.set_text(text);
        self.field.buffer.move_doc_start(false);
        self.set_diagnostics(Vec::new());
        self.hscroll = 0;
        self.scroll.jump_start();
        self.refind();
    }

    pub fn is_empty(&self) -> bool {
        self.field.buffer.is_empty()
    }

    fn cached_segments(&self) -> Ref<'_, SegmentSnapshot> {
        let key = ProviderKey {
            text: self.field.buffer.revision(),
            provider: self.segmenter_generation,
        };
        let stale = self.segment_cache.borrow().as_ref().map(|s| s.key) != Some(key);
        if stale {
            let blocks = self
                .segmenter
                .map(|f| {
                    crate::record_work!(AnnotationQueries, 1usize);
                    f(self.field.buffer.text())
                })
                .unwrap_or_default();
            let snapshot = SegmentSnapshot::build(key, blocks);
            *self.segment_cache.borrow_mut() = Some(snapshot);
        }
        Ref::map(self.segment_cache.borrow(), |slot| {
            slot.as_ref().expect("segment snapshot installed")
        })
    }

    pub fn current_block(&self) -> Option<Range<usize>> {
        self.cached_segments()
            .block_at(self.field.buffer.cursor_offset())
    }

    pub fn selection_or_block(&self) -> Option<(String, Range<usize>)> {
        if let Some(r) = self.field.buffer.selection() {
            return Some((self.field.buffer.text()[r.clone()].to_owned(), r));
        }
        let b = self.current_block()?;
        Some((self.field.buffer.text()[b.clone()].to_owned(), b))
    }

    pub fn blocks(&self) -> Vec<Range<usize>> {
        self.cached_segments().blocks.clone()
    }

    pub fn jump_to(&mut self, offset: usize) {
        let pos = self.field.buffer.position_at(offset);
        self.field.buffer.set_cursor_line_col(pos.line, pos.col);
        self.scroll.ensure_visible(pos.line);
    }

    pub fn cursor_offset(&self) -> usize {
        self.field.buffer.cursor_offset()
    }

    pub fn cursor_cell(&self) -> Option<Rect> {
        let pos = self.field.buffer.cursor_pos();
        if pos.line < self.scroll.offset
            || pos.line >= self.scroll.offset + self.scroll.viewport_len
            || pos.col < self.hscroll
            || pos.col >= self.hscroll.saturating_add(self.text_area.width as usize)
        {
            return None;
        }
        let y = self.text_area.y + (pos.line - self.scroll.offset) as u16;
        let x = self.text_area.x + pos.col.saturating_sub(self.hscroll) as u16;
        Some(Rect::new(x, y, 1, 1))
    }

    pub fn begin_edit(&mut self) {
        if !self.read_only {
            if !self.editing {
                self.snapshot = self.esc_reverts.then(|| {
                    crate::record_work!(PayloadBytesCloned, self.field.buffer.text().len());
                    self.field.buffer.text().to_owned()
                });
            }
            self.editing = true;
        }
    }

    pub fn set_running(&mut self, block: Option<Range<usize>>) {
        self.running = block;
    }

    pub fn commit(&mut self) {
        self.end_edit();
        self.field.buffer.clear_selection();
    }

    fn end_edit(&mut self) {
        self.editing = false;
    }

    pub fn open_find(&mut self) {
        let mut needle = self
            .find
            .as_ref()
            .map(|f| f.needle.clone())
            .unwrap_or_else(|| EditField::single(""));
        needle.keys = self.field.keys;
        self.find = Some(FindState {
            needle,
            matches: vec![],
            current: 0,
            editing: true,
        });
        self.find_scanned = None;
        self.refind();
    }

    fn close_find(&mut self) {
        self.find = None;
        self.find_runs = None;
    }

    fn refind(&mut self) {
        let Some(f) = self.find.as_mut() else {
            return;
        };
        let revision = self.field.buffer.revision();
        let scanned = self
            .find_scanned
            .as_ref()
            .is_some_and(|(rev, needle)| *rev == revision && needle == f.needle.text());
        if !scanned {
            #[cfg(test)]
            index::work::tally(|c| c.find_scans += 1);
            f.matches.clear();
            let needle = f.needle.text();
            if !needle.is_empty() {
                let case_sensitive = needle.chars().any(|c| c.is_uppercase());
                f.matches = find_ranges(self.field.buffer.text(), needle, case_sensitive);
            }
            self.find_scanned = Some((revision, needle.to_owned()));
            self.find_revision = next_generation(self.find_revision);
        }
        let cur = self.field.buffer.cursor_offset();
        let first = f.matches.partition_point(|m| m.start < cur);
        f.current = if first < f.matches.len() { first } else { 0 };
    }

    fn goto_match(&mut self, delta: isize) {
        let Some(f) = self.find.as_mut() else {
            return;
        };
        if f.matches.is_empty() {
            return;
        }
        let n = f.matches.len() as isize;
        f.current = ((f.current as isize + delta).rem_euclid(n)) as usize;
        let m = f.matches[f.current].clone();
        self.jump_to(m.start);
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<EditorAction>) {
        if key.ctrl_char('c') && self.field.buffer.selected_text().is_none() {
            return (Outcome::Ignored, None);
        }

        if (self.edit_menu.is_open() || !self.find.as_ref().is_some_and(|f| f.editing))
            && let Some(o) = edit_menu_key(self, key)
        {
            let ev = self
                .edit_menu
                .take_edited()
                .then_some(EditorAction::Changed);
            return (o, ev);
        }

        if let Some(f) = self.find.as_mut()
            && f.editing
        {
            {
                match key.code {
                    KeyCode::Esc => {
                        self.close_find();
                        return (Outcome::Changed, None);
                    }
                    KeyCode::Enter => {
                        if key.shift() {
                            self.goto_match(-1);
                        } else if f.needle.text().is_empty() {
                            f.editing = false;
                        } else {
                            f.editing = false;
                            self.goto_match(0);
                        }
                        return (Outcome::Changed, Some(EditorAction::CursorMoved));
                    }

                    _ => {
                        let r = f.needle.on_key(key);
                        if r.edited {
                            self.refind();
                            self.goto_match(0);
                            return (Outcome::Changed, Some(EditorAction::CursorMoved));
                        }
                        return (
                            if r.outcome == Outcome::Changed {
                                Outcome::Changed
                            } else {
                                Outcome::Consumed
                            },
                            None,
                        );
                    }
                }
            }
        }

        if self.read_only && self.editing {
            self.end_edit();
        }

        if key.code == KeyCode::F(6) && !key.ctrl() && !key.alt() {
            let backward = key.shift();
            self.end_edit();
            request_focus_next(backward);
            return (Outcome::Changed, Some(EditorAction::Leave { backward }));
        }
        if !self.editing {
            if self.entry() == EditEntry::Immediate && !self.focus_seen && !self.read_only {
                self.begin_edit();
            } else {
                return self.nav_key(key);
            }
        }

        if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) && key.ctrl() {
            let backward = key.code == KeyCode::BackTab || key.shift();
            self.end_edit();
            request_focus_next(backward);
            return (Outcome::Changed, Some(EditorAction::Leave { backward }));
        }
        let action = self.edit_action(key);
        match action {
            EditAction::Cancel if self.esc_reverts => {
                if let Some(snap) = self.snapshot.take()
                    && snap != self.field.buffer.text()
                {
                    self.field.record(EditKind::Other);
                    self.field.buffer.set_text(snap);
                    self.after_change();
                }
                self.commit();
                (Outcome::Changed, Some(EditorAction::Committed))
            }
            EditAction::Cancel | EditAction::Commit => {
                self.commit();
                (Outcome::Changed, Some(EditorAction::Committed))
            }
            EditAction::Tab { backward } => {
                if self.tab_leaves {
                    self.end_edit();
                    request_focus_next(backward);
                    return (Outcome::Changed, Some(EditorAction::Leave { backward }));
                }
                self.field.record(EditKind::Other);
                if backward {
                    self.dedent_selection();
                } else if self.field.buffer.has_selection_lines() {
                    self.indent_selection();
                } else {
                    let spaces = " ".repeat(self.indent);
                    self.field.buffer.insert_str(&spaces);
                }
                self.after_change();
                (Outcome::Changed, Some(EditorAction::Changed))
            }
            EditAction::Apply(_) | EditAction::Insert(_) if self.read_only => {
                (Outcome::Consumed, None)
            }
            EditAction::None => match key.code {
                KeyCode::PageUp if !key.chord() => {
                    for _ in 0..self.scroll.viewport_len.max(1) {
                        self.field.buffer.move_up(false);
                    }
                    self.after_change();
                    (Outcome::Changed, Some(EditorAction::CursorMoved))
                }
                KeyCode::PageDown if !key.chord() => {
                    for _ in 0..self.scroll.viewport_len.max(1) {
                        self.field.buffer.move_down(false);
                    }
                    self.after_change();
                    (Outcome::Changed, Some(EditorAction::CursorMoved))
                }
                _ => (Outcome::Ignored, None),
            },
            action => {
                let edited = self.field.apply_as(action, key, self.read_only).edited;
                self.after_change();
                (
                    Outcome::Changed,
                    Some(if edited {
                        EditorAction::Changed
                    } else {
                        EditorAction::CursorMoved
                    }),
                )
            }
        }
    }

    fn text_cmd(&self, key: &Key) -> Option<crate::core::edit::TextCmd> {
        self.field.text_cmd(key)
    }

    fn nav_key(&mut self, key: &Key) -> (Outcome, Option<EditorAction>) {
        use crate::core::edit::TextCmd;

        if let Some(cmd) = self.text_cmd(key) {
            match cmd {
                TextCmd::Cut | TextCmd::Paste | TextCmd::Undo | TextCmd::Redo if !key.cmd() => {}
                TextCmd::Copy if self.field.buffer.selected_text().is_none() => {}
                TextCmd::Cut | TextCmd::Paste | TextCmd::Undo | TextCmd::Redo => {
                    if self.read_only {
                        return (Outcome::Consumed, None);
                    }
                    self.begin_edit();
                    return self.on_key(key);
                }
                _ => {
                    let action = crate::widgets::field_common::text_cmd_action(cmd, true);
                    if matches!(
                        action,
                        EditAction::Commit
                            | EditAction::Cancel
                            | EditAction::Tab { .. }
                            | EditAction::None
                    ) {
                        return (Outcome::Ignored, None);
                    }
                    self.field.apply_as(action, key, true);
                    self.after_change();
                    return (Outcome::Changed, Some(EditorAction::CursorMoved));
                }
            }
        }
        if let Some(action) = lookup(&self.keymap, key) {
            return self.on_action(action);
        }
        let plain = key.plain();
        match self.nav.action(key) {
            Some(Nav::Up) if plain => {
                self.field.buffer.move_up(false);
                self.after_change();
                (Outcome::Changed, Some(EditorAction::CursorMoved))
            }
            Some(Nav::Down) if plain => {
                self.field.buffer.move_down(false);
                self.after_change();
                (Outcome::Changed, Some(EditorAction::CursorMoved))
            }
            Some(Nav::Left) if plain => {
                self.scroll_h(-8);
                (Outcome::Changed, None)
            }
            Some(Nav::Right) if plain => {
                self.scroll_h(8);
                (Outcome::Changed, None)
            }
            Some(Nav::PageUp) => {
                self.scroll.page_up();
                (Outcome::Changed, None)
            }
            Some(Nav::PageDown) => {
                self.scroll.page_down();
                (Outcome::Changed, None)
            }
            Some(Nav::Home) if plain => {
                self.field.buffer.move_doc_start(false);
                self.after_change();
                (Outcome::Changed, Some(EditorAction::CursorMoved))
            }
            Some(Nav::End) => {
                self.field.buffer.move_doc_end(false);
                self.after_change();
                (Outcome::Changed, Some(EditorAction::CursorMoved))
            }
            _ if key.code == KeyCode::Esc => {
                if self.find.is_some() {
                    self.close_find();
                    (Outcome::Changed, None)
                } else {
                    (Outcome::Ignored, None)
                }
            }
            _ => (Outcome::Ignored, None),
        }
    }

    fn on_action(&mut self, action: CodeCmd) -> (Outcome, Option<EditorAction>) {
        match action {
            CodeCmd::Edit => {
                self.begin_edit();
                (Outcome::Changed, None)
            }
            CodeCmd::Append => {
                self.begin_edit();
                self.field.buffer.move_right(false);
                (Outcome::Changed, None)
            }
            CodeCmd::PrevBlock => {
                let cur = self.field.buffer.cursor_offset();
                let target = self
                    .cached_segments()
                    .blocks
                    .iter()
                    .rev()
                    .find(|b| b.start < cur)
                    .map(|b| b.start);
                if let Some(start) = target {
                    self.jump_to(start);
                }
                (Outcome::Changed, Some(EditorAction::CursorMoved))
            }
            CodeCmd::NextBlock => {
                let cur = self.field.buffer.cursor_offset();
                let target = self
                    .cached_segments()
                    .blocks
                    .iter()
                    .find(|b| b.start > cur)
                    .map(|b| b.start);
                if let Some(start) = target {
                    self.jump_to(start);
                }
                (Outcome::Changed, Some(EditorAction::CursorMoved))
            }
            CodeCmd::Find => {
                self.open_find();
                (Outcome::Changed, None)
            }
            CodeCmd::NextMatch => {
                self.goto_match(1);
                (Outcome::Changed, Some(EditorAction::CursorMoved))
            }
            CodeCmd::PrevMatch => {
                self.goto_match(-1);
                (Outcome::Changed, Some(EditorAction::CursorMoved))
            }
        }
    }

    fn after_change(&mut self) {
        let pos = self.field.buffer.cursor_pos();
        self.scroll.set_content(self.field.buffer.line_count());
        self.scroll.ensure_visible(pos.line);
        let w = self.text_area.width.max(8) as usize;
        if pos.col < self.hscroll + 4 {
            self.hscroll = pos.col.saturating_sub(4);
        } else if pos.col + 4 >= self.hscroll + w {
            self.hscroll = pos.col + 5 - w;
        }
        if self.find.is_some() {
            self.refind();
        }
    }

    fn indent_selection(&mut self) {
        let spaces = " ".repeat(self.indent);
        let (a, b) = self.field.buffer.selection_lines();
        for line in a..=b {
            let off = self.field.buffer.offset_at(line, 0);
            self.field.buffer.insert_at(off, &spaces);
        }
    }

    fn dedent_selection(&mut self) {
        let (a, b) = self.field.buffer.selection_lines();
        for line in a..=b {
            let off = self.field.buffer.offset_at(line, 0);
            let text = self.field.buffer.text();
            let n = text[off..]
                .chars()
                .take(self.indent)
                .take_while(|c| *c == ' ')
                .count();
            self.field.buffer.remove_range(off..off + n);
        }
    }

    fn offset_at_pos(&self, pos: Position) -> usize {
        let line = (self.scroll.offset as isize + (pos.y as isize - self.text_area.y as isize))
            .max(0) as usize;
        let line = line.min(self.field.buffer.line_count().saturating_sub(1));
        let col =
            (self.hscroll as isize + (pos.x as isize - self.text_area.x as isize)).max(0) as usize;
        self.field.buffer.offset_at(line, col)
    }

    pub fn on_click(&mut self, pos: Position) -> Outcome {
        PointerField::on_click_count(self, pos, 1)
    }

    pub fn on_drag(&mut self, pos: Position) -> Outcome {
        PointerField::on_drag(self, pos)
    }

    pub fn on_paste(&mut self, text: &str) -> Outcome {
        if let Some(find) = self.find.as_mut()
            && find.editing
        {
            find.needle.paste(text);
            self.refind();
            self.goto_match(0);
            return Outcome::Changed;
        }
        if !self.editing || self.read_only {
            return Outcome::Ignored;
        }
        self.field.record(EditKind::Other);
        self.field.buffer.insert_str(text);
        self.after_change();
        Outcome::Changed
    }

    fn prepare_annotations(&mut self) {
        let text = self.field.buffer.revision();
        let key = ProviderKey {
            text,
            provider: self.highlighter_generation,
        };
        if self.syntax.as_ref().map(|c| c.key) != Some(key) {
            let spans = self
                .highlighter
                .map(|f| {
                    crate::record_work!(AnnotationQueries, 1usize);
                    f(self.field.buffer.text())
                })
                .unwrap_or_default();
            self.syntax = Some(SyntaxCache::build(key, spans));
        }
        let stale = self
            .diagnostic_index
            .as_ref()
            .is_none_or(|d| d.text != text || d.revision != self.diagnostics_revision);
        if stale {
            self.diagnostic_index = Some(DiagnosticIndex::build(
                text,
                self.diagnostics_revision,
                &self.diagnostics,
                &self.field.buffer,
            ));
        }
        if let Some(f) = &self.find {
            let current = self
                .find_runs
                .as_ref()
                .is_some_and(|(t, r, _)| *t == text && *r == self.find_revision);
            if !current {
                let runs = RunIndex::build(f.matches.iter());
                self.find_runs = Some((text, self.find_revision, runs));
            }
        }
        if self.brackets.as_ref().map(|b| b.text) != Some(text) {
            self.brackets = Some(BracketMap::build(text, self.field.buffer.text()));
        }
    }

    pub fn track_click(mut self, click: crate::core::scroll::TrackClick) -> Self {
        self.scroll.track_click = Some(click);
        self
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        self.render_editor(area, buf, ctx, bg);

        self.edit_menu.render(self.area, buf, ctx);
    }

    fn render_editor(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        self.scroll.begin(ctx.track_click);
        self.entry = self.edit_entry.unwrap_or(ctx.theme.edit_entry);
        self.field.keys = self.field_keys.or(ctx.theme.field_keys).unwrap_or_default();

        let shown = ctx.interaction.focused(self.id) && !ctx.inert;
        if shown
            && !self.focus_seen
            && self.entry == EditEntry::Immediate
            && !self.read_only
            && !self.editing
        {
            self.begin_edit();
            ctx.note_render_mutation();
        }
        self.focus_seen = shown;
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }
        self.area = area;

        let put = |buf: &mut Buffer, x: u16, y: u16, text: &str, style: Style| {
            if area.contains(Position::new(x, y)) {
                buf.set_stringn(x, y, text, (area.right() - x) as usize, style);
            }
        };
        let t = ctx.theme;
        let mut s = ctx.state(self.id);

        let held = s.focused;
        s.focused = ctx
            .interaction
            .focused_under(self.id, self.edit_menu.is_open());
        s.editing = self.editing && held;

        if (!held || self.read_only) && self.editing {
            self.end_edit();
            s.editing = false;
            ctx.note_render_mutation();
        }
        let focused = s.focused;
        ctx.control(self.id, area, false);

        if self.field.buffer.has_selection() {
            ctx.selection(self.id);
        }
        ctx.scrollable(self.id, area);
        ctx.pointer_shape(self.id, crate::core::shape::PointerShape::Text);

        let fs = if self.read_only {
            Style::new().fg(t.text_primary).bg(bg)
        } else {
            t.field_style(s)
        };
        fill(buf, area, fs);
        let body_h = area.height.saturating_sub(1);
        let line_count = self.field.buffer.line_count();
        let num_w = (line_count.to_string().len() as u16).max(2);

        let gutter_w = (1 + 1 + num_w + 1 + 1).min(area.width);
        self.gutter_w = gutter_w;

        self.scroll.h_cfg = self.horizontal.unwrap_or(ctx.hscroll);
        let widest = self.field.buffer.max_line_width();
        let gap = self.scrollbar_gap;
        let text_w = |has_sb: bool| {
            area.width
                .saturating_sub(gutter_w + if has_sb { scrollbar::reserve(gap) } else { 0 })
        };
        let mut has_sb = line_count > body_h as usize;

        self.scroll.h.offset = self.hscroll;
        self.scroll.h.set_content(widest + 1);
        self.scroll.h.set_viewport(text_w(has_sb) as usize);
        let has_hb =
            self.scroll.h_cfg.enabled && body_h > 1 && self.scroll.h_cfg.shows_bar(&self.scroll.h);
        let rows_h = body_h - u16::from(has_hb);
        if !has_sb && line_count > rows_h as usize {
            has_sb = true;
            self.scroll.h.set_viewport(text_w(true) as usize);
        }

        self.hscroll = self.scroll.h.offset;
        let rows = rows_h as usize;
        self.scroll.set_content(line_count);
        self.scroll.set_viewport(rows);
        let text_area = Rect::new(area.x + gutter_w, area.y, text_w(has_sb), rows_h);
        let viewport_changed =
            self.text_area.width != text_area.width || self.text_area.height != text_area.height;
        self.text_area = text_area;
        if self.field.sel.drag {
            ctx.autoscroll(text_area);
        }
        let cur = self.field.buffer.cursor_pos();
        let cur_off = self.field.buffer.cursor_offset();
        let rendered_state = (cur_off, self.field.buffer.revision(), s.editing);
        if s.editing && (viewport_changed || self.rendered_state != Some(rendered_state)) {
            self.scroll.ensure_visible(cur.line);
        }
        self.rendered_state = Some(rendered_state);
        let block = if self.field.buffer.selection().is_none() {
            self.current_block()
        } else {
            None
        };
        self.prepare_annotations();
        let running = self.running.as_ref();
        let sel = self.field.buffer.selection();
        let diags = self.diagnostics.as_slice();
        let syntax = self.syntax.as_ref().expect("annotations prepared above");
        let diag_index = self
            .diagnostic_index
            .as_ref()
            .expect("annotations prepared above");
        let brackets = self.brackets.as_ref().expect("annotations prepared above");
        let bracket = brackets.pair_at(cur_off);
        let find_runs = self
            .find
            .as_ref()
            .and_then(|f| Some((f.current, &self.find_runs.as_ref()?.2)));
        let gutter_bar = t.gutter(s, fs.bg.unwrap_or(bg), false);
        for row in 0..rows {
            let li = self.scroll.offset + row;
            let y = area.y + row as u16;

            if li == cur.line {
                put(buf, area.x, y, t.gutter_symbol(s), gutter_bar);
            }
            if li >= line_count {
                continue;
            }
            let Some(view) = self.field.buffer.line_view(li) else {
                continue;
            };
            #[cfg(test)]
            index::work::tally(|c| c.rows += 1);
            crate::record_work!(VisibleRowsPainted, 1usize);
            let line = view.text();
            let ls = view.start();
            let le = ls + line.len();

            let mut marker = " ";
            let mut marker_style = fs;
            if let Some(r) = running {
                if r.start >= ls && r.start <= le {
                    marker = t.glyphs.spinner_frame(ctx.interaction.tick);
                    marker_style = fs.fg(t.accent);
                }
            } else if let Some(b) = &block {
                let first_content = b.start;
                if first_content >= ls && first_content <= le {
                    marker = t.glyphs.current;

                    marker_style = fs.fg(if focused {
                        t.text_primary
                    } else {
                        t.text_secondary
                    });
                }
            }

            let diag_here = diag_index.gutter(li).map(|ordinal| &diags[ordinal]);
            if let Some(d) = diag_here {
                let c = if d.severity == Severity::Error {
                    t.error
                } else {
                    t.warning
                };
                marker = "!";
                marker_style = fs.fg(c).add_modifier(Modifier::BOLD);
            }
            put(buf, area.x + 1, y, marker, marker_style);

            let in_block = block.as_ref().is_some_and(|b| le >= b.start && ls <= b.end);
            let ns = if li == cur.line && focused {
                fs.fg(t.text_primary).add_modifier(Modifier::BOLD)
            } else if in_block {
                fs.fg(t.text_secondary)
            } else {
                fs.fg(t.text_muted)
            };
            let ns = if self.read_only {
                fs.fg(t.text_faint)
            } else {
                ns
            };
            put(
                buf,
                area.x + 3,
                y,
                &crate::ui::text::fit_right(&(li + 1).to_string(), num_w as usize),
                ns,
            );
            if text_area.width == 0 {
                continue;
            }

            let mut x = text_area.x;
            if self.hscroll > 0 && !line.is_empty() {
                put(buf, x, y, t.glyphs.ellipsis, fs.fg(t.text_muted));
            }
            let underline_line = s.editing && li == cur.line;
            for at in view.graphemes_from_col(self.hscroll) {
                let (g, gw) = (at.text, at.width);
                if at.col == self.hscroll && self.hscroll > 0 {
                    x += gw as u16;
                    continue;
                }
                if x + gw as u16 > text_area.right() {
                    put(
                        buf,
                        text_area.right().saturating_sub(1),
                        y,
                        t.glyphs.ellipsis,
                        fs.fg(t.text_muted),
                    );
                    break;
                }
                #[cfg(test)]
                index::work::tally(|c| c.graphemes += 1);
                let off = at.offset;
                let mut st = fs.patch(t.syntax(syntax.tone_at(off)));
                if let Some(r) = &sel
                    && r.contains(&off)
                {
                    let sel = t.selection();
                    st = st.bg(t.selection_bg).add_modifier(sel.add_modifier);
                }
                if let Some((current, runs)) = find_runs
                    && let Some(mi) = runs.at(off)
                {
                    st = if mi == current {
                        st.patch(t.match_current())
                    } else {
                        st.add_modifier(Modifier::UNDERLINED)
                            .underline_color(t.border_strong)
                    };
                }
                if let Some(d) = diag_index.at(off).map(|ordinal| &diags[ordinal]) {
                    let c = if d.severity == Severity::Error {
                        t.error
                    } else {
                        t.warning
                    };
                    st = st.add_modifier(Modifier::UNDERLINED).underline_color(c);
                }
                if bracket.is_some_and(|(a, b)| off == a || off == b) {
                    st = st
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                        .underline_color(t.border_strong);
                }
                if underline_line && !st.add_modifier.contains(Modifier::UNDERLINED) {
                    st = st
                        .add_modifier(Modifier::UNDERLINED)
                        .underline_color(t.border_strong);
                }
                put(buf, x, y, g, st);
                x += gw as u16;
            }
            if underline_line {
                for xx in x..text_area.right() {
                    if let Some(c) = buf.cell_mut(Position::new(xx, y)) {
                        c.set_style(
                            c.style()
                                .add_modifier(Modifier::UNDERLINED)
                                .underline_color(t.border_strong),
                        );
                    }
                }
            }
        }

        if self.field.buffer.is_empty() && !self.placeholder.is_empty() {
            put(
                buf,
                text_area.x,
                text_area.y,
                &crate::ui::text::truncate(&self.placeholder, text_area.width as usize),
                fs.fg(t.text_muted),
            );
        }

        if (s.editing || (focused && self.read_only))
            && let Some(cursor) = self.cursor_cell()
        {
            ctx.set_cursor(Position::new(cursor.x, cursor.y));
        }
        if has_sb {
            let body = Rect::new(area.x, area.y, area.width, rows_h);
            self.scroll
                .draw_with_fade(body, buf, ctx, self.id, focused, &[]);
        }
        if has_hb {
            let y = area.y + rows_h;
            fill(buf, Rect::new(area.x, y, area.width, 1), fs);
            let hb = Rect::new(text_area.x, y, text_area.width, 1);
            self.scroll.draw_h(hb, buf, ctx, self.id, focused);
        }

        let fy = area.y + body_h;
        if let Some(f) = &self.find {
            let left = area.x + u16::from(area.width > 2);
            let available = (area.right() - left) as usize;
            let label = if available >= 8 { "find " } else { "" };
            put(buf, left, fy, label, fs.fg(t.text_muted));
            let nx = left + label.len() as u16;
            let ns = if f.editing {
                fs.add_modifier(Modifier::UNDERLINED)
                    .underline_color(t.accent)
            } else {
                fs
            };
            let count = if f.matches.is_empty() {
                if f.needle.text().is_empty() {
                    String::new()
                } else {
                    "no matches".into()
                }
            } else {
                format!("{}/{}", f.current + 1, f.matches.len())
            };
            let count_width = width(&count);
            let count_room = if !count.is_empty() && available >= label.len() + count_width + 4 {
                count_width + 1
            } else {
                0
            };
            let query_room = available.saturating_sub(label.len() + count_room);

            let line =
                Rect::new(nx, fy, query_room.min(u16::MAX as usize) as u16, 1).intersection(area);
            let look = LineLook {
                text: ns,
                selection: t.selection(),
                ellipsis: fs.fg(t.text_muted),
                ellipsis_glyph: t.glyphs.ellipsis,
                underline: None,
                follow_caret: true,
            };
            let caret = self
                .find
                .as_mut()
                .map(|f| f.needle.draw_line(buf, line, &look));
            let editing = self.find.as_ref().is_some_and(|f| f.editing);
            if editing {
                ctx.cursor = None;
                if focused
                    && query_room > 0
                    && let Some(caret) = caret
                {
                    ctx.set_cursor(caret);
                }
            }
            if count_room > 0 {
                put(
                    buf,
                    area.right() - count_width as u16,
                    fy,
                    &count,
                    fs.fg(t.text_muted),
                );
            }
            return;
        }
        let pos = if s.editing || focused {
            format!(
                "ln {}/{}{}col {}",
                cur.line + 1,
                line_count,
                t.glyphs.separator,
                cur.col + 1
            )
        } else if self.scroll.overflows() {
            scrollbar::position_label(&self.scroll)
        } else {
            String::new()
        };
        let right_padding = u16::from(area.width > 1);
        let pos = crate::ui::text::truncate(&pos, (area.width - right_padding) as usize);
        let px = area
            .right()
            .saturating_sub(width(&pos) as u16 + right_padding)
            .max(area.x);
        if let Some(d) = diag_index.nearest(cur_off).map(|ordinal| &diags[ordinal]) {
            let c = if d.severity == Severity::Error {
                t.error
            } else {
                t.warning
            };

            let room = px.saturating_sub(area.x + 2) as usize;
            put(
                buf,
                area.x + 1,
                fy,
                &crate::ui::text::truncate(&d.message, room),
                fs.fg(c),
            );
        }
        if !pos.is_empty() {
            put(buf, px, fy, &pos, fs.fg(t.text_faint));
        }
    }
}

impl crate::ui::measure::Measure for CodeEditor {
    fn measure(&self, _avail: ratatui::layout::Size, _ctx: &RenderCtx) -> ratatui::layout::Size {
        let line_count = self.field.buffer.line_count();
        let num_w = (line_count.to_string().len() as u16).max(2);
        let widest = self.field.buffer.max_line_width() as u16;
        ratatui::layout::Size::new(1 + 1 + num_w + 1 + 1 + widest + 1, line_count as u16 + 1)
    }
}

impl Scrollable for CodeEditor {
    fn pane(&mut self) -> &mut ScrollPane {
        self.scroll.h.offset = self.hscroll;
        &mut self.scroll
    }

    fn after_scroll(&mut self, axis: Axis, _moved: bool) -> bool {
        if axis == Axis::Horizontal {
            self.hscroll = self.scroll.h.offset;
        }
        false
    }
}

impl Editable for CodeEditor {
    fn field(&mut self) -> &mut EditField {
        &mut self.field
    }
    fn edit_id(&self) -> WidgetId {
        self.id
    }
    fn offset_at(&self, pos: Position) -> usize {
        self.offset_at_pos(pos)
    }
    fn pointer_begin(&mut self) {
        self.begin_edit();
    }
    fn pointer_after(&mut self) {
        self.after_change();
    }
}

impl crate::core::pointer::MouseTarget for CodeEditor {
    type Action = ();

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<()> {
        use crate::core::intent::Phase;
        if let Some(o) = edit_menu_pointer(self, ev) {
            return o.into();
        }
        if let Some(r) = crate::core::pointer::scroll_input(self, ev) {
            return r;
        }
        match ev.phase() {
            Some(Phase::Press) => PointerField::on_press_count(self, ev.pos(), ev.count()).into(),
            Some(Phase::Click) => PointerField::on_click_count(self, ev.pos(), ev.count()).into(),
            Some(Phase::Drag) => PointerField::on_drag(self, ev.pos()).into(),
            _ => crate::core::response::Response::ignored(),
        }
    }

    fn route(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> Option<crate::core::response::Response<()>> {
        edit_menu_route(self, ev)
    }
}

impl EditMenuHost for CodeEditor {
    fn edit_menu(&mut self) -> &mut EditMenu {
        &mut self.edit_menu
    }
    fn edit_owner(&self) -> WidgetId {
        self.id
    }
    fn edit_caps(&self) -> EditCaps {
        let mut caps = EditCaps::of(&self.field);
        caps.editable = !self.read_only;
        caps
    }
    fn edit_shortcut(&self, cmd: TextCmd) -> Option<String> {
        self.field.shortcut(cmd)
    }
    fn edit_target(&self, ev: &crate::core::pointer::PointerEvent) -> bool {
        ev.id == self.id && self.text_area.contains(ev.pos())
    }
    fn edit_point(&mut self, pos: Position) {
        point_editable(self, pos);
    }
    fn edit_run(&mut self, cmd: TextCmd) -> bool {
        if matches!(cmd, TextCmd::Cut | TextCmd::Paste) {
            self.begin_edit();
        }
        let edited = self.field.run_as(cmd, self.read_only).edited;
        self.after_change();
        edited
    }
}

impl HasHints for CodeEditor {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        CodeEditor::hints(self)
    }
}

#[cfg(test)]
mod issue105_tests {
    use ratatui::crossterm::event::KeyModifiers;

    use super::*;
    use crate::core::focus::FocusRing;
    use crate::core::hit::HitRegistry;
    use crate::theme::{ColorLevel, Theme};
    use crate::ui::ctx::Interaction;

    const ID: WidgetId = WidgetId::of("issue105.editor");
    const AREA: Rect = Rect::new(0, 0, 40, 4);

    fn key(code: KeyCode, mods: KeyModifiers) -> Key {
        Key { code, mods }
    }

    fn paint(editor: &mut CodeEditor, focused: bool) -> Buffer {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(AREA);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let interaction = Interaction {
            focus: focused.then_some(ID),
            ..Default::default()
        };
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        editor.render(AREA, &mut buf, &mut ctx, theme.canvas);
        buf
    }

    fn selected_cells(buf: &Buffer) -> usize {
        let selection_bg = Theme::for_level(ColorLevel::TrueColor).selection_bg;
        buf.content()
            .iter()
            .filter(|c| c.bg == selection_bg)
            .count()
    }

    fn editor_with_selection() -> CodeEditor {
        let mut editor = CodeEditor::new(ID, "select this text");
        paint(&mut editor, true);
        editor.begin_edit();
        for _ in 0..6 {
            editor.on_key(&key(KeyCode::Right, KeyModifiers::SHIFT));
        }
        assert_eq!(
            editor.selection_or_block().map(|(t, _)| t).as_deref(),
            Some("select")
        );
        editor
    }

    #[test]
    fn unfocused_paint_ends_the_edit_and_keeps_the_selection() {
        let mut editor = editor_with_selection();
        let focused = selected_cells(&paint(&mut editor, true));
        assert_eq!(focused, "select".len());

        let blurred = paint(&mut editor, false);
        assert!(!editor.editing, "blur ends the edit session");
        assert_eq!(
            editor.selection_or_block().map(|(t, _)| t).as_deref(),
            Some("select"),
            "the selection is document state and survives blur"
        );
        assert_eq!(selected_cells(&blurred), focused, "it stays painted");

        paint(&mut editor, true);
        assert_eq!(
            editor.selection_or_block().map(|(t, _)| t).as_deref(),
            Some("select"),
            "and is still there when focus returns"
        );
    }

    #[test]
    fn leaving_with_the_keyboard_keeps_the_selection() {
        for leave in [
            key(KeyCode::F(6), KeyModifiers::NONE),
            key(KeyCode::Tab, KeyModifiers::CONTROL),
        ] {
            let mut editor = editor_with_selection();
            let (_, action) = editor.on_key(&leave);
            assert!(matches!(action, Some(EditorAction::Leave { .. })));
            assert!(!editor.editing);
            assert_eq!(
                editor.selection_or_block().map(|(t, _)| t).as_deref(),
                Some("select")
            );
        }
        crate::core::focus::cancel_focus_request();
    }

    #[test]
    fn explicit_commit_still_clears_the_selection() {
        let mut editor = editor_with_selection();
        let (_, action) = editor.on_key(&key(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(action, Some(EditorAction::Committed)));
        assert_eq!(editor.field.buffer.selection(), None);
    }
}

#[cfg(test)]
mod tests;

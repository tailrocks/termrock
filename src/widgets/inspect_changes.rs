use crate::core::event::{Key, Outcome};
use crate::core::focus::Focus;
use crate::core::hit::Axis;
use crate::core::id::WidgetId;
use crate::core::intent::Phase;
use crate::core::pointer::{MouseTarget, PointerEvent};
use crate::core::scroll::{ScrollState, TrackClick};
use crate::ui::ctx::{RenderCtx, fill, legible_on_fill};
use crate::ui::layout::{Split, SplitDir};
use crate::ui::text::{fit, truncate_middle, width};

use crate::widgets::diff::DiffView;
use crate::widgets::keyhint::{Hint, hint};
use crate::widgets::scrollbar;
use crate::widgets::splitter::Splitter;
use crate::widgets::tree::{TreeAction, TreeNode, TreeView};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

use crate::core::scroll::Scrollable;
use crate::ui::layer::{DismissReason, Layer, LayerCx, LayerOut, LayerSpec};
use crate::ui::modal::modal_frame;
use crate::ui::popup::Dismiss;
use crate::widgets::diff::{DiffFile as ChangedFile, DiffStatus, UnreadFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSet {
    pub files: Vec<ChangedFile>,

    pub unpushed: usize,

    /// Changed files the source named but did not list: it reports a bounded
    /// number of files.
    pub omitted: usize,

    /// Files that are listed with no diff because their content was not read.
    pub unread: Vec<UnreadFile>,
}

/// What the list says about the two limits of a [`ChangeSet`]. `omitted`
/// takes `{count}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeWords {
    pub omitted: String,
    pub unread: String,
}

impl Default for ChangeWords {
    fn default() -> Self {
        Self {
            omitted: "{count} more not listed".into(),
            unread: "not read".into(),
        }
    }
}

impl ChangeSet {
    pub fn additions(&self) -> usize {
        self.files.iter().map(|f| f.additions()).sum()
    }
    pub fn deletions(&self) -> usize {
        self.files.iter().map(|f| f.deletions()).sum()
    }
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.unread.is_empty() && self.omitted == 0 && self.unpushed == 0
    }

    pub fn summary(&self) -> String {
        let n = self.files.len() + self.unread.len();
        format!(
            "{n} file{} · +{} −{}",
            if n == 1 { "" } else { "s" },
            self.additions(),
            self.deletions()
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectOutcome {
    Close,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectMode {
    Compact,
    Advanced,
}

impl InspectMode {
    pub fn label(self) -> &'static str {
        match self {
            InspectMode::Compact => "compact",
            InspectMode::Advanced => "advanced",
        }
    }
    fn toggled(self) -> Self {
        match self {
            InspectMode::Compact => InspectMode::Advanced,
            InspectMode::Advanced => InspectMode::Compact,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Region {
    Tree,
    Diff,
}

pub struct InspectChanges {
    pub id: WidgetId,
    pub title: String,
    pub changes: ChangeSet,
    pub mode: InspectMode,
    words: ChangeWords,
    listed: Vec<ChangedFile>,

    cursor: usize,

    open: bool,
    list_scroll: ScrollState,
    tree: TreeView,

    leaves: Vec<(Vec<usize>, usize)>,
    diff: DiffView,
    split: Split,
    seam: Splitter,
    region: Region,

    tree_area: Rect,
    diff_area: Rect,
    container: Rect,
    stacked: bool,

    returns_to_exit: bool,
    result: Option<InspectOutcome>,
    pub area: Rect,
}

const VIEW_KEY: &str = "v";
const VIEW_CHAR: char = VIEW_KEY.as_bytes()[0] as char;
const MIN_W: u16 = 76;
const MIN_H: u16 = 20;

impl InspectChanges {
    pub fn new(id: WidgetId, title: &str, changes: ChangeSet, mode: InspectMode) -> Self {
        let words = ChangeWords::default();
        let listed = listed_files(&changes);
        let (nodes, leaves) = build_tree(&listed, changes.files.len(), &words.unread);
        let mut tree = TreeView::new(id.sub("tree"), nodes);
        tree.expand_all();
        let mut s = Self {
            id,
            title: title.to_owned(),
            changes,
            mode,
            words,
            listed,
            cursor: 0,
            open: false,
            list_scroll: ScrollState::default(),
            tree,
            leaves,
            diff: DiffView::new(id.sub("diff")),
            split: Split::new(34, 22, 30),
            seam: Splitter::new(id.sub("seam"), SplitDir::Horizontal),
            region: Region::Tree,
            tree_area: Rect::ZERO,
            diff_area: Rect::ZERO,
            container: Rect::ZERO,
            stacked: false,
            returns_to_exit: false,
            result: None,
            area: Rect::ZERO,
        };
        s.select_file(0);
        s
    }

    pub fn returns_to_exit(mut self, on: bool) -> Self {
        self.returns_to_exit = on;
        self
    }

    pub fn words(mut self, words: ChangeWords) -> Self {
        self.words = words;
        let (nodes, leaves) =
            build_tree(&self.listed, self.changes.files.len(), &self.words.unread);
        self.tree = TreeView::new(self.id.sub("tree"), nodes);
        self.tree.expand_all();
        self.leaves = leaves;
        self.select_file(self.cursor);
        self
    }

    fn summary_line(&self) -> String {
        let summary = self.changes.summary();
        if self.changes.omitted == 0 {
            return summary;
        }
        let omitted = self
            .words
            .omitted
            .replace("{count}", &self.changes.omitted.to_string());
        format!("{summary} · {omitted}")
    }

    fn select_file(&mut self, i: usize) {
        let f = self.listed.get(i).cloned();
        self.diff.set_file(f);
        if let Some((path, _)) = self.leaves.iter().find(|(_, fi)| *fi == i) {
            self.tree.selected = Some(path.clone());
            if let Some(row) = self.tree.rows().iter().position(|r| &r.path == path) {
                self.tree.cursor = row;
                self.tree.scroll.ensure_visible(row);
            }
        }
        if i < self.listed.len() {
            self.cursor = i;
        }
    }

    fn file_of_path(&self, path: &[usize]) -> Option<usize> {
        self.leaves.iter().find(|(p, _)| p == path).map(|(_, i)| *i)
    }

    fn compact_rows(&self) -> usize {
        self.listed.len() + usize::from(self.changes.unpushed > 0)
    }

    fn set_mode(&mut self, mode: InspectMode, focus: &mut Focus) {
        self.mode = mode;
        self.open = false;
        match mode {
            InspectMode::Advanced => {
                self.region = Region::Tree;
                focus.focus(self.tree.id);
            }
            InspectMode::Compact => focus.focus(self.id),
        }
    }

    fn set_region(&mut self, region: Region, focus: &mut Focus) {
        self.region = region;
        focus.focus(match region {
            Region::Tree => self.tree.id,
            Region::Diff => self.diff.id(),
        });
    }

    fn outcome(&self) -> InspectOutcome {
        if self.returns_to_exit {
            InspectOutcome::Back
        } else {
            InspectOutcome::Close
        }
    }

    fn leave(&mut self) -> Outcome {
        self.result = Some(self.outcome());
        Outcome::Changed
    }

    pub fn wheel_at(&mut self, axis: Axis, delta: i32, pos: Position) -> Outcome {
        if axis == Axis::Horizontal {
            let over_diff = match self.mode {
                InspectMode::Advanced => self.diff_area.contains(pos),
                InspectMode::Compact => self.open,
            };
            return if over_diff {
                self.diff.wheel(axis, delta).outcome()
            } else {
                Outcome::Consumed
            };
        }
        match self.mode {
            InspectMode::Advanced => {
                let out = if self.tree_area.contains(pos) {
                    self.tree.wheel(axis, delta).outcome()
                } else if self.diff_area.contains(pos) {
                    self.diff.wheel(axis, delta).outcome()
                } else {
                    Outcome::Consumed
                };
                out.or(Outcome::Consumed)
            }
            InspectMode::Compact => {
                if self.open {
                    self.diff.wheel(axis, delta).outcome()
                } else {
                    self.list_scroll.scroll_by(delta as isize);
                    Outcome::Changed
                }
            }
        }
    }

    fn compact_key(&mut self, key: &Key, focus: &mut Focus) -> Outcome {
        if self.open {
            match key.code {
                KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => {
                    self.open = false;
                    focus.focus(self.id);
                    return Outcome::Changed;
                }
                _ => {}
            }
            let (o, _) = self.diff.on_key(key);
            return o.or(Outcome::Consumed);
        }
        let n = self.compact_rows();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.leave(),
            KeyCode::Up | KeyCode::Char('k') => {
                self.cursor = self.cursor.saturating_sub(1);
                self.list_scroll.ensure_visible(self.cursor);
                Outcome::Changed
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.cursor = (self.cursor + 1).min(n.saturating_sub(1));
                self.list_scroll.ensure_visible(self.cursor);
                Outcome::Changed
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.cursor = 0;
                self.list_scroll.ensure_visible(0);
                Outcome::Changed
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.cursor = n.saturating_sub(1);
                self.list_scroll.ensure_visible(self.cursor);
                Outcome::Changed
            }
            KeyCode::PageUp => {
                self.cursor = self
                    .cursor
                    .saturating_sub(self.list_scroll.viewport_len.max(1));
                self.list_scroll.ensure_visible(self.cursor);
                Outcome::Changed
            }
            KeyCode::PageDown => {
                self.cursor =
                    (self.cursor + self.list_scroll.viewport_len.max(1)).min(n.saturating_sub(1));
                self.list_scroll.ensure_visible(self.cursor);
                Outcome::Changed
            }
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right | KeyCode::Char('l') => {
                if self.cursor < self.changes.files.len() {
                    self.select_file(self.cursor);
                    self.open = true;
                    focus.focus(self.diff.id());
                }
                Outcome::Changed
            }
            _ => Outcome::Consumed,
        }
    }

    fn advanced_key(&mut self, key: &Key, focus: &mut Focus) -> Outcome {
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                let next = match self.region {
                    Region::Tree => Region::Diff,
                    Region::Diff => Region::Tree,
                };
                self.set_region(next, focus);
                return Outcome::Changed;
            }
            KeyCode::Esc if self.region == Region::Diff => {
                let (o, _) = self.diff.on_key(key);
                if o != Outcome::Changed {
                    self.set_region(Region::Tree, focus);
                }
                return Outcome::Changed;
            }
            KeyCode::Esc | KeyCode::Char('q') => return self.leave(),
            _ => {}
        }
        match self.region {
            Region::Tree => {
                let (o, actions) = self.tree.on_key(key).into_parts();
                for action in actions {
                    if let TreeAction::Activate(path) = action
                        && let Some(i) = self.file_of_path(&path)
                    {
                        self.select_file(i);
                        return Outcome::Changed;
                    }
                }

                if o == Outcome::Changed
                    && let Some(row) = self.tree.rows().get(self.tree.cursor)
                    && let Some(i) = self.file_of_path(&row.path)
                {
                    self.select_file(i);
                    self.tree.cursor =
                        row_index(&self.tree, i, &self.leaves).unwrap_or(self.tree.cursor);
                }
                o.or(Outcome::Consumed)
            }
            Region::Diff => {
                let (o, _) = self.diff.on_key(key);
                o.or(Outcome::Consumed)
            }
        }
    }

    fn draw_compact(
        &mut self,
        inner: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        let t = ctx.theme;
        if self.open {
            let Some(f) = self.diff.file().cloned() else {
                return;
            };
            let head = format!("{}  {}", f.header(), f.summary());
            buf.set_string(
                inner.x,
                inner.y,
                crate::ui::reveal::cut(Rect::new(inner.x, inner.y, inner.width, 1), &head),
                t.secondary().bg(bg),
            );
            let mode = format!("{} · {VIEW_KEY} switches", self.diff.mode.label());
            let mw = width(&mode) as u16;
            if inner.width > width(&head) as u16 + mw + 3 {
                buf.set_string(inner.right() - mw, inner.y, &mode, t.faint().bg(bg));
            }
            let body = Rect::new(
                inner.x,
                inner.y + 2,
                inner.width,
                inner.height.saturating_sub(2),
            );
            self.diff_area = body;
            self.diff.render(body, buf, ctx, bg);
            return;
        }
        let summary = self.summary_line();
        buf.set_string(inner.x, inner.y, &summary, t.secondary().bg(bg));
        let mode = format!("{} · {}", self.mode.label(), self.diff.mode.label());
        let mw = width(&mode) as u16;
        if inner.width > width(&summary) as u16 + mw + 3 {
            buf.set_string(inner.right() - mw, inner.y, &mode, t.faint().bg(bg));
        }
        let list = Rect::new(
            inner.x,
            inner.y + 2,
            inner.width,
            inner.height.saturating_sub(2),
        );
        let n = self.compact_rows();
        self.list_scroll.set_content(n);
        self.list_scroll.set_viewport(list.height as usize);
        ctx.control(self.id, list, false);
        ctx.scrollable(self.id, list);
        let has_sb = self.list_scroll.overflows();
        let row_w = list.width.saturating_sub(u16::from(has_sb));
        let focused = ctx.interaction.focused(self.id);
        if n == 0 {
            buf.set_string(
                list.x + 2,
                list.y,
                "Working tree clean · nothing to inspect",
                t.muted().bg(bg),
            );
        }
        for (k, i) in self.list_scroll.visible_range().enumerate() {
            let y = list.y + k as u16;
            let rid = self.id.sub("row").child(i);
            let mut s = ctx.state(rid);
            s.focused = focused && i == self.cursor;
            s.selected = i == self.cursor;
            let st = t.row(s, bg);
            let rect = Rect::new(list.x, y, row_w, 1);
            fill(buf, rect, st);
            buf.set_string(
                rect.x,
                y,
                t.gutter_symbol(s),
                t.gutter(s, st.bg.unwrap_or(bg), false),
            );
            if i < self.listed.len() {
                let f = &self.listed[i];
                buf.set_string(
                    rect.x + 2,
                    y,
                    f.status.marker(),
                    st.fg(t.tone(f.status.tone())).add_modifier(Modifier::BOLD),
                );
                let counts = if i < self.changes.files.len() {
                    format!("+{} −{}", f.additions(), f.deletions())
                } else {
                    self.words.unread.clone()
                };
                let cw = width(&counts) as u16;
                let pw = rect.width.saturating_sub(6 + cw + 2) as usize;
                let path = match &f.status {
                    DiffStatus::Renamed { from } => format!("{from} → {}", f.path),
                    _ => f.path.clone(),
                };
                buf.set_string(
                    rect.x + 4,
                    y,
                    fit(&truncate_middle(&path, pw), pw),
                    if s.selected {
                        st.fg(t.text_primary)
                    } else {
                        st
                    },
                );
                buf.set_string(
                    rect.right().saturating_sub(cw + 1),
                    y,
                    &counts,
                    st.fg(t.text_muted).remove_modifier(Modifier::BOLD),
                );
            } else {
                let u = self.changes.unpushed;
                buf.set_string(rect.x + 2, y, "↑", st.fg(t.warning));
                buf.set_string(
                    rect.x + 4,
                    y,
                    format!("{u} commit{} not pushed", if u == 1 { "" } else { "s" }),
                    st.fg(t.warning),
                );
            }

            legible_on_fill(buf, rect, t, bg);
            ctx.clickable(rid, rect);
        }
        if has_sb {
            scrollbar::render_with_fade(
                Rect::new(list.x, list.y, list.width, list.height),
                buf,
                ctx,
                self.id,
                &self.list_scroll,
                focused,
            );
        }
    }

    fn draw_advanced(
        &mut self,
        inner: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        let t = ctx.theme;
        let summary = self.summary_line();
        buf.set_string(inner.x, inner.y, &summary, t.secondary().bg(bg));
        let mode = format!(
            "{} · {} · {VIEW_KEY} switches · m compact",
            self.mode.label(),
            self.diff.mode.label()
        );
        let mw = width(&mode) as u16;
        if inner.width > width(&summary) as u16 + mw + 3 {
            buf.set_string(inner.right() - mw, inner.y, &mode, t.faint().bg(bg));
        }
        let body = Rect::new(
            inner.x,
            inner.y + 2,
            inner.width,
            inner.height.saturating_sub(2),
        );
        self.container = body;

        self.stacked = body.width < 90;
        let (tree_r, diff_r) = if self.stacked {
            let th = (body.height * 35 / 100).clamp(4, 10);
            (
                Rect::new(body.x, body.y, body.width, th),
                Rect::new(
                    body.x,
                    body.y + th + 1,
                    body.width,
                    body.height.saturating_sub(th + 1),
                ),
            )
        } else {
            self.split.horizontal(body, 1)
        };
        self.tree_area = tree_r;
        self.diff_area = diff_r;
        self.tree.render(tree_r, buf, ctx, bg);
        if self.stacked {
            let y = tree_r.bottom();
            for x in body.left()..body.right() {
                buf.set_string(
                    x,
                    y,
                    t.glyphs.h_divider,
                    Style::new().fg(t.border_subtle).bg(bg),
                );
            }
        } else {
            let handle = self.split.handle(SplitDir::Horizontal, body, 1);
            self.seam.render(handle, buf, ctx, bg);
        }
        self.diff.render(diff_r, buf, ctx, bg);
        if let Some(u) = Some(self.changes.unpushed).filter(|u| *u > 0)
            && diff_r.height > 2
        {
            let text = format!("↑ {u} commit{} not pushed", if u == 1 { "" } else { "s" });
            let y = diff_r.bottom() - 1;
            let w = width(&text) as u16;
            buf.set_string(
                diff_r.right().saturating_sub(w),
                y,
                &text,
                Style::new().fg(t.warning).bg(bg),
            );
        }
    }
}

fn row_index(tree: &TreeView, i: usize, leaves: &[(Vec<usize>, usize)]) -> Option<usize> {
    let (path, _) = leaves.iter().find(|(_, fi)| *fi == i)?;
    tree.rows().iter().position(|r| &r.path == path)
}

fn listed_files(changes: &ChangeSet) -> Vec<ChangedFile> {
    changes
        .files
        .iter()
        .cloned()
        .chain(changes.unread.iter().map(|u| ChangedFile {
            path: u.path.clone(),
            status: u.status.clone(),
            hunks: vec![],
        }))
        .collect()
}

fn build_tree(
    files: &[ChangedFile],
    read: usize,
    unread_word: &str,
) -> (Vec<TreeNode>, Vec<(Vec<usize>, usize)>) {
    #[derive(Default)]
    struct Dir {
        dirs: std::collections::BTreeMap<String, Dir>,
        files: Vec<(String, usize)>,
    }
    let mut root = Dir::default();
    for (i, f) in files.iter().enumerate() {
        let mut parts: Vec<&str> = f.path.split('/').collect();
        let name = parts.pop().unwrap_or("").to_owned();
        let mut d = &mut root;
        for p in parts {
            d = d.dirs.entry(p.to_owned()).or_default();
        }
        d.files.push((name, i));
    }
    fn count(d: &Dir) -> usize {
        d.files.len() + d.dirs.values().map(count).sum::<usize>()
    }
    fn emit(
        d: &Dir,
        files: &[ChangedFile],
        read: usize,
        unread_word: &str,
        path: &mut Vec<usize>,
        leaves: &mut Vec<(Vec<usize>, usize)>,
    ) -> Vec<TreeNode> {
        let mut out = vec![];
        for (name, sub) in &d.dirs {
            path.push(out.len());
            let children = emit(sub, files, read, unread_word, path, leaves);
            path.pop();
            let n = count(sub);
            out.push(TreeNode::dir(name, children).meta(&format!("{n} changed")));
        }
        for (name, i) in &d.files {
            path.push(out.len());
            leaves.push((path.clone(), *i));
            path.pop();
            let f = &files[*i];
            let meta = if *i < read {
                format!("+{} −{}", f.additions(), f.deletions())
            } else {
                unread_word.to_owned()
            };
            out.push(TreeNode::leaf_meta(name, &meta).glyph(f.status.marker()));
        }
        out
    }
    let mut leaves = vec![];
    let nodes = emit(&root, files, read, unread_word, &mut vec![], &mut leaves);
    (nodes, leaves)
}

fn view_hint(mode: crate::widgets::diff::DiffMode) -> Hint {
    crate::widgets::keyhint::toggle_hint(
        VIEW_KEY,
        mode == crate::widgets::diff::DiffMode::Review,
        "Show unified",
        "Show side by side",
    )
}

impl InspectChanges {
    pub fn initial_focus(&self) -> WidgetId {
        match self.mode {
            InspectMode::Compact => self.id,
            InspectMode::Advanced => self.tree.id,
        }
    }
}

impl<R: From<InspectOutcome> + 'static, W: 'static> Layer<R, W> for InspectChanges {
    fn animating(&self) -> bool {
        true
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::modal(Dismiss::LIGHT).initial_focus(self.initial_focus())
    }

    fn on_dismiss(&mut self, _: DismissReason) {
        self.result = Some(self.outcome());
    }

    fn on_key(&mut self, key: &Key, cx: &mut LayerCx<'_, W>) -> Outcome {
        let focus = &mut *cx.focus;
        match key.code {
            KeyCode::Char(VIEW_CHAR) if key.plain() => {
                self.diff.toggle_mode();
                return Outcome::Changed;
            }
            KeyCode::Char('m') | KeyCode::F(2) if key.plain() => {
                self.set_mode(self.mode.toggled(), focus);
                return Outcome::Changed;
            }
            _ => {}
        }
        match self.mode {
            InspectMode::Compact => self.compact_key(key, focus),
            InspectMode::Advanced => self.advanced_key(key, focus),
        }
    }

    fn pointer(&mut self, ev: &PointerEvent, cx: &mut LayerCx<'_, W>) -> Outcome {
        let focus = &mut *cx.focus;
        let (id, pos) = (ev.id, ev.pos());
        if let Some((axis, delta)) = ev.wheel() {
            return self.wheel_at(axis, delta, pos);
        }
        let click = ev.is(Phase::Click);
        if ev.is(Phase::Drag) && id == self.seam.id && !self.stacked {
            return self.seam.on_drag(&mut self.split, self.container, 1, pos);
        }
        let diff_live = self.mode == InspectMode::Advanced || self.open;
        if diff_live && let Some(r) = self.diff.route(ev) {
            if self.mode == InspectMode::Advanced && (click || ev.is(Phase::Press)) {
                self.set_region(Region::Diff, focus);
            }
            return r.outcome();
        }
        if self.mode == InspectMode::Advanced
            && let Some(r) = self.tree.route(ev)
        {
            if click {
                self.set_region(Region::Tree, focus);
            }
            let (o, actions) = r.into_parts();
            for action in actions {
                if let TreeAction::Activate(path) = action
                    && let Some(i) = self.file_of_path(&path)
                {
                    self.select_file(i);
                }
            }
            return if click { o.or(Outcome::Changed) } else { o };
        }
        if self.mode == InspectMode::Compact
            && !self.open
            && let Some(o) =
                scrollbar::pointer(ev, self.id, &mut self.list_scroll, TrackClick::Jump)
        {
            return o;
        }
        if !click {
            return Outcome::Ignored;
        }
        if self.mode == InspectMode::Compact && !self.open {
            for i in 0..self.compact_rows() {
                if self.id.sub("row").child(i) == id {
                    let same = self.cursor == i;
                    self.cursor = i;
                    focus.focus(self.id);
                    if same && i < self.changes.files.len() {
                        self.select_file(i);
                        self.open = true;
                        focus.focus(self.diff.id());
                    }
                    return Outcome::Changed;
                }
            }
        }
        Outcome::Consumed
    }

    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _w: &W) {
        let w = (screen.width * 9 / 10).max(MIN_W.min(screen.width.saturating_sub(2)));
        let h = (screen.height * 9 / 10).max(MIN_H.min(screen.height.saturating_sub(2)));
        let meta = match self.mode {
            InspectMode::Compact if self.open => "Esc back to the list".to_owned(),
            InspectMode::Compact => "m advanced view".to_owned(),
            InspectMode::Advanced => "Tab tree / diff".to_owned(),
        };
        let dismiss = Layer::<R, W>::spec(self).dismiss;
        let (area, inner) = modal_frame(screen, buf, ctx, w, h, &self.title, Some(&meta), dismiss);
        self.area = area;
        let bg = ctx.theme.surface_elevated;
        match self.mode {
            InspectMode::Compact => self.draw_compact(inner, buf, ctx, bg),
            InspectMode::Advanced => self.draw_advanced(inner, buf, ctx, bg),
        }
    }

    fn take_result(&mut self) -> Option<LayerOut<R>> {
        self.result.take().map(|o| LayerOut::Close(R::from(o)))
    }

    fn hints(&self, _: Option<WidgetId>) -> Vec<Hint> {
        let back = if self.returns_to_exit {
            "Back"
        } else {
            "Close"
        };
        match self.mode {
            InspectMode::Compact if self.open => vec![
                hint("↑↓", "Scroll"),
                hint("drag", "Select"),
                hint("y", "Copy"),
                view_hint(self.diff.mode),
                hint("Esc", "Back to list"),
            ],
            InspectMode::Compact => vec![
                hint("↑↓", "Move"),
                hint("Enter", "Open diff"),
                view_hint(self.diff.mode),
                hint("m", "Advanced"),
                hint("Esc", back),
            ],
            InspectMode::Advanced => match self.region {
                Region::Tree => vec![
                    hint("↑↓", "Move"),
                    hint("←→", "Fold"),
                    hint("Tab", "Diff"),
                    view_hint(self.diff.mode),
                    hint("m", "Compact"),
                    hint("Esc", back),
                ],
                Region::Diff => vec![
                    hint("↑↓", "Scroll"),
                    hint("drag", "Select"),
                    hint("y", "Copy"),
                    hint("Tab", "Tree"),
                    view_hint(self.diff.mode),
                    hint("Esc", "Back to tree"),
                ],
            },
        }
    }
}

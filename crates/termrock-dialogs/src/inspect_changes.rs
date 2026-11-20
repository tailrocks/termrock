use termrock_core::event::{Key, Outcome};
use termrock_core::focus::Focus;
use termrock_core::hit::Axis;
use termrock_core::id::WidgetId;
use termrock_core::intent::Phase;
use termrock_core::pointer::{MouseTarget, PointerEvent};
use termrock_core::scroll::{ScrollState, TrackClick};
use termrock_layout::layout::{Split, SplitDir};
use termrock_runtime::ctx::{RenderCtx, fill, legible_on_fill};
use termrock_text::cells::{fit, paint_clipped, paint_truncated, truncate_middle, width};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use termrock_collections::tree::{TreeAction, TreeNode, TreeView};
use termrock_controls::keyhint::{Hint, hint};
use termrock_controls::scrollbar;
use termrock_editors::diff::DiffView;
use termrock_layout::splitter::Splitter;

use termrock_core::scroll::Scrollable;
use termrock_editors::diff::{DiffFile as ChangedFile, DiffStatus, UnreadFile};
use termrock_overlays::layer::{DismissReason, Layer, LayerCx, LayerOut, LayerSpec};
use termrock_overlays::modal::modal_frame;
use termrock_runtime::popup::Dismiss;

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

    fn forget_geometry(&mut self) {
        self.tree_area = Rect::ZERO;
        self.diff_area = Rect::ZERO;
        self.container = Rect::ZERO;
    }

    /// The summary line on `inner`'s first row, with `mode` right-aligned
    /// when both fit.
    fn paint_header(buf: &mut Buffer, inner: Rect, summary: &str, mode: &str, ink: (Style, Style)) {
        let top = Rect::new(inner.x, inner.y, inner.width, 1);
        paint_truncated(buf, top, 0, summary, ink.0);
        let mw = width(mode);
        if usize::from(inner.width) > width(summary) + mw + 3 {
            paint_clipped(buf, top, usize::from(inner.width) - mw, mode, ink.1);
        }
    }

    /// `inner` below its first `rows` rows.
    fn below(inner: Rect, rows: u16) -> Rect {
        let rows = rows.min(inner.height);
        Rect::new(inner.x, inner.y + rows, inner.width, inner.height - rows)
    }

    fn draw_compact(
        &mut self,
        inner: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        let t = ctx.theme;
        self.forget_geometry();
        let inner = inner.intersection(*buf.area());
        if inner.is_empty() {
            self.list_scroll.set_viewport(0);
            return;
        }
        if self.open {
            let Some(f) = self.diff.file().cloned() else {
                return;
            };
            let head = format!("{}  {}", f.header(), f.summary());
            let top = Rect::new(inner.x, inner.y, inner.width, 1);
            paint_clipped(
                buf,
                top,
                0,
                &termrock_runtime::reveal::cut(top, &head),
                t.secondary().bg(bg),
            );
            let mode = format!("{} · {VIEW_KEY} switches", self.diff.mode.label());
            let mw = width(&mode);
            if usize::from(inner.width) > width(&head) + mw + 3 {
                paint_clipped(
                    buf,
                    top,
                    usize::from(inner.width) - mw,
                    &mode,
                    t.faint().bg(bg),
                );
            }
            let body = Self::below(inner, 2);
            self.diff_area = body;
            if !body.is_empty() {
                self.diff.render(body, buf, ctx, bg);
            }
            return;
        }
        let summary = self.summary_line();
        let mode = format!("{} · {}", self.mode.label(), self.diff.mode.label());
        Self::paint_header(
            buf,
            inner,
            &summary,
            &mode,
            (t.secondary().bg(bg), t.faint().bg(bg)),
        );
        let list = Self::below(inner, 2);
        let n = self.compact_rows();
        self.list_scroll.set_content(n);
        self.list_scroll.set_viewport(usize::from(list.height));
        if list.is_empty() {
            return;
        }
        ctx.control(self.id, list, false);
        ctx.scrollable(self.id, list);
        let has_sb = self.list_scroll.overflows();
        let row_w = list.width.saturating_sub(u16::from(has_sb));
        let focused = ctx.interaction.focused(self.id);
        if n == 0 {
            let first = Rect::new(list.x, list.y, list.width, 1);
            paint_truncated(
                buf,
                first,
                2,
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
            paint_clipped(
                buf,
                rect,
                0,
                t.gutter_symbol(s),
                t.gutter(s, st.bg.unwrap_or(bg), false),
            );
            if i < self.listed.len() {
                let f = &self.listed[i];
                paint_clipped(
                    buf,
                    rect,
                    2,
                    f.status.marker(),
                    st.fg(t.tone(f.status.tone())).add_modifier(Modifier::BOLD),
                );
                let counts = if i < self.changes.files.len() {
                    format!("+{} −{}", f.additions(), f.deletions())
                } else {
                    self.words.unread.clone()
                };
                let cw = width(&counts);
                let pw = usize::from(rect.width).saturating_sub(6 + cw + 2);
                let path = match &f.status {
                    DiffStatus::Renamed { from } => format!("{from} → {}", f.path),
                    _ => f.path.clone(),
                };
                paint_clipped(
                    buf,
                    rect,
                    4,
                    &fit(&truncate_middle(&path, pw), pw),
                    if s.selected {
                        st.fg(t.text_primary)
                    } else {
                        st
                    },
                );
                paint_clipped(
                    buf,
                    rect,
                    usize::from(rect.width).saturating_sub(cw + 1),
                    &counts,
                    st.fg(t.text_muted).remove_modifier(Modifier::BOLD),
                );
            } else {
                let u = self.changes.unpushed;
                paint_clipped(buf, rect, 2, "↑", st.fg(t.warning));
                paint_truncated(
                    buf,
                    rect,
                    4,
                    &format!("{u} commit{} not pushed", if u == 1 { "" } else { "s" }),
                    st.fg(t.warning),
                );
            }

            legible_on_fill(buf, rect, t, bg);
            if !rect.is_empty() {
                ctx.clickable(rid, rect);
            }
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
        self.forget_geometry();
        let inner = inner.intersection(*buf.area());
        if inner.is_empty() {
            return;
        }
        let summary = self.summary_line();
        let mode = format!(
            "{} · {} · {VIEW_KEY} switches · m compact",
            self.mode.label(),
            self.diff.mode.label()
        );
        Self::paint_header(
            buf,
            inner,
            &summary,
            &mode,
            (t.secondary().bg(bg), t.faint().bg(bg)),
        );
        let body = Self::below(inner, 2);
        if body.is_empty() {
            return;
        }
        self.container = body;

        self.stacked = body.width < 90;
        let (tree_r, gap_r, diff_r) = if self.stacked {
            let (th, gap, dh) = stacked_heights(body.height);
            (
                Rect::new(body.x, body.y, body.width, th),
                Rect::new(body.x, body.y + th, body.width, gap),
                Rect::new(body.x, body.y + th + gap, body.width, dh),
            )
        } else {
            let (tree_r, diff_r) = self.split.horizontal(body, 1);
            (tree_r, Rect::ZERO, diff_r)
        };
        self.tree_area = tree_r;
        self.diff_area = diff_r;
        if !tree_r.is_empty() {
            self.tree.render(tree_r, buf, ctx, bg);
        }
        if self.stacked {
            for x in gap_r.left()..gap_r.right() {
                paint_clipped(
                    buf,
                    Rect::new(x, gap_r.y, 1, 1),
                    0,
                    t.glyphs.h_divider,
                    Style::new().fg(t.border_subtle).bg(bg),
                );
            }
        } else {
            let handle = self.split.handle(SplitDir::Horizontal, body, 1);
            self.seam.render(handle, buf, ctx, bg);
        }
        if diff_r.is_empty() {
            return;
        }
        self.diff.render(diff_r, buf, ctx, bg);
        if let Some(u) = Some(self.changes.unpushed).filter(|u| *u > 0)
            && diff_r.height > 2
        {
            let text = format!("↑ {u} commit{} not pushed", if u == 1 { "" } else { "s" });
            let last = Rect::new(diff_r.x, diff_r.bottom() - 1, diff_r.width, 1);
            let col = usize::from(diff_r.width).saturating_sub(width(&text));
            paint_truncated(buf, last, col, &text, Style::new().fg(t.warning).bg(bg));
        }
    }
}

/// Splits `available` rows into tree, divider and diff heights. The tree
/// prefers 35% of the rows (4 to 10) but never more than exist. A divider row
/// follows it when any row is left over and the diff gets what remains.
fn stacked_heights(available: u16) -> (u16, u16, u16) {
    if available == 0 {
        return (0, 0, 0);
    }
    let preferred = (u32::from(available) * 35 / 100).clamp(4, 10) as u16;
    let tree = preferred.min(available);
    let left = available - tree;
    let gap = u16::from(left > 0);
    (tree, gap, left - gap)
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

fn view_hint(mode: termrock_editors::diff::DiffMode) -> Hint {
    termrock_controls::keyhint::toggle_hint(
        VIEW_KEY,
        mode == termrock_editors::diff::DiffMode::Review,
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
        let dismissal = if id == termrock_overlays::modal::CLOSE {
            Some(DismissReason::CloseButton)
        } else if id == termrock_overlays::modal::BACKDROP {
            Some(DismissReason::OutsideClick)
        } else {
            None
        };
        if click && let Some(reason) = dismissal {
            if Layer::<R, W>::spec(self).dismiss.allows(reason) {
                Layer::<R, W>::on_dismiss(self, reason);
                return Outcome::Changed;
            }
            return Outcome::Consumed;
        }
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
        let screen = screen.intersection(*buf.area());
        if screen.is_empty() {
            self.area = Rect::ZERO;
            self.forget_geometry();
            return;
        }
        let share = |extent: u16, min: u16| {
            let wanted = u32::from(extent) * 9 / 10;
            u16::try_from(wanted).map_or(extent, |w| w.max(min.min(extent.saturating_sub(2))))
        };
        let (w, h) = (share(screen.width, MIN_W), share(screen.height, MIN_H));
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

#[cfg(test)]
mod tests {
    use super::stacked_heights;

    #[test]
    fn inspect_composite_stacked_heights_degrade_inside_the_parent() {
        for available in 0..=40u16 {
            let (tree, gap, diff) = stacked_heights(available);
            assert_eq!(tree + gap + diff, available, "rows add up at {available}");
            assert!(tree <= available, "the tree never exceeds the parent");
            assert!(gap <= 1, "one divider row at most");
        }
        assert_eq!(stacked_heights(1), (1, 0, 0));
        assert_eq!(stacked_heights(4), (4, 0, 0));
        assert_eq!(stacked_heights(5), (4, 1, 0));
        assert_eq!(stacked_heights(30), (10, 1, 19));
    }
}

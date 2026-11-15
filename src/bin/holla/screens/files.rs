use crate::screens::Holla;
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::hit::Axis;
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::scroll::ScrollState;
use junie_tui::core::selection::ClickPolicy;
use junie_tui::shell::Page as ShellPage;
use junie_tui::theme::Tone;
use junie_tui::ui::ctx::{RenderCtx, fill, legible_on_fill};
use junie_tui::ui::master_detail::MasterDetail;
use junie_tui::ui::text::{fit, fit_right, truncate, truncate_middle, width};
use junie_tui::widgets::empty::{self, EmptyState};
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::menu::{ContextMenu, MenuItem, Placement};
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::page_header::Heading;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::picker::{Picker, PickerStatus};
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::statusbar::StatusItem;
use junie_tui::widgets::viewport::{Mark, Span, TextViewport, ViewportAction};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

use crate::domain::exec::Command;
use crate::screens::{Cx, Go, ModalResult, Screen, StatusBits, scroll_drag, scroll_press};
use crate::sim::fs::{FindHit, Fs, NodeKind, Preview};
use crate::sim::world::{SourceState, World};
use junie_tui::ui::layer::LayerTag;
use junie_tui::ui::text::plural;

pub const LIST: WidgetId = WidgetId::of("files.list");
pub const PREVIEW: WidgetId = WidgetId::of("files.preview");
pub const QUERY: WidgetId = WidgetId::of("files.query");
pub const JUMP_MAX: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    path: String,
    name: String,
    kind: &'static str,
    is_dir: bool,
    size: u64,
    mtime: i64,
    hidden: bool,

    error: Option<String>,

    lossy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Browse,
    Find,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Pending {
    dir: String,
    generation: u64,
    due: u64,
}

pub struct FilesPage {
    mode: Mode,
    pub dir: String,
    entries: Vec<Entry>,
    listing_error: Option<String>,
    pub cursor: usize,
    scroll: ScrollState,
    pub show_hidden: bool,

    generation: u64,
    pending: Option<Pending>,

    remembered: Vec<(String, String)>,
    preview: TextViewport,
    preview_for: Option<(String, u64)>,
    preview_title: String,
    preview_meta: String,
    preview_lines: Vec<String>,
    preview_err: Option<String>,

    md: MasterDetail,

    pub query: String,
    index: Vec<String>,
    hits: Vec<FindHit>,

    find_query: Option<String>,
    find_matches: Vec<(usize, std::ops::Range<usize>)>,
    find_at: usize,
    last_tick: u64,
    list_area: Rect,

    jump_error: Option<String>,
}

impl FilesPage {
    pub fn new(path: &str, query: Option<&str>, w: &World) -> Self {
        let mut p = Self::blank(Mode::Browse);

        let (dir, highlight) = if w.fs.is_dir(path) {
            (path.to_owned(), None)
        } else if w.fs.exists(path) {
            (
                Fs::parent(path).unwrap_or("/".into()),
                Some(path.to_owned()),
            )
        } else {
            p.listing_error = Some(format!("{path}: no such file or directory"));
            (w.location.cwd.clone(), None)
        };
        p.dir = dir;
        if let Some(h) = highlight {
            p.remembered.push((p.dir.clone(), h));
        }
        p.query = query.unwrap_or("").to_owned();
        p
    }

    pub fn find(w: &World) -> Self {
        let mut p = Self::blank(Mode::Find);
        p.dir = w.location.home.clone();
        p
    }

    fn blank(mode: Mode) -> Self {
        Self {
            mode,
            dir: String::new(),
            entries: vec![],
            listing_error: None,
            cursor: 0,
            scroll: ScrollState::default(),
            show_hidden: false,
            generation: 0,
            pending: None,
            remembered: vec![],
            preview: TextViewport::new(PREVIEW).wrap(false),
            preview_for: None,
            preview_title: String::new(),
            preview_meta: String::new(),
            preview_lines: vec![],
            preview_err: None,
            md: MasterDetail::new(LIST, PREVIEW).detail_share(44, 36, 70),
            query: String::new(),
            index: vec![],
            hits: vec![],
            find_query: None,
            find_matches: vec![],
            find_at: 0,
            last_tick: u64::MAX,
            list_area: Rect::ZERO,
            jump_error: None,
        }
    }

    fn entry_of(&self, node: &crate::sim::fs::Node, w: &World) -> Entry {
        let (kind, error) = match &node.kind {
            NodeKind::Symlink { target } => {
                if w.fs.exists(target) {
                    ("symlink", None)
                } else {
                    ("symlink", Some(format!("broken link → {target}")))
                }
            }
            NodeKind::Dir if !node.readable => ("directory", Some("permission denied".into())),
            NodeKind::Dir => ("directory", None),
            NodeKind::File => ("file", None),
            NodeKind::Fifo => ("fifo", Some("special file".into())),
            NodeKind::Device => ("device", Some("special file".into())),
        };
        Entry {
            path: node.path.clone(),
            name: node.name().to_owned(),
            kind,
            is_dir: node.is_dir()
                || matches!(&node.kind, NodeKind::Symlink { target } if w.fs.is_dir(target)),
            size: node.apparent,
            mtime: node.mtime,
            hidden: node.hidden(),
            error,
            lossy: false,
        }
    }

    fn navigate(&mut self, dir: &str, w: &World, cx: &mut Cx) {
        self.generation += 1;
        self.dir = dir.to_owned();
        self.listing_error = None;
        let latency = w
            .fs_latency
            .iter()
            .find(|(p, _)| p == dir)
            .map(|(_, t)| *t)
            .unwrap_or(0);
        if latency > 0 {
            self.pending = Some(Pending {
                dir: dir.into(),
                generation: self.generation,
                due: w.tick + latency,
            });
            self.entries.clear();
            cx.status(format!("Listing {} …", w.location.short(dir)));
            return;
        }
        self.pending = None;
        self.apply_listing(dir, self.generation, w);
    }

    fn apply_listing(&mut self, dir: &str, generation: u64, w: &World) {
        if generation != self.generation || dir != self.dir {
            return;
        }
        match w.fs.list(dir) {
            Ok(nodes) => {
                let mut entries: Vec<Entry> = nodes.iter().map(|n| self.entry_of(n, w)).collect();

                let mut seen: std::collections::BTreeMap<String, usize> = Default::default();
                for e in &entries {
                    let key = unicode_normalize(&e.name);
                    *seen.entry(key).or_insert(0) += 1;
                }
                for e in &mut entries {
                    if seen.get(&unicode_normalize(&e.name)).copied().unwrap_or(0) > 1 {
                        e.lossy = true;
                    }
                }
                self.entries = entries;
                self.listing_error = None;
            }
            Err(e) => {
                self.entries.clear();
                self.listing_error = Some(e.message());
            }
        }

        let target = self
            .remembered
            .iter()
            .rev()
            .find(|(d, _)| d == dir)
            .map(|(_, p)| p.clone());
        self.cursor = target
            .and_then(|p| self.visible().iter().position(|e| e.path == p))
            .unwrap_or(0);
        self.scroll.jump_start();
        self.scroll.ensure_visible(self.cursor);
        self.preview_for = None;
    }

    fn visible(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| self.show_hidden || !e.hidden)
            .collect()
    }

    fn current(&self) -> Option<Entry> {
        self.visible().get(self.cursor).map(|e| (*e).clone())
    }

    fn remember(&mut self) {
        if let Some(e) = self.current() {
            self.remembered.retain(|(d, _)| d != &self.dir);
            self.remembered.push((self.dir.clone(), e.path));
        }
    }

    fn go_parent(&mut self, w: &World, cx: &mut Cx) {
        let Some(parent) = Fs::parent(&self.dir) else {
            cx.status("Already at the root");
            return;
        };
        let leaving = self.dir.clone();
        self.remember();

        self.remembered.retain(|(d, _)| d != &parent);
        self.remembered.push((parent.clone(), leaving));
        self.navigate(&parent, w, cx);
    }

    fn open_current(&mut self, w: &World, cx: &mut Cx) -> Outcome {
        let Some(e) = self.current() else {
            return Outcome::Consumed;
        };
        if let Some(err) = &e.error {
            cx.error(format!("{} · {err}", e.name));
            return Outcome::Changed;
        }
        if e.is_dir {
            self.remember();
            let target = match w.fs.get(&e.path).map(|n| &n.kind) {
                Some(NodeKind::Symlink { target }) => target.clone(),
                _ => e.path.clone(),
            };
            self.navigate(&target, w, cx);
            return Outcome::Changed;
        }
        self.load_preview(&e.path, w);
        self.md.open_drawer(cx.focus);
        Outcome::Changed
    }

    fn load_preview(&mut self, path: &str, w: &World) {
        let key = (path.to_owned(), self.generation);
        if self.preview_for.as_ref() == Some(&key) {
            return;
        }
        self.preview_for = Some(key);
        self.find_query = None;
        self.find_matches.clear();
        self.preview.clear_marks();
        let short = w.location.short(path);
        let (title, meta, lines, err): (String, String, Vec<String>, Option<String>) =
            match w.fs.preview(path) {
                Preview::Text {
                    lines,
                    bytes_shown,
                    total_bytes,
                    truncated_bytes,
                    truncated_lines,
                    long_lines,
                    link,
                } => {
                    let mut m = vec![];
                    if let Some(l) = &link {
                        m.push(format!("link → {}", w.location.short(l)));
                    }
                    if truncated_bytes {
                        m.push("first 256 KiB".into());
                    }
                    if truncated_lines {
                        m.push("first 2000 lines".into());
                    }
                    if long_lines > 0 {
                        m.push(format!("{long_lines} long lines cut at 4096"));
                    }
                    m.push(format!(
                        "{} of {}",
                        junie_tui::ui::text::bytes(bytes_shown as u64),
                        junie_tui::ui::text::bytes(total_bytes)
                    ));
                    (crate::sim::fs::sanitize(&short), m.join(" · "), lines, None)
                }
                Preview::Empty => (short, "empty file".into(), vec![], None),
                Preview::Binary { reason } => (
                    short,
                    "binary".into(),
                    vec![],
                    Some(format!("binary · {reason} · not shown")),
                ),
                Preview::Directory => (
                    short,
                    "directory".into(),
                    vec![],
                    Some("a directory · Enter opens it".into()),
                ),
                Preview::Error(e) => (short, "error".into(), vec![], Some(e.message())),
            };
        self.preview_title = title;
        self.preview_meta = meta;
        self.preview_err = err;
        let vp: Vec<Vec<Span>> = lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                vec![
                    Span::muted(format!("{:>5} ", i + 1)),
                    Span::plain(l.clone()),
                ]
            })
            .collect();
        self.preview_lines = lines;
        self.preview.set_lines(vp);
        self.preview.set_follow(false);
        self.preview.scroll.jump_start();
        self.preview.scroll.h.jump_start();
    }

    fn run_find(&mut self) {
        let q = self.find_query.clone().unwrap_or_default();
        self.find_matches.clear();
        if q.is_empty() {
            self.preview.clear_marks();
            return;
        }
        for (i, l) in self.preview_lines.iter().enumerate() {
            for r in junie_tui::ui::text::find_ranges(l, &q, false) {
                self.find_matches.push((i, r));
            }
        }
        self.find_at = 0;
        self.apply_marks();
    }

    fn apply_marks(&mut self) {
        let prefix = 6;
        let marks: Vec<Mark> = self
            .find_matches
            .iter()
            .enumerate()
            .map(|(k, (line, r))| Mark {
                line: *line,
                range: r.start + prefix..r.end + prefix,
                current: k == self.find_at,
            })
            .collect();
        self.preview.set_marks(marks);
        if let Some((line, _)) = self.find_matches.get(self.find_at) {
            self.preview.reveal_line(*line);
        }
    }

    fn indexed(&self, w: &World) -> (usize, bool) {
        match w.source_state("index") {
            SourceState::Loading => {
                let done_at = w
                    .sources
                    .iter()
                    .find(|s| s.name == "index")
                    .map(|s| s.done_at)
                    .unwrap_or(1)
                    .max(1);
                let frac = w.tick as f64 / done_at as f64;
                (((self.index.len() as f64) * frac) as usize, false)
            }
            _ => (self.index.len(), true),
        }
    }

    fn refresh_hits(&mut self, w: &World) {
        let (n, _) = self.indexed(w);
        let slice: Vec<String> = self.index.iter().take(n).cloned().collect();
        let keep = self.hits.get(self.cursor).map(|h| h.path.clone());
        self.hits = w.fs.find(&slice, &self.query);
        self.cursor = keep
            .and_then(|p| self.hits.iter().position(|h| h.path == p))
            .unwrap_or(0);
        self.scroll.set_content(self.hits.len());
        self.scroll.ensure_visible(self.cursor);
    }

    fn selected_path(&self) -> Option<String> {
        self.path_at(self.cursor)
    }

    fn path_at(&self, i: usize) -> Option<String> {
        match self.mode {
            Mode::Browse => self.visible().get(i).map(|e| e.path.clone()),
            Mode::Find => self.hits.get(i).map(|h| h.path.clone()),
        }
    }

    fn open_actions(&mut self, cx: &mut Cx) -> Outcome {
        let Some(path) = self.selected_path() else {
            cx.status("Select a file or folder first");
            return Outcome::Changed;
        };
        let anchor = Rect::new(
            self.list_area.x + 4,
            self.list_area.y + (self.cursor.saturating_sub(self.scroll.offset)) as u16,
            1,
            1,
        );
        self.open_actions_at(path, anchor, cx)
    }

    fn open_actions_at(&mut self, path: String, anchor: Rect, cx: &mut Cx) -> Outcome {
        let items = vec![
            MenuItem::new("Open in the OS").shortcut("Enter"),
            MenuItem::new("Reveal in the file manager"),
            MenuItem::new("Copy full path").shortcut("y"),
            MenuItem::new("Analyze disk usage").separator(),
            MenuItem::new("Preview here"),
        ];
        let m = ContextMenu::new(WidgetId::of("files.actions"), items)
            .anchor(anchor, Placement::Below)
            .title(truncate_middle(path.as_str(), 40));
        cx.open(m, LayerTag::new("file-actions").key(path));
        Outcome::Changed
    }

    fn file_action(&mut self, i: usize, path: &str, w: &World, cx: &mut Cx) {
        let host = w.host.name.clone();
        let cwd = w.location.cwd.clone();
        let is_dir = w.fs.is_dir(path);
        match i {
            0 => {
                let Some(opener) = w.platform.opener.clone() else {
                    cx.error(format!(
                        "No opener on {} · install xdg-open or open the path yourself: {path}",
                        w.host.name
                    ));
                    return;
                };
                cx.go(Go::Exec {
                    label: format!("open {}", path.rsplit('/').next().unwrap_or(path)),
                    argv: vec![Command::argv(&opener, &[path], &cwd, &host)],
                });
            }
            1 => {
                let Some(opener) = w.platform.opener.clone() else {
                    cx.error(format!("No opener on {} · nothing revealed", w.host.name));
                    return;
                };
                let argv = if opener == "open" {
                    Command::argv("open", &["-R", path], &cwd, &host)
                } else {
                    let parent = Fs::parent(path).unwrap_or("/".into());
                    Command::argv(&opener, &[&parent], &cwd, &host)
                };
                cx.go(Go::Exec {
                    label: format!("reveal {}", path.rsplit('/').next().unwrap_or(path)),
                    argv: vec![argv],
                });
            }
            2 => cx.go(Go::Osc52(path.to_owned())),
            3 => {
                let root = if is_dir {
                    path.to_owned()
                } else {
                    Fs::parent(path).unwrap_or("/".into())
                };
                cx.go(Go::Analyze(root));
            }
            _ => {
                if is_dir {
                    self.mode = Mode::Browse;
                    self.navigate(path, w, cx);
                } else {
                    self.load_preview(path, w);
                    self.md.open_drawer(cx.focus);
                }
            }
        }
    }

    fn jump_suggestions(&self, query: &str, w: &World) -> Vec<Item> {
        let q = query.trim();
        let expanded = expand(q, &self.dir, &w.location.home);
        let mut items: Vec<Item> = vec![];
        let mut seen = std::collections::BTreeSet::new();
        if !q.is_empty() && w.fs.exists(&expanded) {
            seen.insert(expanded.clone());
            items.push(
                Item::new(w.location.short(&expanded))
                    .detail("exact path")
                    .group("exact")
                    .key(ItemKey::text(&expanded))
                    .insert(expanded.clone()),
            );
        }

        if !q.is_empty() {
            let (dir, prefix) = match expanded.rsplit_once('/') {
                Some((d, p)) if w.fs.is_dir(if d.is_empty() { "/" } else { d }) => (
                    if d.is_empty() {
                        "/".to_owned()
                    } else {
                        d.to_owned()
                    },
                    p.to_owned(),
                ),
                _ => (self.dir.clone(), q.to_owned()),
            };
            if let Ok(children) = w.fs.list(&dir) {
                for c in children.iter().filter(|c| c.name().starts_with(&prefix)) {
                    if seen.insert(c.path.clone()) {
                        items.push(
                            Item::new(w.location.short(&c.path))
                                .detail(if c.is_dir() { "directory" } else { "file" })
                                .group("here")
                                .key(ItemKey::text(&c.path))
                                .insert(c.path.clone()),
                        );
                    }
                }
            }
        }
        if !q.is_empty() {
            for h in w.fs.find(&self.index, q) {
                if items.len() >= JUMP_MAX {
                    break;
                }
                if seen.insert(h.path.clone()) {
                    items.push(
                        Item::new(w.location.short(&h.path))
                            .detail(if h.is_dir { "directory" } else { "file" })
                            .group("indexed")
                            .key(ItemKey::text(&h.path))
                            .insert(h.path.clone()),
                    );
                }
            }
        }
        items.truncate(JUMP_MAX);
        items
    }

    fn open_jump(&mut self, w: &World, cx: &mut Cx) {
        let mut p = Picker::new(WidgetId::of("files.jump"), "Go to path");
        p.placeholder = "path · ~/… · relative to this folder".into();
        p.set_width(74);
        p.scope = Some(w.location.short(&self.dir));
        p.empty_text = "type a path or a name".into();
        p.set_items(vec![])
            .unwrap_or_else(|error| p.report_item_error(error));
        self.jump_error = None;
        cx.open(p, LayerTag::new("files-jump"));
    }

    fn jump_to(&mut self, target: &str, w: &World, cx: &mut Cx) -> Result<(), String> {
        let expanded = expand(target, &self.dir, &w.location.home);
        let node = w
            .fs
            .get(&expanded)
            .ok_or_else(|| format!("{}: no such file or directory", w.location.short(&expanded)))?;
        match &node.kind {
            NodeKind::Symlink { target } if !w.fs.exists(target) => {
                return Err(format!(
                    "{}: broken link → {target}",
                    w.location.short(&expanded)
                ));
            }
            _ => {}
        }
        self.remember();
        if w.fs.is_dir(&expanded) {
            self.mode = Mode::Browse;
            self.navigate(&expanded, w, cx);
        } else {
            let parent = Fs::parent(&expanded).unwrap_or("/".into());
            self.remembered.retain(|(d, _)| d != &parent);
            self.remembered.push((parent.clone(), expanded.clone()));
            self.mode = Mode::Browse;
            self.navigate(&parent, w, cx);
        }
        Ok(())
    }

    fn render_browse_list(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        _w: &World,
    ) {
        let t = ctx.theme;
        let bg = t.canvas;
        let focused = ctx.interaction.focused(LIST);
        self.list_area = area;
        ctx.control(LIST, area, false);
        ctx.scrollable(LIST, area);
        if self.pending.is_some() {
            empty::render(
                area,
                buf,
                t,
                &EmptyState::new("Listing…")
                    .hint("the folder answers shortly · Backspace goes back"),
                bg,
            );
            return;
        }
        if let Some(e) = &self.listing_error {
            empty::render(
                area,
                buf,
                t,
                &EmptyState::error(e).hint("Backspace goes to the parent · g jumps to a path"),
                bg,
            );
            return;
        }
        let visible: Vec<Entry> = self.visible().into_iter().cloned().collect();
        if visible.is_empty() {
            empty::render(
                area,
                buf,
                t,
                &EmptyState::new("Empty folder").hint(if self.entries.is_empty() {
                    "← goes to the parent"
                } else {
                    "Ctrl+H shows hidden entries"
                }),
                bg,
            );
            return;
        }
        self.scroll.set_content(visible.len());
        self.scroll.set_viewport(area.height as usize);
        let has_sb = self.scroll.overflows();
        let row_w = area.width.saturating_sub(u16::from(has_sb));
        let name_w = (row_w * 45 / 100).clamp(14, 48);
        for (k, i) in self.scroll.visible_range().enumerate() {
            let y = area.y + k as u16;
            let e = &visible[i];
            let rid = LIST.child(i);
            let mut s = ctx.state(rid);
            s.focused = focused && i == self.cursor;
            s.selected = i == self.cursor;
            let st = t.row(s, bg);
            let row = Rect::new(area.x, y, row_w, 1);
            fill(buf, row, st);
            buf.set_string(
                row.x,
                y,
                t.gutter_symbol(s),
                t.gutter(s, st.bg.unwrap_or(bg), false),
            );
            let plain = st.remove_modifier(Modifier::BOLD);
            let glyph = match e.kind {
                "directory" => "▸",
                "symlink" => "→",
                "fifo" | "device" => "◇",
                _ => " ",
            };
            buf.set_string(row.x + 1, y, glyph, plain.fg(t.text_secondary));
            let name = if e.lossy {
                format!("{} ⚠", e.name)
            } else {
                e.name.clone()
            };
            let ns = if e.error.is_some() {
                st.fg(t.text_faint)
            } else if e.hidden {
                st.fg(t.text_muted)
            } else {
                st
            };
            buf.set_string(row.x + 3, y, fit(&name, name_w as usize), ns);
            let mut x = row.x + 3 + name_w + 2;
            if x + 10 <= row.right() {
                buf.set_string(
                    x,
                    y,
                    fit_right(
                        &if e.is_dir {
                            "–".to_owned()
                        } else {
                            junie_tui::ui::text::bytes(e.size)
                        },
                        9,
                    ),
                    plain.fg(t.text_secondary),
                );
            }
            x += 11;
            if x + 17 <= row.right() {
                buf.set_string(
                    x,
                    y,
                    crate::scenario::CLOCK.stamp(e.mtime),
                    plain.fg(t.text_muted),
                );
            }
            x += 18;
            let tail = match &e.error {
                Some(err) => err.clone(),
                None => format!("{}{}", e.kind, if e.hidden { " · hidden" } else { "" }),
            };
            let avail = row.right().saturating_sub(x + 1) as usize;
            if avail >= 6 {
                buf.set_string(
                    x,
                    y,
                    junie_tui::ui::reveal::cut(Rect::new(x, y, (avail) as u16, 1), &tail),
                    plain.fg(if e.error.is_some() {
                        t.error
                    } else {
                        t.text_faint
                    }),
                );
            }

            legible_on_fill(buf, row, t, bg);
            ctx.clickable(rid, row);
        }
        if has_sb {
            scrollbar::render_with_fade(
                Rect::new(area.x, area.y, area.width, area.height),
                buf,
                ctx,
                LIST,
                &self.scroll,
                focused,
            );
        }
    }

    fn render_find_list(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let bg = t.canvas;
        let focused = ctx.interaction.focused(LIST);
        self.list_area = area;
        ctx.control(LIST, area, false);
        ctx.scrollable(LIST, area);
        if self.query.trim().is_empty() {
            empty::render(area, buf, t, &EmptyState::new("Type to search under home").hint("exact name and stem rank first, then substring, then fuzzy · 100 results at most"), bg);
            return;
        }
        if self.hits.is_empty() {
            empty::render(
                area,
                buf,
                t,
                &EmptyState::new(&format!("No matches for “{}”", truncate(&self.query, 30))).hint(
                    if self.indexed(w).1 {
                        "Esc clears the query"
                    } else {
                        "indexing is still running"
                    },
                ),
                bg,
            );
            return;
        }
        self.scroll.set_content(self.hits.len());
        self.scroll.set_viewport(area.height as usize);
        let has_sb = self.scroll.overflows();
        let row_w = area.width.saturating_sub(u16::from(has_sb));
        let name_w = (row_w * 40 / 100).clamp(14, 44);
        for (k, i) in self.scroll.visible_range().enumerate() {
            let y = area.y + k as u16;
            let h = &self.hits[i];
            let rid = LIST.child(i);
            let mut s = ctx.state(rid);
            s.focused = focused && i == self.cursor;
            s.selected = i == self.cursor;
            let st = t.row(s, bg);
            let row = Rect::new(area.x, y, row_w, 1);
            fill(buf, row, st);
            buf.set_string(
                row.x,
                y,
                t.gutter_symbol(s),
                t.gutter(s, st.bg.unwrap_or(bg), false),
            );
            buf.set_string(
                row.x + 1,
                y,
                if h.is_dir { "▸" } else { " " },
                st.fg(t.text_secondary).remove_modifier(Modifier::BOLD),
            );
            let name = h.path.rsplit('/').next().unwrap_or(&h.path);

            let mut x = row.x + 3;
            let mut used = 0usize;
            for (bi, g) in unicode_segmentation::UnicodeSegmentation::grapheme_indices(name, true) {
                let gw = width(g);
                if used + gw > name_w as usize {
                    break;
                }
                let mut cs = st;
                if h.matched.contains(&bi) {
                    cs = cs.add_modifier(Modifier::BOLD).fg(t.accent);
                } else if !s.focused {
                    cs = cs.remove_modifier(Modifier::BOLD);
                }
                buf.set_string(x, y, g, cs);
                x += gw as u16;
                used += gw;
            }
            let px = row.x + 3 + name_w + 2;
            let parent = w.location.short(&Fs::parent(&h.path).unwrap_or("/".into()));
            let avail = row.right().saturating_sub(px + 1) as usize;
            if avail >= 6 {
                buf.set_string(
                    px,
                    y,
                    truncate_middle(&parent, avail),
                    st.fg(t.text_muted).remove_modifier(Modifier::BOLD),
                );
            }

            legible_on_fill(buf, row, t, bg);
            ctx.clickable(rid, row);
        }
        if has_sb {
            scrollbar::render_with_fade(
                Rect::new(area.x, area.y, area.width, area.height),
                buf,
                ctx,
                LIST,
                &self.scroll,
                focused,
            );
        }
    }

    fn render_preview(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let focused = ctx.interaction.focused(PREVIEW);
        let Some(path) = self.selected_path() else {
            let panel = Panel::card(Some("Preview")).focused(focused);
            let inner = panel.render(area, buf, t);
            empty::render(
                inner,
                buf,
                t,
                &EmptyState::new("Nothing selected"),
                panel.bg(t),
            );
            ctx.control(PREVIEW, area, false);
            return;
        };
        if w.fs.is_dir(&path) {
            let title = crate::sim::fs::sanitize(&w.location.short(&path));
            let panel = Panel::card(Some(&title)).focused(focused).meta("directory");
            let inner = panel.render(area, buf, t);
            ctx.control(PREVIEW, area, false);
            let bg = panel.bg(t);
            let count = w.fs.count(&path);
            let node = w.fs.get(&path);
            let mut lines: Vec<(String, Tone)> = vec![];
            match count {
                Ok(c) => lines.push((
                    format!(
                        "{} visible · {} hidden{}",
                        c.visible,
                        c.hidden,
                        if c.capped {
                            " · at least (count capped at 2048 / 40 ms)"
                        } else {
                            ""
                        }
                    ),
                    Tone::Normal,
                )),
                Err(e) => lines.push((e.message(), Tone::Error)),
            }
            if let Some(n) = node {
                lines.push((
                    format!("modified {}", crate::scenario::CLOCK.stamp(n.mtime)),
                    Tone::Muted,
                ));
            }

            let markers: Vec<&str> = [
                ".git",
                "Cargo.toml",
                "package.json",
                "justfile",
                "Makefile",
                "docker-compose.yml",
                "compose.yaml",
                "build.gradle",
                "build.gradle.kts",
                "mise.toml",
                ".idea",
            ]
            .into_iter()
            .filter(|m| w.fs.exists(&Fs::join(&path, m)))
            .collect();
            if !markers.is_empty() {
                lines.push((format!("markers · {}", markers.join(", ")), Tone::Secondary));
                let recs = recommendations(&path, &markers, w);
                for r in recs.iter().take(8) {
                    lines.push((format!("  {r}"), Tone::Muted));
                }
                lines.push((
                    "recommendations are preview-only · run them from Here in that folder".into(),
                    Tone::Faint,
                ));
            }
            lines.push((
                "Find and Browse are always available from Here".into(),
                Tone::Faint,
            ));
            for (i, (l, tone)) in lines.iter().enumerate() {
                if i as u16 >= inner.height {
                    break;
                }
                buf.set_string(
                    inner.x,
                    inner.y + i as u16,
                    junie_tui::ui::reveal::cut(
                        Rect::new(inner.x, inner.y + i as u16, inner.width, 1),
                        l,
                    ),
                    Style::new().fg(t.tone(*tone)).bg(bg),
                );
            }
            return;
        }
        self.load_preview(&path, w);
        let meta = if let Some(q) = &self.find_query {
            format!(
                "find “{q}” · {} of {}",
                if self.find_matches.is_empty() {
                    0
                } else {
                    self.find_at + 1
                },
                self.find_matches.len()
            )
        } else {
            self.preview_meta.clone()
        };

        let room = area.width.saturating_sub(6) as usize;
        let meta = if width(&meta) + 3 > room {
            truncate(&meta, room.saturating_sub(3).max(4))
        } else {
            meta
        };
        let budget = room.saturating_sub(width(&meta) + 3);
        let title = if width(&self.preview_title) <= budget {
            Some(self.preview_title.clone())
        } else if budget >= 8 {
            Some(truncate_middle(&self.preview_title, budget))
        } else {
            None
        };
        let panel = Panel::framed(title.as_deref()).focused(focused).meta(&meta);
        let inner = panel.render(area, buf, t);
        ctx.control(PREVIEW, area, false);
        if let Some(e) = &self.preview_err {
            empty::render(
                inner,
                buf,
                t,
                &EmptyState::error(e).hint("Esc returns to the list"),
                t.canvas,
            );
            ctx.scrollable(PREVIEW, inner);
            return;
        }
        if self.preview_lines.is_empty() {
            empty::render(inner, buf, t, &EmptyState::new("Empty file"), t.canvas);
            ctx.scrollable(PREVIEW, inner);
            return;
        }
        self.preview.render(inner, buf, ctx, t.canvas);
    }
}

fn unicode_normalize(name: &str) -> String {
    name.chars()
        .filter(|c| !unicode_is_combining(*c))
        .collect::<String>()
        .to_lowercase()
}

fn unicode_is_combining(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036f | 0x1ab0..=0x1aff | 0x1dc0..=0x1dff | 0x20d0..=0x20ff | 0xfe20..=0xfe2f)
}

pub fn expand(q: &str, dir: &str, home: &str) -> String {
    let q = q.trim();
    if q == "~" {
        return home.into();
    }
    if let Some(rest) = q.strip_prefix("~/") {
        return format!("{home}/{rest}");
    }
    if q.starts_with('/') {
        return q.trim_end_matches('/').to_owned().replace("//", "/");
    }
    if q.is_empty() {
        return dir.into();
    }
    Fs::join(dir, q.trim_end_matches('/'))
}

pub fn recommendations(path: &str, markers: &[&str], w: &World) -> Vec<String> {
    let mut v = vec![];
    if markers.contains(&".git") {
        v.push("git status · git pull".into());
    }
    if markers.contains(&"Cargo.toml") {
        v.push(if w.tools.contains("cargo") {
            "cargo build · cargo test".into()
        } else {
            "cargo build · cargo is not on PATH".into()
        });
    }
    if markers.contains(&"package.json") {
        let locks: Vec<&str> = ["pnpm-lock.yaml", "yarn.lock", "bun.lock", "bun.lockb"]
            .into_iter()
            .filter(|l| w.fs.exists(&Fs::join(path, l)))
            .collect();
        let runner = crate::domain::manifest::node_runner(&locks);
        let text = match w
            .fs
            .get(&Fs::join(path, "package.json"))
            .map(|n| &n.content)
        {
            Some(crate::sim::fs::Content::Text(t)) => Some(t.clone()),
            _ => None,
        };
        let d = crate::domain::manifest::node_scripts(path, text.as_deref(), &locks);
        for s in d.tasks.iter().take(8) {
            v.push(format!("{runner} run {}", s.name));
        }
    }
    if markers.contains(&"justfile") {
        v.push("just --list".into());
    }
    if markers.contains(&"Makefile") {
        let text = match w.fs.get(&Fs::join(path, "Makefile")).map(|n| &n.content) {
            Some(crate::sim::fs::Content::Text(t)) => t.clone(),
            _ => String::new(),
        };
        for t in crate::domain::manifest::make_targets(&text)
            .0
            .iter()
            .take(8)
        {
            v.push(format!("make {t}"));
        }
    }
    if markers.contains(&"docker-compose.yml") || markers.contains(&"compose.yaml") {
        v.push("docker compose up -d · docker compose logs --tail 200".into());
    }
    if markers.contains(&"build.gradle") || markers.contains(&"build.gradle.kts") {
        v.push(if w.tools.contains("gradle") {
            "gradle build · gradle test".into()
        } else {
            "gradle build · gradle is not on PATH".into()
        });
    }
    if markers.contains(&"mise.toml") {
        v.push("mise run <task>".into());
    }
    if markers.contains(&".idea") {
        v.push("idea.clean · IntelliJ metadata cleanup".into());
    }
    v
}

impl ShellPage<Holla> for FilesPage {
    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        if cx.focus.is(PREVIEW) {
            if let Some(q) = self.find_query.clone() {
                match key.code {
                    KeyCode::Esc => {
                        self.find_query = None;
                        self.find_matches.clear();
                        self.preview.clear_marks();
                        return Outcome::Changed;
                    }
                    KeyCode::Enter | KeyCode::Down if !self.find_matches.is_empty() => {
                        self.find_at = (self.find_at + 1) % self.find_matches.len();
                        self.apply_marks();
                        return Outcome::Changed;
                    }
                    KeyCode::Up if !self.find_matches.is_empty() => {
                        self.find_at =
                            (self.find_at + self.find_matches.len() - 1) % self.find_matches.len();
                        self.apply_marks();
                        return Outcome::Changed;
                    }
                    KeyCode::Enter | KeyCode::Down | KeyCode::Up => return Outcome::Consumed,
                    KeyCode::Backspace => {
                        let mut q = q;
                        q.pop();
                        self.find_query = Some(q);
                        self.run_find();
                        return Outcome::Changed;
                    }
                    KeyCode::Char(c) if !key.chord() => {
                        let mut q = q;
                        q.push(c);
                        self.find_query = Some(q);
                        self.run_find();
                        return Outcome::Changed;
                    }
                    _ => {}
                }
            }
            match key.code {
                KeyCode::Char('/') if key.plain() => {
                    self.find_query = Some(String::new());
                    return Outcome::Changed;
                }
                KeyCode::Esc => {
                    if self.preview.has_selection() {
                        self.preview.clear_selection();
                        return Outcome::Changed;
                    }
                    self.md.close_drawer(cx.focus);
                    return Outcome::Changed;
                }
                KeyCode::Left if key.plain() => {
                    self.md.close_drawer(cx.focus);
                    return Outcome::Changed;
                }
                _ => {}
            }
            let (o, ev) = self.preview.on_key(key);
            match ev {
                Some(ViewportAction::Copy(t)) => cx.copy(t),
                Some(ViewportAction::FollowChanged(_)) => {}
                _ => {}
            }

            return o;
        }
        if !cx.focus.is(LIST) {
            return Outcome::Ignored;
        }

        if key.ctrl() {
            return match key.code {
                KeyCode::Char('h') => {
                    self.show_hidden = !self.show_hidden;
                    let keep = self.current().map(|e| e.path);
                    self.cursor = keep
                        .and_then(|p| self.visible().iter().position(|e| e.path == p))
                        .unwrap_or(0);
                    cx.status(if self.show_hidden {
                        "Hidden entries shown · from the same listing"
                    } else {
                        "Hidden entries hidden"
                    });
                    Outcome::Changed
                }
                KeyCode::Char('u') if self.mode == Mode::Find => {
                    self.query.clear();
                    self.refresh_hits(w);
                    Outcome::Changed
                }
                _ => Outcome::Ignored,
            };
        }
        if key.alt() && key.code == KeyCode::Enter {
            return self.open_actions(cx);
        }
        if key.alt() {
            return Outcome::Ignored;
        }
        match (&self.mode, key.code) {
            (_, KeyCode::Up) => {
                self.cursor = self.cursor.saturating_sub(1);
                self.scroll.ensure_visible(self.cursor);
                Outcome::Changed
            }
            (_, KeyCode::Down) => {
                let n = match self.mode {
                    Mode::Browse => self.visible().len(),
                    Mode::Find => self.hits.len(),
                };
                self.cursor = (self.cursor + 1).min(n.saturating_sub(1));
                self.scroll.ensure_visible(self.cursor);
                Outcome::Changed
            }
            (_, KeyCode::PageUp) => {
                self.cursor = self.cursor.saturating_sub(self.scroll.viewport_len.max(1));
                self.scroll.ensure_visible(self.cursor);
                Outcome::Changed
            }
            (_, KeyCode::PageDown) => {
                let n = match self.mode {
                    Mode::Browse => self.visible().len(),
                    Mode::Find => self.hits.len(),
                };
                self.cursor =
                    (self.cursor + self.scroll.viewport_len.max(1)).min(n.saturating_sub(1));
                self.scroll.ensure_visible(self.cursor);
                Outcome::Changed
            }
            (_, KeyCode::Home) => {
                self.cursor = 0;
                self.scroll.jump_start();
                Outcome::Changed
            }
            (_, KeyCode::End) => {
                let n = match self.mode {
                    Mode::Browse => self.visible().len(),
                    Mode::Find => self.hits.len(),
                };
                self.cursor = n.saturating_sub(1);
                self.scroll.ensure_visible(self.cursor);
                Outcome::Changed
            }
            (Mode::Browse, KeyCode::Enter) => self.open_current(w, cx),
            (Mode::Browse, KeyCode::Right) => {
                if self.current().is_some_and(|e| e.is_dir) {
                    self.open_current(w, cx)
                } else {
                    self.md.open_drawer(cx.focus);
                    Outcome::Changed
                }
            }
            (Mode::Browse, KeyCode::Left) | (Mode::Browse, KeyCode::Backspace) => {
                self.go_parent(w, cx);
                Outcome::Changed
            }
            (Mode::Browse, KeyCode::Char('g')) => {
                self.open_jump(w, cx);
                Outcome::Changed
            }
            (Mode::Browse, KeyCode::Char('/')) => {
                self.mode = Mode::Find;
                self.query.clear();
                self.cursor = 0;
                self.refresh_hits(w);
                Outcome::Changed
            }
            (Mode::Browse, KeyCode::Char('y')) => {
                if let Some(p) = self.selected_path() {
                    cx.go(Go::Osc52(p));
                }
                Outcome::Changed
            }
            (Mode::Browse, KeyCode::Char('a')) => {
                if let Some(p) = self.selected_path() {
                    let root = if w.fs.is_dir(&p) {
                        p
                    } else {
                        Fs::parent(&p).unwrap_or("/".into())
                    };
                    cx.go(Go::Analyze(root));
                }
                Outcome::Changed
            }
            (Mode::Browse, KeyCode::Esc) => {
                cx.go(Go::Pop);
                Outcome::Changed
            }
            (Mode::Find, KeyCode::Enter) => self.open_actions(cx),
            (Mode::Find, KeyCode::Backspace) => {
                use unicode_segmentation::UnicodeSegmentation;
                if let Some((i, _)) = self.query.grapheme_indices(true).next_back() {
                    self.query.truncate(i);
                    self.cursor = 0;
                    self.refresh_hits(w);
                }
                Outcome::Changed
            }
            (Mode::Find, KeyCode::Delete) => {
                self.query.clear();
                self.cursor = 0;
                self.refresh_hits(w);
                Outcome::Changed
            }
            (Mode::Find, KeyCode::Esc) => {
                if !self.query.is_empty() {
                    self.query.clear();
                    self.cursor = 0;
                    self.refresh_hits(w);
                    return Outcome::Changed;
                }
                cx.go(Go::Pop);
                Outcome::Changed
            }
            (Mode::Find, KeyCode::Char(c)) => {
                self.query.push(c);
                self.cursor = 0;
                self.refresh_hits(w);
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn on_paste(&mut self, text: &str, w: &mut World, _cx: &mut Cx) -> Outcome {
        if self.mode == Mode::Find {
            let flat: String = text
                .chars()
                .map(|c| {
                    if c == '\n' || c == '\r' || c == '\t' {
                        ' '
                    } else {
                        c
                    }
                })
                .collect();
            self.query.push_str(&flat);
            self.cursor = 0;
            self.refresh_hits(w);
            return Outcome::Changed;
        }
        if let Some(q) = self.find_query.as_mut() {
            q.push_str(text.trim());
            self.run_find();
            return Outcome::Changed;
        }
        Outcome::Consumed
    }

    fn on_layer_result(
        &mut self,
        tag: &LayerTag,
        result: ModalResult,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        match tag.kind {
            "file-actions" => {
                if let ModalResult::MenuChosen(i) = result {
                    let path = tag.key.clone();
                    self.file_action(i, &path, w, cx);
                }
                Outcome::Changed
            }
            "files-jump" => {
                if let ModalResult::Picked(_) = result
                    && let Some(target) = tag.key.strip_prefix("jump:")
                {
                    match self.jump_to(target, w, cx) {
                        Ok(()) => cx.status(format!(
                            "Jumped to {}",
                            w.location
                                .short(&expand(target, &self.dir, &w.location.home))
                        )),
                        Err(e) => cx.error(e),
                    }
                }
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        let (id, pos) = (ev.id, ev.pos());

        if ev.on(PREVIEW) && !ev.is(Phase::Secondary) {
            if ev.is(Phase::Click) {
                cx.focus.focus(PREVIEW);
            }
            return self.preview.pointer(ev).outcome();
        }
        let row = self.scroll.visible_range().find(|&i| LIST.child(i) == id);
        if let Some((axis, delta)) = ev.wheel() {
            if id == LIST && axis == Axis::Vertical {
                self.scroll.scroll_by(delta as isize);
                return Outcome::Changed;
            }
            return Outcome::Ignored;
        }
        match ev.phase() {
            Some(Phase::Press) if id == scrollbar::id_for(LIST) => {
                scroll_press(self.list_area, pos, &mut self.scroll)
            }
            Some(Phase::Drag) if id == scrollbar::id_for(LIST) => {
                scroll_drag(self.list_area, pos, &mut self.scroll)
            }
            Some(Phase::Click) => {
                if id == scrollbar::id_for(LIST) {
                    cx.focus.focus(LIST);
                    return scroll_press(self.list_area, pos, &mut self.scroll);
                }
                if let Some(i) = row {
                    let again = self.cursor == i;
                    self.cursor = i;

                    if ClickPolicy::DATA.activates(ev.count(), again) {
                        return match self.mode {
                            Mode::Browse => self.open_current(w, cx),
                            Mode::Find => self.open_actions(cx),
                        };
                    }
                    cx.focus.focus(LIST);
                    return Outcome::Changed;
                }
                if id == LIST {
                    cx.focus.focus(LIST);
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            Some(Phase::Secondary) => {
                if let Some(i) = row
                    && let Some(path) = self.path_at(i)
                {
                    if cx.context.moves_cursor {
                        self.cursor = i;
                    }
                    cx.focus.focus(LIST);

                    return self.open_actions_at(path, Rect::new(pos.x, pos.y, 1, 1), cx);
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn on_tick(&mut self, w: &mut World, cx: &mut Cx) -> Outcome {
        let mut changed = false;
        if let Some(p) = self.pending.clone()
            && w.tick >= p.due
        {
            self.pending = None;
            if p.generation == self.generation && p.dir == self.dir {
                self.apply_listing(&p.dir, p.generation, w);
                cx.status(format!("Listed {}", w.location.short(&p.dir)));
            }
            changed = true;
        }
        if self.mode == Mode::Find && w.tick != self.last_tick && !self.indexed(w).1 {
            self.refresh_hits(w);
            changed = true;
        }
        self.last_tick = w.tick;
        if changed {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn enter(&mut self, w: &mut World, cx: &mut Cx) {
        if self.index.is_empty() {
            self.index = w.fs.home_index(&w.location.home);
        }
        match self.mode {
            Mode::Browse => {
                if self.entries.is_empty() && self.pending.is_none() {
                    let dir = self.dir.clone();
                    let had_error = self.listing_error.take();
                    self.navigate(&dir, w, cx);
                    if let Some(e) = had_error {
                        cx.error(e);
                    }
                }
                cx.focus.focus(LIST);
            }
            Mode::Find => {
                self.refresh_hits(w);
                cx.focus.focus(LIST);
            }
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        match self.mode {
            Mode::Browse => {
                let title = format!(
                    "Files · {}",
                    crate::sim::fs::sanitize(&w.location.short(&self.dir))
                );
                buf.set_string(
                    area.x + 1,
                    area.y,
                    junie_tui::ui::reveal::cut(
                        Rect::new(area.x + 1, area.y, area.width.saturating_sub(2), 1),
                        &title,
                    ),
                    t.title(),
                );
                let n = self.visible().len();
                let hidden = self.entries.iter().filter(|e| e.hidden).count();
                let line = format!(
                    "{} · {} hidden {} · directories first",
                    plural(n, "entry", "entries"),
                    hidden,
                    if self.show_hidden {
                        "shown"
                    } else {
                        "hidden · Ctrl+H shows"
                    }
                );
                buf.set_string(
                    area.x + 1,
                    area.y + 1,
                    junie_tui::ui::reveal::cut(
                        Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), 1),
                        &line,
                    ),
                    t.muted(),
                );
            }
            Mode::Find => {
                let (n, done) = self.indexed(w);
                let query = Rect::new(area.x, area.y, area.width, 1);
                let focused = ctx.interaction.focused(LIST);
                let fs = t.field_style(junie_tui::ui::ctx::VisualState {
                    focused,
                    editing: focused,
                    ..Default::default()
                });
                fill(buf, query, fs);
                let bg = fs.bg.unwrap_or(t.field);
                buf.set_string(
                    query.x,
                    query.y,
                    t.gutter_symbol(junie_tui::ui::ctx::VisualState {
                        focused,
                        ..Default::default()
                    }),
                    t.gutter(
                        junie_tui::ui::ctx::VisualState {
                            focused,
                            ..Default::default()
                        },
                        bg,
                        false,
                    ),
                );
                let readout = format!(
                    "{} · {} of {} indexed{}",
                    w.location.short(&w.location.home),
                    n,
                    self.index.len(),
                    if done { "" } else { " · indexing" }
                );
                let rw = width(&readout) as u16 + 2;
                let rx = query.right().saturating_sub(rw);
                buf.set_string(rx, query.y, format!(" {readout} "), t.muted().bg(bg));
                let text_w = rx.saturating_sub(query.x + 3) as usize;
                if self.query.is_empty() {
                    buf.set_string(
                        query.x + 2,
                        query.y,
                        junie_tui::ui::reveal::cut(
                            Rect::new(query.x + 2, query.y, (text_w) as u16, 1),
                            "Find files under home…",
                        ),
                        fs.fg(t.text_muted),
                    );
                } else {
                    buf.set_string(
                        query.x + 2,
                        query.y,
                        junie_tui::ui::reveal::cut(
                            Rect::new(query.x + 2, query.y, (text_w) as u16, 1),
                            &self.query,
                        ),
                        fs.add_modifier(Modifier::UNDERLINED)
                            .underline_color(t.accent),
                    );
                }
                if focused {
                    ctx.set_cursor(Position::new(
                        query.x + 2 + width(&self.query).min(text_w) as u16,
                        query.y,
                    ));
                }
                ctx.control(QUERY, query, false);
                Heading::new(&format!(
                    "{} · home scope",
                    plural(self.hits.len(), "result", "results")
                ))
                .render(
                    Rect::new(area.x + 3, area.y + 1, area.width.saturating_sub(3), 1),
                    buf,
                    t,
                    t.canvas,
                );
            }
        }
        let body = Rect::new(
            area.x,
            area.y + 3,
            area.width,
            area.height.saturating_sub(3),
        );
        let l = self.md.layout(body, buf, ctx, ctx.theme.canvas);
        if !l.master.is_empty() {
            match self.mode {
                Mode::Browse => self.render_browse_list(l.master, buf, ctx, w),
                Mode::Find => self.render_find_list(l.master, buf, ctx, w),
            }
        }
        if !l.detail.is_empty() {
            self.render_preview(l.detail, buf, ctx, w);
        }
        self.md.finish(ctx);
    }

    fn hints(&self, focus: Option<WidgetId>, _w: &World) -> Vec<Hint> {
        if focus == Some(PREVIEW) {
            if self.find_query.is_some() {
                return vec![
                    hint("Type", "Find"),
                    hint("Enter / ↓", "Next"),
                    hint("↑", "Previous"),
                    hint("Esc", "Close find"),
                ];
            }
            return vec![
                hint("↑↓", "Scroll"),
                hint("Shift+↑↓", "Select"),
                hint("y", "Copy"),
                hint("/", "Find"),
                hint("Esc", "Back to list"),
            ];
        }
        match self.mode {
            Mode::Browse => vec![
                hint("↑↓", "Move"),
                hint("Enter", "Open / preview"),
                hint("←", "Parent"),
                hint("g", "Go to path"),
                junie_tui::widgets::keyhint::toggle_hint(
                    "Ctrl+H",
                    self.show_hidden,
                    "Hide hidden",
                    "Show hidden",
                ),
                hint("Alt+Enter", "Alternatives…"),
                hint("/", "Find"),
                hint("Esc", "Back"),
            ],
            Mode::Find => vec![
                hint("Type", "Find"),
                hint("↑↓", "Move"),
                hint("Enter", "Alternatives…"),
                hint("Tab", "Preview"),
                hint(
                    "Esc",
                    if self.query.is_empty() {
                        "Back"
                    } else {
                        "Clear"
                    },
                ),
            ],
        }
    }

    fn crumb(&self, w: &World) -> String {
        match self.mode {
            Mode::Browse => format!(
                "Files › {}",
                w.location
                    .short(&self.dir)
                    .rsplit('/')
                    .next()
                    .unwrap_or("/")
            ),
            Mode::Find => "Files › Find".into(),
        }
    }

    fn animating(&self, w: &World) -> bool {
        self.pending.is_some() || (self.mode == Mode::Find && !self.indexed(w).1)
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(LIST)
    }
}

impl Screen for FilesPage {
    fn on_context_key(&mut self, _w: &mut World, cx: &mut Cx) -> Outcome {
        if !cx.focus.is(LIST) {
            return Outcome::Ignored;
        }
        self.open_actions(cx)
    }

    fn status(&self, w: &World) -> StatusBits {
        let text = match self.mode {
            Mode::Browse => format!(
                "{} · {}",
                w.location.short(&self.dir),
                if self.show_hidden {
                    "hidden shown"
                } else {
                    "hidden off"
                }
            ),
            Mode::Find => {
                let (n, done) = self.indexed(w);
                format!(
                    "{} indexed{} · {} results",
                    n,
                    if done { "" } else { " · indexing" },
                    self.hits.len()
                )
            }
        };
        let mut item = StatusItem::new(text, Tone::Secondary).priority(6);
        if self.mode == Mode::Find && !self.indexed(w).1 {
            item = item.busy();
        }
        StatusBits {
            center: Some(item),
            right: vec![],
        }
    }

    fn typing_hot(&self) -> bool {
        self.mode == Mode::Find
    }

    fn as_files(&mut self) -> Option<&mut FilesPage> {
        Some(self)
    }
}

impl FilesPage {
    pub fn jump_rows(&self, query: &str, w: &World) -> (Vec<Item>, PickerStatus) {
        let items = self.jump_suggestions(query, w);
        let status = match &self.jump_error {
            Some(e) => PickerStatus::Error {
                message: e.clone(),
                detail: Some("edit the path or choose a suggestion".into()),
            },
            None => PickerStatus::Ready,
        };
        (items, status)
    }

    pub fn set_jump_error(&mut self, e: Option<String>) {
        self.jump_error = e;
    }

    pub fn jump_target(&self, query: &str, selected: Option<&str>, w: &World) -> Option<String> {
        let expanded = expand(query, &self.dir, &w.location.home);
        if !query.trim().is_empty() && w.fs.exists(&expanded) {
            return Some(expanded);
        }
        if let Some(s) = selected {
            return Some(s.to_owned());
        }
        if query.trim().is_empty() {
            return None;
        }
        Some(expanded)
    }

    pub fn perform_jump(&mut self, target: &str, w: &World, cx: &mut Cx) -> Result<(), String> {
        self.jump_to(target, w, cx)
    }
}

#[cfg(test)]
mod tests;

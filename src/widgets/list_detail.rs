use crate::core::event::{Key, Outcome};
use crate::core::focus::Focus;
use crate::core::id::WidgetId;
use crate::core::pointer::PointerEvent;
use crate::core::scroll::{ScrollState, TrackClick};
use crate::theme::{ButtonKind, StatusKind, Tone};
use crate::ui::ctx::{RenderCtx, fill, legible_on_fill};
use crate::ui::layout::Split;
use crate::ui::master_detail::{MasterDetail, Mode};
use crate::ui::text::{fit, truncate, width, wrap};
use crate::widgets::button::{Button, ButtonRow, RowOverflow};
use crate::widgets::keyhint::{Hint, hint};
use crate::widgets::notice::Notice;
use crate::widgets::panel::{Panel, PanelFocus};
use crate::widgets::scrollbar;
use crate::widgets::splitter::Splitter;
use crate::widgets::viewport::{Span, render_spans};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

const COLUMN_GAP: u16 = 2;

fn columns(rest: u16, detail: u16, meta: u16) -> (u16, u16) {
    let both = detail > 0 && meta > 0;
    let gap = if both { COLUMN_GAP } else { 0 };
    let (mut d, mut m) = if detail + gap + meta <= rest {
        (detail, meta)
    } else if detail <= meta {
        let d = detail.min(rest);
        (d, rest.saturating_sub(d + gap))
    } else {
        let m = meta.min(rest);
        (rest.saturating_sub(m + gap), m)
    };
    if m < meta.min(4) {
        m = 0;
        d = detail.min(rest);
    }
    if d < detail.min(4) {
        d = 0;
    }
    (d, m)
}

#[derive(Debug, Clone)]
pub struct ListRow {
    pub mark: Option<StatusKind>,
    pub label: String,
    pub detail: String,
    pub meta: String,
    pub meta_tone: Tone,
    pub faint: bool,
}

impl ListRow {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            mark: None,
            label: label.into(),
            detail: String::new(),
            meta: String::new(),
            meta_tone: Tone::Secondary,
            faint: false,
        }
    }

    pub fn mark(mut self, mark: StatusKind) -> Self {
        self.mark = Some(mark);
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    pub fn meta(mut self, meta: impl Into<String>, tone: Tone) -> Self {
        self.meta = meta.into();
        self.meta_tone = tone;
        self
    }

    pub fn faint(mut self) -> Self {
        self.faint = true;
        self
    }
}

#[derive(Debug, Clone)]
pub enum Line {
    Prop(String, String, Tone),
    Text(String, Tone),
    Heading(String, String),
    Item {
        mark: Option<StatusKind>,
        cols: [String; 3],
        tone: Tone,
    },
    Notice(Notice),
    Blank,
}

#[derive(Debug, Clone)]
pub struct Action {
    pub name: &'static str,
    pub label: String,
    pub kind: ButtonKind,
    pub disabled: bool,
}

impl Action {
    pub fn new(name: &'static str, label: &str, kind: ButtonKind) -> Self {
        Self {
            name,
            label: label.to_owned(),
            kind,
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

pub struct Frame<'a> {
    pub title: &'a str,
    pub meta: String,
    pub head: Vec<Span>,
    pub rows: &'a [ListRow],
    pub under: Option<&'a str>,
    pub detail_title: String,
    pub detail_meta: String,
    pub lines: Vec<Line>,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Item(usize),
    Action(&'static str),
    Row(usize),
}

enum Rendered {
    Prop(String, String, Tone),
    Text(String, Tone),
    Heading(String, String),
    Item(usize, usize),
    Notice(usize, u16),
    Skip,
    Blank,
}

pub struct ListDetail {
    pub master: WidgetId,
    pub detail: WidgetId,
    md: MasterDetail,
    pub cursor: usize,
    pub item_cursor: usize,
    scroll: ScrollState,
    detail_scroll: ScrollState,
    names: Vec<&'static str>,
    buttons: Vec<Button>,
    items: usize,
    item_rows: Vec<usize>,
    detail_had_focus: bool,
}

impl ListDetail {
    pub fn new(master: WidgetId, detail: WidgetId, split: Split) -> Self {
        Self {
            master,
            detail,
            md: MasterDetail::new(master, detail)
                .resizable(split)
                .seam(Splitter::new(
                    master.sub("seam"),
                    crate::ui::layout::SplitDir::Horizontal,
                ))
                .collapse_below(100)
                .summary_rows(6),
            cursor: 0,
            item_cursor: 0,
            scroll: ScrollState::default(),
            detail_scroll: ScrollState::default(),
            names: vec![],
            buttons: vec![],
            items: 0,
            item_rows: vec![],
            detail_had_focus: false,
        }
    }

    pub fn clamp(&mut self, len: usize) {
        self.cursor = self.cursor.min(len.saturating_sub(1));
    }

    pub fn select(&mut self, index: usize) {
        self.cursor = index;
        self.item_cursor = 0;
        self.detail_scroll.jump_start();
    }

    pub fn open_detail(&mut self, focus: &mut Focus) {
        self.md.open_drawer(focus);
    }

    pub fn close_detail(&mut self, focus: &mut Focus) {
        self.md.close_drawer(focus);
    }

    pub fn actions_focused(&self, focus: Option<WidgetId>) -> bool {
        self.buttons.iter().any(|b| Some(b.id) == focus)
    }

    pub fn on_detail(&self, focus: Option<WidgetId>) -> bool {
        focus.is_some_and(|f| self.md.in_detail(f))
    }

    fn reveal_item(&mut self) {
        if let Some(&row) = self.item_rows.get(self.item_cursor) {
            self.detail_scroll.ensure_visible(row);
        }
    }

    fn cursor_item_visible(&self) -> bool {
        self.item_rows
            .get(self.item_cursor)
            .is_none_or(|row| self.detail_scroll.visible_range().contains(row))
    }

    fn move_cursor(&mut self, delta: isize, len: usize) {
        if len == 0 {
            return;
        }
        let next = (self.cursor as isize + delta).clamp(0, len as isize - 1) as usize;
        if next != self.cursor {
            self.select(next);
        }
    }

    pub fn on_key(&mut self, key: &Key, focus: &mut Focus, len: usize) -> (Outcome, Option<Event>) {
        if let Some(o) = self.md.on_key(key, focus) {
            return (o, None);
        }
        let current = focus.current();
        if current == Some(self.master) {
            return (self.master_key(key, focus, len), None);
        }
        if let Some(i) = self.buttons.iter().position(|b| Some(b.id) == current) {
            return self.action_key(i, key, focus);
        }
        if current == Some(self.detail) {
            return self.detail_key(key, focus);
        }
        (Outcome::Ignored, None)
    }

    fn master_key(&mut self, key: &Key, focus: &mut Focus, len: usize) -> Outcome {
        if !key.plain() {
            return Outcome::Ignored;
        }
        let page = self.scroll.viewport_len.max(1) as isize;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1, len),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1, len),
            KeyCode::PageUp => self.move_cursor(-page, len),
            KeyCode::PageDown => self.move_cursor(page, len),
            KeyCode::Home | KeyCode::Char('g') => self.move_cursor(-(len as isize), len),
            KeyCode::End | KeyCode::Char('G') => self.move_cursor(len as isize, len),
            KeyCode::Enter | KeyCode::Tab => self.md.open_drawer(focus),
            _ => return Outcome::Ignored,
        }
        Outcome::Changed
    }

    fn detail_key(&mut self, key: &Key, focus: &mut Focus) -> (Outcome, Option<Event>) {
        if !key.plain() {
            return (Outcome::Ignored, None);
        }
        let step = |s: &mut Self, delta: isize| {
            if s.items > 0 {
                let next = (s.item_cursor as isize + delta).clamp(0, s.items as isize - 1);
                s.item_cursor = next as usize;
                s.reveal_item();
            } else {
                s.detail_scroll.scroll_by(delta);
            }
        };
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => step(self, -1),
            KeyCode::Down | KeyCode::Char('j') => step(self, 1),
            KeyCode::PageUp => {
                self.detail_scroll.page_up();
            }
            KeyCode::PageDown => {
                self.detail_scroll.page_down();
            }
            KeyCode::Enter if self.items > 0 => {
                if !self.cursor_item_visible() {
                    self.reveal_item();
                    return (Outcome::Changed, None);
                }
                return (Outcome::Changed, Some(Event::Item(self.item_cursor)));
            }
            KeyCode::Esc | KeyCode::Left | KeyCode::BackTab => self.md.close_drawer(focus),
            KeyCode::Tab | KeyCode::Enter => {
                if let Some(b) = self.buttons.first() {
                    focus.focus(b.id);
                }
            }
            _ => return (Outcome::Ignored, None),
        }
        (Outcome::Changed, None)
    }

    fn action_key(
        &mut self,
        index: usize,
        key: &Key,
        focus: &mut Focus,
    ) -> (Outcome, Option<Event>) {
        let name = self.names[index];
        let (o, fired) = self.buttons[index].on_key(key);
        if fired {
            return (Outcome::Changed, Some(Event::Action(name)));
        }
        if o.consumed() {
            return (o, None);
        }
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                if index > 0 {
                    focus.focus(self.buttons[index - 1].id);
                } else {
                    focus.focus(self.detail);
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if let Some(b) = self.buttons.get(index + 1) {
                    focus.focus(b.id);
                }
            }
            KeyCode::Esc => self.md.close_drawer(focus),
            _ => return (Outcome::Ignored, None),
        }
        (Outcome::Changed, None)
    }

    pub fn on_pointer(&mut self, ev: &PointerEvent) -> Option<Outcome> {
        self.md
            .on_pointer(ev)
            .or_else(|| scrollbar::pointer(ev, self.master, &mut self.scroll, TrackClick::Jump))
            .or_else(|| {
                scrollbar::pointer(ev, self.detail, &mut self.detail_scroll, TrackClick::Jump)
            })
    }

    pub fn wheel(&mut self, id: WidgetId, delta: i32) -> Outcome {
        if id == self.master || id == scrollbar::id_for(self.master) {
            self.scroll.scroll_by(delta as isize);
            return Outcome::Changed;
        }
        if id == self.detail || id == scrollbar::id_for(self.detail) {
            self.detail_scroll.scroll_by(delta as isize);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    pub fn click(
        &mut self,
        id: WidgetId,
        focus: &mut Focus,
        len: usize,
    ) -> (Outcome, Option<Event>) {
        for i in self.scroll.visible_range() {
            if i < len && self.master.child(i) == id {
                let same = self.cursor == i;
                if !same {
                    self.select(i);
                }
                self.md.close_drawer(focus);
                return (Outcome::Changed, Some(Event::Row(i)));
            }
        }
        for i in 0..self.items {
            if self.detail.child(i).sub("item") == id {
                self.item_cursor = i;
                focus.focus(self.detail);
                return (Outcome::Changed, Some(Event::Item(i)));
            }
        }
        if id == self.detail {
            self.md.open_drawer(focus);
            return (Outcome::Changed, None);
        }
        for (name, b) in self.names.iter().zip(self.buttons.iter_mut()) {
            if b.id == id {
                focus.focus(id);
                let fired = b.on_click();
                return (Outcome::Changed, fired.then_some(Event::Action(name)));
            }
        }
        (Outcome::Ignored, None)
    }

    pub fn hints(&self, focus: Option<WidgetId>, item_hints: &[Hint]) -> Option<Vec<Hint>> {
        if let Some(b) = self.buttons.iter().find(|b| Some(b.id) == focus) {
            return Some(vec![hint("←→", "Choose"), b.hint(), hint("Esc", "Back")]);
        }
        if focus == Some(self.detail) {
            let mut v = if self.items > 0 {
                vec![hint("↑↓", "Move")]
            } else {
                vec![hint("↑↓", "Scroll")]
            };
            v.extend(item_hints.iter().cloned());
            v.push(hint("Tab", "Actions"));
            v.push(hint("Esc", "Back"));
            return Some(v);
        }
        None
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, frame: &Frame) {
        let t = ctx.theme;
        self.names = frame.actions.iter().map(|a| a.name).collect();
        self.buttons = frame
            .actions
            .iter()
            .map(|a| Button::new(self.detail.sub(a.name), &a.label, a.kind).disabled(a.disabled))
            .collect();
        self.md.detail_owns = self.buttons.iter().map(|b| b.id).collect();
        self.clamp(frame.rows.len());
        let l = self.md.layout(area, buf, ctx, t.canvas);
        if !l.master.is_empty() {
            self.draw_list(l.master, buf, ctx, frame);
        }
        match l.mode {
            Mode::Split => self.draw_detail(l.detail, buf, ctx, frame, false),
            Mode::Drawer => self.draw_detail(l.detail, buf, ctx, frame, true),
            Mode::Master => self.draw_summary(l.summary, buf, ctx, frame),
        }
        self.md.finish(ctx);
    }

    fn draw_list(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, frame: &Frame) {
        let t = ctx.theme;
        let focused = ctx.interaction.focused(self.master);
        let pos = scrollbar::position_label(&self.scroll);
        let meta = if pos.is_empty() {
            frame.meta.clone()
        } else {
            format!("{} · {pos}", frame.meta)
        };
        let over = u16::from(!frame.head.is_empty());
        let under_lines = frame
            .under
            .map(|u| wrap(u, area.width.saturating_sub(2).max(8) as usize))
            .unwrap_or_default();
        let under = (under_lines.len() as u16).min(area.height.saturating_sub(over));
        if over > 0 {
            render_spans(
                buf,
                area.x + 1,
                area.y,
                area.width.saturating_sub(2),
                t,
                &frame.head,
                t.canvas,
            );
        }
        let framed = Rect::new(
            area.x,
            area.y + over,
            area.width,
            area.height.saturating_sub(under + over),
        );
        let inner = Panel::framed(Some(frame.title))
            .focused(focused)
            .meta(&meta)
            .render(framed, buf, t);
        let bg = t.canvas;
        self.scroll.set_content(frame.rows.len());
        self.scroll.set_viewport(inner.height as usize);
        self.scroll.follow(self.cursor);
        ctx.control(self.master, inner, false);
        ctx.scrollable(self.master, inner);
        let has_sb = self.scroll.overflows();
        let row_w = inner.width.saturating_sub(u16::from(has_sb));
        #[cfg(test)]
        crate::core::work::add(crate::core::work::Counter::WidthRows, frame.rows.len());
        let label_w = frame
            .rows
            .iter()
            .map(|r| width(&r.label))
            .max()
            .unwrap_or(8)
            .clamp(8, 22);
        let (detail_w, meta_w) = frame.rows.iter().fold((0, 0), |(d, m), r| {
            (d.max(width(&r.detail) as u16), m.max(width(&r.meta) as u16))
        });
        for (k, i) in self.scroll.visible_range().enumerate() {
            let y = inner.y + k as u16;
            let row = &frame.rows[i];
            let rid = self.master.child(i);
            let mut s = ctx.state(rid);
            s.focused = focused && self.cursor == i;
            s.selected = self.cursor == i;
            let st = t.row(s, bg);
            let rect = Rect::new(inner.x - 1, y, row_w + 1, 1);
            fill(buf, rect, st);
            buf.set_string(
                rect.x,
                y,
                t.gutter_symbol(s),
                t.gutter(s, st.bg.unwrap_or(bg), false),
            );
            let mut x = rect.x + 2;
            if let Some(kind) = row.mark {
                let (glyph, gs) = t.status_at(kind, ctx.interaction.tick);
                buf.set_string(x, y, glyph, gs.bg(st.bg.unwrap_or(bg)));
            }
            x += 2;
            let right = rect.right().saturating_sub(1);
            let lw = (label_w as u16).min(right.saturating_sub(x));
            let label_style = if row.faint {
                st.fg(t.text_muted)
            } else if s.selected {
                st.fg(t.accent)
            } else {
                st
            };
            buf.set_string(
                x,
                y,
                fit(&truncate(&row.label, lw as usize), lw as usize),
                label_style,
            );
            let dx = x + lw + COLUMN_GAP;
            let (detail_col, meta_col) = columns(right.saturating_sub(dx), detail_w, meta_w);
            if !row.detail.is_empty() && detail_col > 0 {
                buf.set_string(
                    dx,
                    y,
                    truncate(&row.detail, detail_col as usize),
                    st.fg(t.text_muted).remove_modifier(Modifier::BOLD),
                );
            }
            if !row.meta.is_empty() && meta_col > 0 {
                let meta = truncate(&row.meta, meta_col as usize);
                buf.set_string(
                    right - width(&meta) as u16,
                    y,
                    &meta,
                    st.fg(t.tone(row.meta_tone)).remove_modifier(Modifier::BOLD),
                );
            }
            legible_on_fill(buf, rect, t, bg);
            ctx.clickable(rid, rect);
        }
        if has_sb {
            scrollbar::render_with_fade(inner, buf, ctx, self.master, &self.scroll, focused);
        }
        for (k, text) in under_lines.iter().take(under as usize).enumerate() {
            let y = area.bottom() - under + k as u16;
            buf.set_string(area.x + 1, y, text, t.faint().bg(t.canvas));
        }
    }

    fn rendered(&mut self, lines: &[Line], body: Rect, label_w: u16) -> Vec<Rendered> {
        let vw = body.width.saturating_sub(label_w).max(8) as usize;
        let mut out = vec![];
        let mut items = 0;
        self.item_rows.clear();
        for (n, line) in lines.iter().enumerate() {
            match line {
                Line::Prop(k, v, tone) => {
                    for (i, p) in wrap(v, vw).into_iter().enumerate() {
                        let k = if i == 0 { k.clone() } else { String::new() };
                        out.push(Rendered::Prop(k, p, *tone));
                    }
                }
                Line::Text(v, tone) => {
                    for p in wrap(v, body.width.max(8) as usize) {
                        out.push(Rendered::Text(p, *tone));
                    }
                }
                Line::Heading(h, m) => out.push(Rendered::Heading(h.clone(), m.clone())),
                Line::Item { .. } => {
                    self.item_rows.push(out.len());
                    out.push(Rendered::Item(n, items));
                    items += 1;
                }
                Line::Notice(notice) => {
                    let h = notice.height(body.width).max(1);
                    out.push(Rendered::Notice(n, h));
                    for _ in 1..h {
                        out.push(Rendered::Skip);
                    }
                }
                Line::Blank => out.push(Rendered::Blank),
            }
        }
        self.items = items;
        out
    }

    fn draw_detail(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        frame: &Frame,
        as_drawer: bool,
    ) {
        let t = ctx.theme;
        let focused =
            ctx.interaction.focused(self.detail) || self.actions_focused(ctx.interaction.focus);
        let inner = if as_drawer {
            Panel::framed(Some(&frame.detail_title))
                .focused(focused)
                .meta(&frame.detail_meta)
                .render(area, buf, t)
        } else {
            Panel::card(Some(&frame.detail_title))
                .focused(focused)
                .focus_mark(if ctx.interaction.focused(self.detail) {
                    PanelFocus::Bar
                } else {
                    PanelFocus::Title
                })
                .meta(&frame.detail_meta)
                .render(area, buf, t)
        };
        let bg = if as_drawer { t.canvas } else { t.surface };
        let row = ButtonRow::new().overflow(RowOverflow::Wrap);
        let action_rows = row.rows(inner.width + 1, &self.buttons, t);
        let body_h = inner.height.saturating_sub(1 + action_rows);
        let mut body = Rect::new(inner.x, inner.y, inner.width, body_h);
        let label_w: u16 = frame
            .lines
            .iter()
            .map(|l| match l {
                Line::Prop(k, _, _) => width(k) as u16,
                _ => 0,
            })
            .max()
            .unwrap_or(13)
            .clamp(13, 22)
            + 1;
        let mut rendered = self.rendered(&frame.lines, body, label_w);
        if rendered.len() > body_h as usize {
            body.width = inner
                .width
                .saturating_sub(scrollbar::reserve(scrollbar::GAP));
            rendered = self.rendered(&frame.lines, body, label_w);
        }
        self.item_cursor = self.item_cursor.min(self.items.saturating_sub(1));
        self.detail_scroll.set_content(rendered.len());
        self.detail_scroll.set_viewport(body_h as usize);
        ctx.control(self.detail, body, false);
        ctx.scrollable(self.detail, body);
        let detail_focused = ctx.interaction.focused(self.detail);
        if detail_focused && !self.detail_had_focus {
            self.reveal_item();
        }
        self.detail_had_focus = detail_focused;
        for (k, i) in self.detail_scroll.visible_range().enumerate() {
            let y = body.y + k as u16;
            match &rendered[i] {
                Rendered::Prop(label, value, tone) => {
                    buf.set_string(body.x, y, label, t.muted().bg(bg));
                    let vx = body.x + label_w;
                    buf.set_string(
                        vx,
                        y,
                        crate::ui::reveal::cut(
                            Rect::new(vx, y, body.width.saturating_sub(label_w), 1),
                            value,
                        ),
                        Style::new().fg(t.tone(*tone)).bg(bg),
                    );
                }
                Rendered::Text(text, tone) => {
                    buf.set_string(
                        body.x,
                        y,
                        crate::ui::reveal::cut(Rect::new(body.x, y, body.width, 1), text),
                        Style::new().fg(t.tone(*tone)).bg(bg),
                    );
                }
                Rendered::Heading(h, meta) => {
                    buf.set_string(
                        body.x,
                        y,
                        h,
                        t.secondary().bg(bg).add_modifier(Modifier::BOLD),
                    );
                    let room = body.width.saturating_sub(width(h) as u16 + 3) as usize;
                    let meta = truncate(meta, room);
                    let mw = width(&meta) as u16;
                    if mw > 0 && room >= 8 {
                        buf.set_string(body.right() - mw, y, &meta, t.faint().bg(bg));
                    }
                }
                Rendered::Item(n, item_no) => {
                    let Line::Item { mark, cols, tone } = &frame.lines[*n] else {
                        continue;
                    };
                    let id = self.detail.child(*item_no).sub("item");
                    let mut s = ctx.state(id);
                    s.focused = detail_focused && self.item_cursor == *item_no;
                    s.selected = self.item_cursor == *item_no;
                    let st = t.row(s, bg);
                    let rect = Rect::new(body.x - 1, y, body.width + 1, 1);
                    fill(buf, rect, st);
                    buf.set_string(
                        rect.x,
                        y,
                        t.gutter_symbol(s),
                        t.gutter(s, st.bg.unwrap_or(bg), false),
                    );
                    let mut x = rect.x + 2;
                    if let Some(kind) = mark {
                        let (glyph, gs) = t.status(*kind);
                        buf.set_string(x, y, glyph, gs.bg(st.bg.unwrap_or(bg)));
                    }
                    x += 2;
                    let avail = rect.right().saturating_sub(x) as usize;
                    let c0 = 11usize.min(avail);
                    let wide = |n: usize| {
                        frame
                            .lines
                            .iter()
                            .filter_map(|l| match l {
                                Line::Item { cols, .. } => Some(width(&cols[n])),
                                _ => None,
                            })
                            .max()
                            .unwrap_or(0)
                    };
                    let room = (body.width as usize).saturating_sub(c0 + wide(2) + 5);
                    let c1 = wide(1).min(room).max(12).min(avail.saturating_sub(c0 + 2));
                    buf.set_string(x, y, fit(&truncate(&cols[0], c0), c0), st);
                    buf.set_string(
                        x + c0 as u16 + 1,
                        y,
                        fit(&truncate(&cols[1], c1), c1),
                        st.fg(t.tone(*tone)),
                    );
                    let x2 = x + (c0 + c1 + 2) as u16;
                    let room = rect.right().saturating_sub(x2 + 1) as usize;
                    buf.set_string(x2, y, truncate(&cols[2], room), st.fg(t.text_muted));
                    legible_on_fill(buf, rect, t, bg);
                    ctx.clickable(id, rect);
                }
                Rendered::Notice(n, h) => {
                    if let Line::Notice(notice) = &frame.lines[*n] {
                        let room = body.bottom().saturating_sub(y).min(*h);
                        notice.render(Rect::new(body.x, y, body.width, room), buf, ctx, bg);
                    }
                }
                Rendered::Skip | Rendered::Blank => {}
            }
        }
        if self.detail_scroll.overflows() {
            let bar = Rect::new(inner.x, body.y, inner.width, body_h);
            scrollbar::render_with_fade(
                bar,
                buf,
                ctx,
                self.detail,
                &self.detail_scroll,
                ctx.interaction.focused(self.detail),
            );
        }
        let ay = inner.bottom().saturating_sub(action_rows);
        row.render(
            self.detail,
            Rect::new(inner.x - 1, ay, inner.width + 1, action_rows),
            &mut self.buttons,
            buf,
            ctx,
            bg,
        );
    }

    fn draw_summary(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, frame: &Frame) {
        let t = ctx.theme;
        let bg = t.surface;
        let inner = Panel::card(Some(&frame.detail_title))
            .meta(&format!("{} · Tab details", frame.detail_meta))
            .render(area, buf, t);
        let mut y = inner.y;
        for line in &frame.lines {
            let text = match line {
                Line::Prop(k, v, tone) if !k.is_empty() => Some((format!("{k}  {v}"), *tone)),
                Line::Text(v, tone) => Some((v.clone(), *tone)),
                Line::Notice(n) => Some((n.message.clone(), n.kind.message_tone())),
                _ => None,
            };
            if let Some((text, tone)) = text {
                buf.set_string(
                    inner.x,
                    y,
                    crate::ui::reveal::cut(Rect::new(inner.x, y, inner.width, 1), &text),
                    Style::new().fg(t.tone(tone)).bg(bg),
                );
                y += 1;
            }
            if y >= inner.bottom() {
                break;
            }
        }
        ctx.clickable(self.detail, area);
    }
}

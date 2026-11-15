use crate::core::event::{Key, Outcome};
use crate::core::focus::Focus;
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keyed::{KeyedSourceError, KeyedState};
use crate::core::list::{KeyedRowSource, SourceStamp};
use crate::core::pointer::PointerEvent;
use crate::core::scroll::{ScrollState, TrackClick};
use crate::theme::{ButtonKind, StatusKind, Tone};
use crate::ui::ctx::{RenderCtx, fill, legible_on_fill};
use crate::ui::layout::Split;
use crate::ui::master_detail::{MasterDetail, Mode};
use crate::ui::text::{fit, paint_clipped, paint_truncated, truncate, width, wrap};
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

fn cells(text: &str) -> u16 {
    u16::try_from(width(text)).unwrap_or(u16::MAX)
}

/// Paints `text` on `row` starting at absolute column `x`, clipped to `row`.
fn put(buf: &mut Buffer, row: Rect, x: u16, text: &str, style: Style) {
    if let Some(col) = x.checked_sub(row.x) {
        paint_clipped(buf, row, usize::from(col), text, style);
    }
}

/// `inner` clipped to `area`, collapsed to an empty rectangle at `area`'s
/// origin when nothing is left, so no child is placed beside its parent.
fn within(inner: Rect, area: Rect) -> Rect {
    let clipped = inner.intersection(area);
    if clipped.is_empty() {
        Rect::new(area.x, area.y, 0, 0)
    } else {
        clipped
    }
}

fn columns(rest: u16, detail: u16, meta: u16) -> (u16, u16) {
    let both = detail > 0 && meta > 0;
    let gap = if both { COLUMN_GAP } else { 0 };
    let (mut d, mut m) = if detail.saturating_add(gap).saturating_add(meta) <= rest {
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

#[derive(Debug, Clone, PartialEq)]
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

/// One master row borrowed from its provider.
#[derive(Debug, Clone, Copy)]
pub struct ListRowRef<'a> {
    pub mark: Option<StatusKind>,
    pub label: &'a str,
    pub detail: &'a str,
    pub meta: &'a str,
    pub meta_tone: Tone,
    pub faint: bool,
}

impl ListRow {
    /// The borrowed facet the shared renderer reads.
    pub fn borrowed(&self) -> ListRowRef<'_> {
        ListRowRef {
            mark: self.mark,
            label: &self.label,
            detail: &self.detail,
            meta: &self.meta,
            meta_tone: self.meta_tone,
            faint: self.faint,
        }
    }
}

impl crate::core::keyed::KeyedRecord for ListRow {
    fn label(&self) -> &str {
        &self.label
    }
}

impl ListDetailSource for crate::core::keyed::KeyedVec<ListRow> {
    fn row(&self, visible: usize) -> ListRowRef<'_> {
        self.value_at(visible)
            .expect("visible row within the store")
            .borrowed()
    }
}

/// A keyed provider of master rows. It is the only master-row provider of a
/// [`SourceFrame`]; the frame deliberately has no `rows` field.
pub trait ListDetailSource: KeyedRowSource {
    fn row(&self, visible: usize) -> ListRowRef<'_>;
}

/// Everything but the master rows, borrowed for one frame.
pub struct SourceFrame<'a> {
    pub title: &'a str,
    pub meta: &'a str,
    pub head: &'a [Span],
    pub under: Option<&'a str>,
    pub detail_title: &'a str,
    pub detail_meta: &'a str,
    pub lines: &'a [Line],
    pub actions: &'a [Action],
}

impl<'a> Frame<'a> {
    fn source_frame(&'a self) -> SourceFrame<'a> {
        SourceFrame {
            title: self.title,
            meta: &self.meta,
            head: &self.head,
            under: self.under,
            detail_title: &self.detail_title,
            detail_meta: &self.detail_meta,
            lines: &self.lines,
            actions: &self.actions,
        }
    }
}

/// The master rows the shared renderer reads: an owned slice, or a keyed
/// provider whose aggregate widths are cached by its [`SourceStamp`].
trait MasterRows {
    fn len(&self) -> usize;
    fn row(&self, i: usize) -> ListRowRef<'_>;
    fn stamp(&self) -> Option<SourceStamp> {
        None
    }
    fn hit_key(&self, _i: usize) -> Option<ItemKey> {
        None
    }
}

impl MasterRows for [ListRow] {
    fn len(&self) -> usize {
        <[ListRow]>::len(self)
    }
    fn row(&self, i: usize) -> ListRowRef<'_> {
        self[i].borrowed()
    }
}

struct SourceRows<'a, S: ?Sized>(&'a S);

impl<S: ListDetailSource + ?Sized> MasterRows for SourceRows<'_, S> {
    fn len(&self) -> usize {
        self.0.len()
    }
    fn row(&self, i: usize) -> ListRowRef<'_> {
        self.0.row(i)
    }
    fn stamp(&self) -> Option<SourceStamp> {
        Some(self.0.stamp())
    }
    fn hit_key(&self, i: usize) -> Option<ItemKey> {
        Some(self.0.key(i))
    }
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
    keyed: KeyedState,
    synced_cursor: Option<usize>,
    widths: Option<(SourceStamp, usize, u16, u16)>,
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
            keyed: KeyedState::new(),
            synced_cursor: None,
            widths: None,
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
        self.render_in(frame.rows, &frame.source_frame(), area, buf, ctx);
    }

    /// Paints master rows borrowed from a keyed provider. Aggregate column
    /// widths are cached by the provider's full [`SourceStamp`], so an
    /// unchanged frame measures no hidden row.
    pub fn render_source<S: ListDetailSource + ?Sized>(
        &mut self,
        area: Rect,
        source: &S,
        frame: &SourceFrame<'_>,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
    ) -> Result<(), KeyedSourceError> {
        self.bind_source(source)?;
        self.render_in(&SourceRows(source), frame, area, buf, ctx);
        self.synced_cursor = Some(self.cursor);
        Ok(())
    }

    /// Reconciles the selection with `source` without painting: a cursor the
    /// user moved is adopted by key, then the view index follows the key
    /// through reorder, filter and deletion. Hosts call it before handling an
    /// event on rows that may have changed since the last frame.
    pub fn bind_source<S: ListDetailSource + ?Sized>(
        &mut self,
        source: &S,
    ) -> Result<(), KeyedSourceError> {
        if let Some(synced) = self.synced_cursor
            && self.cursor != synced
        {
            self.keyed.adopt_cursor_view(self.cursor);
        }
        let first = self.keyed.stamp().is_none();
        let out = self.keyed.reconcile(source)?;
        if first || out.data_changed || out.view_changed || out.cursor_changed {
            self.cursor = self.keyed.cursor_view().unwrap_or(0);
        }
        self.synced_cursor = Some(self.cursor);
        Ok(())
    }

    /// The stable-identity state of a list driven through `render_source`.
    pub fn keyed(&self) -> &KeyedState {
        &self.keyed
    }

    /// The key of the selected master row, once a source has been rendered.
    pub fn selected_key(&self) -> Option<ItemKey> {
        self.keyed.cursor()
    }

    /// Click handling for a list rendered with `render_source`: master rows
    /// are identified by key, so a click on a row that moved still reaches
    /// the same logical record.
    pub fn click_source(&mut self, id: WidgetId, focus: &mut Focus) -> (Outcome, Option<Event>) {
        for i in self.scroll.visible_range() {
            if let Some(key) = self.keyed.visible_key(i)
                && self.master_hit(Some(key), i) == id
            {
                let same = self.cursor == i;
                if !same {
                    self.select(i);
                }
                self.keyed.adopt_cursor_view(i);
                self.synced_cursor = Some(self.cursor);
                self.md.close_drawer(focus);
                return (Outcome::Changed, Some(Event::Row(i)));
            }
        }
        let len = self.keyed.visible_len();
        self.click(id, focus, len)
    }

    fn master_hit(&self, key: Option<ItemKey>, i: usize) -> WidgetId {
        match key {
            Some(key) => self.master.part(PartRef::item(Part::ROW, key)),
            None => self.master.child(i),
        }
    }

    fn render_in<R: MasterRows + ?Sized>(
        &mut self,
        rows: &R,
        frame: &SourceFrame<'_>,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
    ) {
        let t = ctx.theme;
        self.names = frame.actions.iter().map(|a| a.name).collect();
        self.buttons = frame
            .actions
            .iter()
            .map(|a| Button::new(self.detail.sub(a.name), &a.label, a.kind).disabled(a.disabled))
            .collect();
        self.md.detail_owns = self.buttons.iter().map(|b| b.id).collect();
        self.clamp(rows.len());
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            self.scroll.set_viewport(0);
            self.detail_scroll.set_viewport(0);
            return;
        }
        let l = self.md.layout(area, buf, ctx, t.canvas);
        if !l.master.is_empty() {
            self.draw_list(l.master, buf, ctx, rows, frame);
        }
        match l.mode {
            Mode::Split => self.draw_detail(l.detail, buf, ctx, frame, false),
            Mode::Drawer => self.draw_detail(l.detail, buf, ctx, frame, true),
            Mode::Master => self.draw_summary(l.summary, buf, ctx, frame),
        }
        self.md.finish(ctx);
    }

    fn aggregate_widths<R: MasterRows + ?Sized>(&mut self, rows: &R) -> (usize, u16, u16) {
        if let Some(stamp) = rows.stamp()
            && let Some((cached, label, detail, meta)) = self.widths
            && cached == stamp
        {
            return (label, detail, meta);
        }
        #[cfg(test)]
        crate::core::work::add(crate::core::work::Counter::WidthRows, rows.len());
        let mut label = 0;
        let (mut detail, mut meta) = (0u16, 0u16);
        for i in 0..rows.len() {
            let row = rows.row(i);
            label = label.max(width(row.label));
            detail = detail.max(cells(row.detail));
            meta = meta.max(cells(row.meta));
        }
        let label = if rows.len() == 0 { 8 } else { label };
        if let Some(stamp) = rows.stamp() {
            self.widths = Some((stamp, label, detail, meta));
        }
        (label, detail, meta)
    }

    fn draw_list<R: MasterRows + ?Sized>(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        rows: &R,
        frame: &SourceFrame<'_>,
    ) {
        let t = ctx.theme;
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            self.scroll.set_viewport(0);
            return;
        }
        let focused = ctx.interaction.focused(self.master);
        let pos = scrollbar::position_label(&self.scroll);
        let meta = if pos.is_empty() {
            frame.meta.to_owned()
        } else {
            format!("{} · {pos}", frame.meta)
        };
        let over = u16::from(!frame.head.is_empty() && area.height > 0);
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
                frame.head,
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
        let inner = within(inner, area);
        let lead = inner.x.saturating_sub(1).max(area.x);
        let bg = t.canvas;
        self.scroll.set_content(rows.len());
        self.scroll.set_viewport(inner.height as usize);
        self.scroll.follow(self.cursor);
        ctx.control(self.master, inner, false);
        ctx.scrollable(self.master, inner);
        let has_sb = self.scroll.overflows();
        let row_w = inner.width.saturating_sub(u16::from(has_sb));
        let (label_w, detail_w, meta_w) = self.aggregate_widths(rows);
        let label_w = label_w.clamp(8, 22);
        for (k, i) in self.scroll.visible_range().enumerate() {
            let y = inner.y + k as u16;
            let row = rows.row(i);
            let rid = self.master_hit(rows.hit_key(i), i);
            let mut s = ctx.state(rid);
            s.focused = focused && self.cursor == i;
            s.selected = self.cursor == i;
            let st = t.row(s, bg);
            let rect = Rect::new(
                lead,
                y,
                row_w.saturating_add(inner.x.saturating_sub(lead)),
                1,
            )
            .intersection(area);
            fill(buf, rect, st);
            put(
                buf,
                rect,
                rect.x,
                t.gutter_symbol(s),
                t.gutter(s, st.bg.unwrap_or(bg), false),
            );
            let mut x = rect.x.saturating_add(2);
            if let Some(kind) = row.mark {
                let (glyph, gs) = t.status_at(kind, ctx.interaction.tick);
                put(buf, rect, x, glyph, gs.bg(st.bg.unwrap_or(bg)));
            }
            x = x.saturating_add(2);
            let right = rect.right().saturating_sub(1);
            let lw = (label_w as u16).min(right.saturating_sub(x));
            let label_style = if row.faint {
                st.fg(t.text_muted)
            } else if s.selected {
                st.fg(t.accent)
            } else {
                st
            };
            put(
                buf,
                rect,
                x,
                &fit(&truncate(row.label, lw as usize), lw as usize),
                label_style,
            );
            let dx = x.saturating_add(lw).saturating_add(COLUMN_GAP);
            let (detail_col, meta_col) = columns(right.saturating_sub(dx), detail_w, meta_w);
            if !row.detail.is_empty() && detail_col > 0 {
                put(
                    buf,
                    rect,
                    dx,
                    &truncate(row.detail, detail_col as usize),
                    st.fg(t.text_muted).remove_modifier(Modifier::BOLD),
                );
            }
            if !row.meta.is_empty() && meta_col > 0 {
                let meta = truncate(row.meta, meta_col as usize);
                put(
                    buf,
                    rect,
                    right.saturating_sub(cells(&meta)),
                    &meta,
                    st.fg(t.tone(row.meta_tone)).remove_modifier(Modifier::BOLD),
                );
            }
            legible_on_fill(buf, rect, t, bg);
            if !rect.is_empty() {
                ctx.clickable(rid, rect);
            }
        }
        if has_sb {
            scrollbar::render_with_fade(inner, buf, ctx, self.master, &self.scroll, focused);
        }
        for (k, text) in under_lines.iter().take(under as usize).enumerate() {
            let y = area.bottom() - under + k as u16;
            let line = Rect::new(area.x, y, area.width, 1);
            paint_clipped(buf, line, 1, text, t.faint().bg(t.canvas));
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
        frame: &SourceFrame<'_>,
        as_drawer: bool,
    ) {
        let t = ctx.theme;
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            self.detail_scroll.set_viewport(0);
            return;
        }
        let focused =
            ctx.interaction.focused(self.detail) || self.actions_focused(ctx.interaction.focus);
        let inner = if as_drawer {
            Panel::framed(Some(frame.detail_title))
                .focused(focused)
                .meta(frame.detail_meta)
                .render(area, buf, t)
        } else {
            Panel::card(Some(frame.detail_title))
                .focused(focused)
                .focus_mark(if ctx.interaction.focused(self.detail) {
                    PanelFocus::Bar
                } else {
                    PanelFocus::Title
                })
                .meta(frame.detail_meta)
                .render(area, buf, t)
        };
        let inner = within(inner, area);
        let bg = if as_drawer { t.canvas } else { t.surface };
        let lead = inner.x.saturating_sub(1).max(area.x);
        let wide = inner.width.saturating_add(inner.x.saturating_sub(lead));
        let row = ButtonRow::new().overflow(RowOverflow::Wrap);
        let action_rows = row.rows(wide, &self.buttons, t);
        let body_h = inner.height.saturating_sub(1 + action_rows);
        let mut body = Rect::new(inner.x, inner.y, inner.width, body_h);
        let label_w = frame
            .lines
            .iter()
            .map(|l| match l {
                Line::Prop(k, _, _) => width(k),
                _ => 0,
            })
            .max()
            .unwrap_or(13)
            .clamp(13, 22)
            + 1;
        let label_w = u16::try_from(label_w).unwrap_or(u16::MAX).min(body.width);
        let mut rendered = self.rendered(frame.lines, body, label_w);
        if rendered.len() > body_h as usize {
            body.width = inner
                .width
                .saturating_sub(scrollbar::reserve(scrollbar::GAP));
            rendered = self.rendered(frame.lines, body, label_w);
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
            let line = Rect::new(body.x, y, body.width, 1);
            match &rendered[i] {
                Rendered::Prop(label, value, tone) => {
                    let key = Rect::new(body.x, y, label_w.saturating_sub(1), 1);
                    paint_truncated(buf, key, 0, label, t.muted().bg(bg));
                    let value_room = Rect::new(
                        body.x.saturating_add(label_w),
                        y,
                        body.width.saturating_sub(label_w),
                        1,
                    );
                    paint_clipped(
                        buf,
                        value_room,
                        0,
                        &crate::ui::reveal::cut(value_room, value),
                        Style::new().fg(t.tone(*tone)).bg(bg),
                    );
                }
                Rendered::Text(text, tone) => {
                    paint_clipped(
                        buf,
                        line,
                        0,
                        &crate::ui::reveal::cut(line, text),
                        Style::new().fg(t.tone(*tone)).bg(bg),
                    );
                }
                Rendered::Heading(h, meta) => {
                    paint_truncated(
                        buf,
                        line,
                        0,
                        h,
                        t.secondary().bg(bg).add_modifier(Modifier::BOLD),
                    );
                    let room = usize::from(body.width).saturating_sub(width(h) + 3);
                    let meta = truncate(meta, room);
                    let mw = width(&meta);
                    if mw > 0 && room >= 8 {
                        paint_clipped(
                            buf,
                            line,
                            usize::from(body.width) - mw,
                            &meta,
                            t.faint().bg(bg),
                        );
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
                    let rect =
                        Rect::new(lead, y, body.right().saturating_sub(lead), 1).intersection(area);
                    fill(buf, rect, st);
                    put(
                        buf,
                        rect,
                        rect.x,
                        t.gutter_symbol(s),
                        t.gutter(s, st.bg.unwrap_or(bg), false),
                    );
                    let mut x = rect.x.saturating_add(2);
                    if let Some(kind) = mark {
                        let (glyph, gs) = t.status(*kind);
                        put(buf, rect, x, glyph, gs.bg(st.bg.unwrap_or(bg)));
                    }
                    x = x.saturating_add(2);
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
                    let at = |n: usize| u16::try_from(usize::from(x) + n).unwrap_or(u16::MAX);
                    put(buf, rect, x, &fit(&truncate(&cols[0], c0), c0), st);
                    put(
                        buf,
                        rect,
                        at(c0 + 1),
                        &fit(&truncate(&cols[1], c1), c1),
                        st.fg(t.tone(*tone)),
                    );
                    let x2 = at(c0 + c1 + 2);
                    let room = usize::from(rect.right().saturating_sub(x2.saturating_add(1)));
                    put(
                        buf,
                        rect,
                        x2,
                        &truncate(&cols[2], room),
                        st.fg(t.text_muted),
                    );
                    legible_on_fill(buf, rect, t, bg);
                    if !rect.is_empty() {
                        ctx.clickable(id, rect);
                    }
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
        let footer = action_rows.min(inner.height);
        row.render(
            self.detail,
            Rect::new(lead, inner.bottom() - footer, wide, footer),
            &mut self.buttons,
            buf,
            ctx,
            bg,
        );
    }

    fn draw_summary(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        frame: &SourceFrame<'_>,
    ) {
        let t = ctx.theme;
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }
        let bg = t.surface;
        let inner = Panel::card(Some(frame.detail_title))
            .meta(&format!("{} · Tab details", frame.detail_meta))
            .render(area, buf, t);
        let inner = within(inner, area);
        let mut y = inner.y;
        for line in frame.lines {
            if y >= inner.bottom() {
                break;
            }
            let text = match line {
                Line::Prop(k, v, tone) if !k.is_empty() => Some((format!("{k}  {v}"), *tone)),
                Line::Text(v, tone) => Some((v.clone(), *tone)),
                Line::Notice(n) => Some((n.message.clone(), n.kind.message_tone())),
                _ => None,
            };
            if let Some((text, tone)) = text {
                let row = Rect::new(inner.x, y, inner.width, 1);
                paint_clipped(
                    buf,
                    row,
                    0,
                    &crate::ui::reveal::cut(row, &text),
                    Style::new().fg(t.tone(tone)).bg(bg),
                );
                y += 1;
            }
        }
        ctx.clickable(self.detail, area);
    }
}

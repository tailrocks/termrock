use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;

use crate::core::event::Key;
use crate::core::hit::Axis;
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keymap::{Nav, NavKeys};
use crate::core::list::{Cursor, KeyStep, Moved, RowSource};
use crate::core::pointer::PointerEvent;
use crate::core::response::Response;
use crate::core::scroll::{ScrollPane, ScrollState, Scrollable};
use crate::core::typeahead::TypeAhead;
use crate::theme::CursorStyle;
use crate::ui::ctx::RenderCtx;
use crate::ui::popup::{Dismiss, Place, Placement, backdrop_id};
use crate::ui::row::{RowLook, RowPaint, RowUi};
use crate::widgets::edit::EditField;
use crate::widgets::scrollbar;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectArrows {
    Step,

    Open,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WheelMode {
    #[default]
    Viewport,

    Cursor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub key: Option<ItemKey>,
    pub label: String,

    pub glyph: Cow<'static, str>,
    pub detail: String,

    pub tag: Option<Cow<'static, str>>,

    pub group: Cow<'static, str>,

    pub matched: Vec<usize>,

    pub insert: Option<String>,

    pub disabled: bool,

    pub reason: Option<String>,

    pub selection_group: Option<ItemKey>,

    pub value: usize,
}

impl Default for Item {
    fn default() -> Self {
        Self::new("")
    }
}

impl Item {
    /// Text bytes a deep clone of this item copies. A borrowed `Cow` copies
    /// no bytes.
    #[cfg(feature = "work-metrics")]
    pub(crate) fn payload_bytes(&self) -> usize {
        let owned = |text: &Cow<'static, str>| match text {
            Cow::Owned(text) => text.len(),
            Cow::Borrowed(_) => 0,
        };
        self.label.len()
            + self.detail.len()
            + owned(&self.glyph)
            + self.tag.as_ref().map_or(0, owned)
            + owned(&self.group)
            + self.insert.as_ref().map_or(0, String::len)
            + self.reason.as_ref().map_or(0, String::len)
    }

    pub fn new(label: impl Into<String>) -> Self {
        Self {
            key: None,
            label: label.into(),
            glyph: Cow::Borrowed(""),
            detail: String::new(),
            tag: None,
            group: Cow::Borrowed(""),
            matched: vec![],
            insert: None,
            disabled: false,
            reason: None,
            selection_group: None,
            value: 0,
        }
    }

    pub fn key(mut self, key: ItemKey) -> Self {
        self.key = Some(key);
        self
    }

    pub fn glyph(mut self, glyph: impl Into<Cow<'static, str>>) -> Self {
        self.glyph = glyph.into();
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    pub fn tag(mut self, tag: impl Into<Cow<'static, str>>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn group(mut self, group: impl Into<Cow<'static, str>>) -> Self {
        self.group = group.into();
        self
    }

    pub fn matched(mut self, matched: Vec<usize>) -> Self {
        self.matched = matched;
        self
    }

    pub fn insert(mut self, insert: impl Into<String>) -> Self {
        self.insert = Some(insert.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self.disabled = true;
        self
    }

    pub fn selection_group(mut self, group: ItemKey) -> Self {
        self.selection_group = Some(group);
        self
    }

    pub fn value(mut self, value: usize) -> Self {
        self.value = value;
        self
    }

    pub fn insertion(&self) -> &str {
        self.insert.as_deref().unwrap_or(&self.label)
    }
}

pub trait AsItem {
    fn as_item(&self) -> &Item;
}

impl AsItem for Item {
    fn as_item(&self) -> &Item {
        self
    }
}

impl<T: AsItem> RowSource for [T] {
    fn len(&self) -> usize {
        <[T]>::len(self)
    }
    fn enabled(&self, i: usize) -> bool {
        !self[i].as_item().disabled
    }
    fn label(&self, i: usize) -> &str {
        &self[i].as_item().label
    }
    fn key(&self, i: usize) -> ItemKey {
        self[i].as_item().key.unwrap_or(ItemKey::index(i))
    }
}

#[derive(Debug, Clone)]
pub struct OptionList {
    pub id: WidgetId,

    pub cursor: Cursor,

    pub scroll: ScrollPane,

    pub look: CursorStyle,

    pub wheel: WheelMode,

    pub hover_moves_cursor: bool,

    pub nav: NavKeys,

    pub type_ahead: bool,

    pub fade_edges: bool,

    pub dismiss: Dismiss,

    pub placement: Placement,

    pub fit: Place,

    pub max_rows: u16,

    pub width: (u16, u16),

    pub query: Option<Box<EditField>>,

    pub chosen: Option<usize>,
    pub checked: Vec<ItemKey>,

    pub clear_on_leave: bool,
    typeahead: TypeAhead,

    hover: Option<WidgetId>,

    pointed: Option<usize>,

    hovering: bool,

    hidden: bool,

    rows_area: Rect,

    lines: Vec<usize>,
}

impl OptionList {
    pub const MAX_ROWS: u16 = 8;

    pub const WIDTH: (u16, u16) = (12, 40);

    pub fn new(id: WidgetId) -> Self {
        Self {
            id,
            cursor: Cursor::new(0),
            scroll: ScrollPane::default(),
            look: CursorStyle::default(),
            wheel: WheelMode::Viewport,
            hover_moves_cursor: true,
            nav: NavKeys::default(),
            type_ahead: crate::core::list::NavProfile::current().type_ahead(),
            fade_edges: true,
            dismiss: Dismiss::LIGHT,
            placement: Placement::Below,
            fit: Place::DEFAULT,
            max_rows: Self::MAX_ROWS,
            width: Self::WIDTH,
            query: None,
            chosen: None,
            checked: Vec::new(),
            clear_on_leave: true,
            typeahead: TypeAhead::default(),
            hover: None,
            pointed: None,
            hovering: false,
            hidden: false,
            rows_area: Rect::ZERO,
            lines: Vec::new(),
        }
    }

    pub fn type_ahead_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.typeahead.timeout = timeout;
        self
    }

    pub fn set_gaps(&mut self, gaps: impl IntoIterator<Item = bool>) {
        self.lines.clear();
        let mut line = 0;
        let mut any = false;
        for gap in gaps {
            self.lines.push(line);
            line += 1 + usize::from(gap);
            any |= gap;
        }
        self.lines.push(line);
        if !any {
            self.lines.clear();
        }
    }

    pub fn line_of(&self, i: usize) -> usize {
        match self.lines.get(i) {
            Some(&l) => l,
            None => i,
        }
    }

    fn row_on_line(&self, line: usize) -> Option<usize> {
        if self.lines.is_empty() {
            return Some(line);
        }
        let i = self.lines.binary_search(&line).ok()?;
        (i + 1 < self.lines.len()).then_some(i)
    }

    fn line_count(&self, len: usize) -> usize {
        if self.lines.len() == len + 1 {
            self.lines[len]
        } else {
            len
        }
    }

    pub fn visible_lines(&self) -> impl Iterator<Item = (u16, Option<usize>)> + '_ {
        self.scroll
            .visible_range()
            .enumerate()
            .map(|(k, line)| (k as u16, self.row_on_line(line)))
    }

    pub fn more_below(&self, len: usize) -> bool {
        len > 0 && self.line_of(len - 1) >= self.scroll.offset + self.scroll.viewport_len
    }

    pub fn wheel_mode(mut self, mode: WheelMode) -> Self {
        self.wheel = mode;
        self
    }

    pub fn hover_moves_cursor(mut self, on: bool) -> Self {
        self.hover_moves_cursor = on;
        self
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn type_ahead(mut self, on: bool) -> Self {
        self.type_ahead = on;
        self
    }

    pub fn fade_edges(mut self, on: bool) -> Self {
        self.fade_edges = on;
        self
    }

    pub fn look(mut self, look: CursorStyle) -> Self {
        self.look = look;
        self
    }

    pub fn dismiss(mut self, dismiss: Dismiss) -> Self {
        self.dismiss = dismiss;
        self
    }

    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    pub fn fit(mut self, fit: Place) -> Self {
        self.fit = fit;
        self
    }

    pub fn max_rows(mut self, rows: u16) -> Self {
        self.max_rows = rows;
        self
    }

    pub fn width(mut self, min: u16, max: u16) -> Self {
        self.width = (min, max.max(min));
        self
    }

    pub fn query(mut self, field: EditField) -> Self {
        self.query = Some(Box::new(field));
        self
    }

    pub fn clear_on_leave(mut self, on: bool) -> Self {
        self.clear_on_leave = on;
        self
    }

    pub fn toggle_checked(&mut self, key: ItemKey) {
        if let Some(pos) = self.checked.iter().position(|&c| c == key) {
            self.checked.remove(pos);
        } else {
            self.checked.push(key);
        }
    }
    pub fn is_checked(&self, key: ItemKey) -> bool {
        self.checked.contains(&key)
    }
    pub fn reset(&mut self, len: usize) {
        self.cursor.index = 0;
        self.scroll.v = ScrollState::new(len);
        self.typeahead.reset();
        self.hover = None;
        self.pointed = None;
        self.hovering = false;
        self.hidden = false;
    }

    pub fn highlighted(&self) -> Option<usize> {
        (!self.hidden).then_some(self.cursor.index)
    }

    pub fn row_part(i: usize) -> PartRef {
        PartRef::item(Part::ROW, ItemKey::index(i))
    }

    pub fn row_id(&self, i: usize) -> WidgetId {
        self.id.part(Self::row_part(i))
    }

    pub fn row_of(&self, id: WidgetId, len: usize) -> Option<usize> {
        self.id.item_of(id, Part::ROW, 0..len)
    }

    pub fn row_of_event(&self, ev: &PointerEvent) -> Option<usize> {
        if !ev.on(self.id) {
            return None;
        }
        ev.index_of(Part::ROW)
            .filter(|&i| i < self.scroll.content_len)
    }

    pub fn on_backdrop(&self, ev: &PointerEvent) -> bool {
        ev.id == backdrop_id(self.id)
    }

    pub fn row_at(&self, pos: Position) -> Option<usize> {
        if !self.rows_area.contains(pos) {
            return None;
        }
        let line = self.scroll.offset + (pos.y - self.rows_area.y) as usize;
        let i = self.row_on_line(line)?;
        (line < self.scroll.content_len).then_some(i)
    }

    pub fn rows_area(&self) -> Rect {
        self.rows_area
    }

    pub fn key<S: RowSource + ?Sized>(&mut self, key: &Key, src: &S) -> Moved {
        match KeyStep::of(key, &self.nav, self.type_ahead) {
            Some(KeyStep::Nav(nav)) => self.nav_step(nav, src),
            Some(KeyStep::Type(c)) => {
                let moved = self.cursor.type_ahead(c, &mut self.typeahead, src);
                self.keyed(moved)
            }
            None => Moved::Ignored,
        }
    }

    pub fn type_char<S: RowSource + ?Sized>(&mut self, c: char, src: &S) -> Moved {
        let moved = self.cursor.type_ahead(c, &mut self.typeahead, src);
        self.keyed(moved)
    }

    pub fn nav_step<S: RowSource + ?Sized>(&mut self, nav: Nav, src: &S) -> Moved {
        let page = self.scroll.viewport_len;
        let moved = self.cursor.apply(nav, src, page);
        self.keyed(moved)
    }

    pub fn step<S: RowSource + ?Sized>(&mut self, delta: isize, src: &S) -> Moved {
        let moved = self.cursor.step(delta, src);
        self.keyed(moved)
    }

    pub fn select<S: RowSource + ?Sized>(&mut self, i: usize, src: &S) -> Moved {
        let moved = self.cursor.select(i, src);
        if let Some(i) = moved.index() {
            self.hidden = false;
            self.scroll.pointer_move();
            self.scroll.follow(self.line_of(i));
        }
        moved
    }

    fn keyed(&mut self, moved: Moved) -> Moved {
        if let Some(i) = moved.index() {
            self.hidden = false;
            self.scroll.follow(self.line_of(i));

            self.hovering = false;
            self.pointed = None;
        }
        moved
    }

    pub fn layout<S: RowSource + ?Sized>(&mut self, inner: Rect, src: &S, ctx: &RenderCtx) {
        let len = src.len();
        self.scroll.set_content(self.line_count(len));
        self.scroll.set_viewport(inner.height as usize);
        self.cursor.index = self.cursor.index.min(len.saturating_sub(1));
        let with_bar = self.scroll.overflows() && inner.width >= 2;
        self.rows_area = Rect::new(
            inner.x,
            inner.y,
            inner
                .width
                .saturating_sub(u16::from(self.scroll.overflows())),
            inner.height,
        );
        self.hover(ctx, src);
        self.scroll.follow(self.line_of(self.cursor.index));
        if with_bar {
            let track = Rect::new(inner.right() - 1, inner.y, 1, inner.height);
            self.scroll.set_track(self.id, Axis::Vertical, track);
        } else {
            self.scroll.clear_track(Axis::Vertical);
        }
    }

    fn hover<S: RowSource + ?Sized>(&mut self, ctx: &RenderCtx, src: &S) {
        let hover = ctx.interaction.live_hover();
        if hover == self.hover {
            return;
        }
        self.hover = hover;
        let row = hover.and_then(|h| self.row_of(h, src.len()));
        self.pointed = row;
        if !self.hover_moves_cursor {
            return;
        }
        match row {
            Some(i) if src.enabled(i) => {
                self.cursor.index = i;
                self.hovering = true;
                self.hidden = false;

                self.scroll.pointer_move();
                self.scroll.follow(self.line_of(i));
            }

            _ if self.hovering => {
                self.hovering = false;
                self.hidden = self.clear_on_leave;
            }
            _ => {}
        }
    }

    pub fn paint_rows<S: RowSource + ?Sized>(
        &self,
        inner: Rect,
        bg: Color,
        src: &S,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        mut content: impl FnMut(usize, &RowPaint, &mut Buffer, &mut RenderCtx),
    ) {
        let has_sb = self.scroll.overflows();
        let highlighted = self.highlighted();
        let len = src.len();
        for (k, i) in self.visible_lines() {
            if k >= inner.height {
                break;
            }

            let Some(i) = i else { continue };
            if i >= len {
                break;
            }
            crate::record_work!(VisibleRowsPainted, 1usize);
            let y = inner.y + k;
            let mut state = ctx.state(self.row_id(i));
            state.focused = highlighted == Some(i);
            state.disabled = !src.enabled(i);
            state.selected = self.chosen == Some(i) || self.checked.contains(&src.key(i));

            state.hovered &= !self.hover_moves_cursor;
            let area = Rect::new(inner.x, y, inner.width.saturating_sub(u16::from(has_sb)), 1);
            let row = RowUi::paint(ctx, buf, area, state, RowLook::Fill(self.look), bg);
            content(i, &row, buf, ctx);
            ctx.register_part(self.id, Self::row_part(i), area);
        }
        if has_sb {
            if self.fade_edges {
                scrollbar::render_with_fade(inner, buf, ctx, self.id, &self.scroll, true);
            } else if inner.width >= 2 {
                let track = Rect::new(inner.right() - 1, inner.y, 1, inner.height);
                scrollbar::render_vertical(track, buf, ctx, self.id, &self.scroll, false);
            }
        }
    }

    pub fn status<'a, T: AsItem>(&self, items: &'a [T]) -> Option<&'a str> {
        let reason = |i: usize| {
            let it = items.get(i)?.as_item();
            if it.disabled {
                it.reason.as_deref()
            } else {
                None
            }
        };

        self.pointed
            .and_then(reason)
            .or_else(|| self.highlighted().and_then(reason))
    }
}

pub fn query_pointer(query: &mut EditField, ev: &PointerEvent) -> Option<Response<()>> {
    use crate::core::intent::Phase;
    if ev.part().part != Part::QUERY {
        return None;
    }
    let at = query.line_offset_at(ev.pos());
    match ev.phase() {
        Some(Phase::Press) => {
            query.press(at, ev.count(), false);
        }
        Some(Phase::Drag) => {
            query.drag(at);
        }
        Some(Phase::Click) => query.click(at, ev.count().max(1), false),
        _ => return Some(Response::ignored()),
    }
    Some(Response::changed())
}

impl Scrollable for OptionList {
    fn pane(&mut self) -> &mut ScrollPane {
        &mut self.scroll
    }

    fn wheel_first(&mut self, axis: Axis, delta: i32) -> Option<Response<()>> {
        if axis == Axis::Horizontal {
            return Some(Response::consumed());
        }
        if self.wheel == WheelMode::Viewport {
            return None;
        }
        let rows = if self.lines.is_empty() {
            self.scroll.content_len
        } else {
            self.lines.len() - 1
        };
        let last = rows.saturating_sub(1);
        let from = self.cursor.index.min(last);
        let to = if delta < 0 {
            from.saturating_sub(delta.unsigned_abs() as usize)
        } else {
            (from + delta as usize).min(last)
        };
        if to == from {
            return Some(Response::consumed());
        }
        self.cursor.index = to;
        self.hidden = false;
        self.scroll.follow(self.line_of(to));
        Some(Response::changed())
    }
}

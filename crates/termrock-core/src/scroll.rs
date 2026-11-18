use std::cell::Cell;

use ratatui::layout::{Position, Rect};

use crate::core::hit::{Axis, Bar};
use crate::core::id::WidgetId;
use crate::core::response::Response;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollState {
    pub offset: usize,
    pub content_len: usize,
    pub viewport_len: usize,

    pub(crate) grab: Option<usize>,

    pub(crate) paging: bool,

    pub follow_cursor: FollowCursor,

    pub(crate) followed: Option<(usize, usize)>,

    pub context: usize,

    pub(crate) exact: bool,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new(0)
    }
}

impl ScrollState {
    pub fn new(content_len: usize) -> Self {
        Self {
            offset: 0,
            content_len,
            viewport_len: 0,
            grab: None,
            paging: false,
            follow_cursor: FollowCursor::OnMove,
            followed: None,
            context: 1,
            exact: false,
        }
    }

    pub fn with_context(mut self, rows: usize) -> Self {
        self.context = rows;
        self
    }

    pub fn with_follow_cursor(mut self, follow: FollowCursor) -> Self {
        self.follow_cursor = follow;
        self
    }

    pub fn max_offset(&self) -> usize {
        self.content_len.saturating_sub(self.viewport_len)
    }

    pub fn overflows(&self) -> bool {
        self.content_len > self.viewport_len && self.viewport_len > 0
    }

    pub fn at_end(&self) -> bool {
        self.offset >= self.max_offset()
    }

    pub fn set_viewport(&mut self, len: usize) {
        self.viewport_len = len;
        self.clamp();
    }

    pub fn set_content(&mut self, len: usize) {
        self.content_len = len;
        self.clamp();
    }

    pub fn clamp(&mut self) {
        self.offset = self.offset.min(self.max_offset());
    }

    pub fn scroll_by(&mut self, delta: isize) -> bool {
        let before = self.offset;
        self.offset = self
            .offset
            .saturating_add_signed(delta)
            .min(self.max_offset());
        self.offset != before
    }

    pub fn scroll_to(&mut self, offset: usize) -> bool {
        let before = self.offset;
        self.offset = offset.min(self.max_offset());
        self.offset != before
    }

    pub fn page_up(&mut self) -> bool {
        self.scroll_by(-(self.viewport_len.max(1) as isize))
    }

    pub fn page_down(&mut self) -> bool {
        self.scroll_by(self.viewport_len.max(1) as isize)
    }

    pub fn jump_start(&mut self) -> bool {
        self.scroll_to(0)
    }

    pub fn jump_end(&mut self) -> bool {
        self.scroll_to(self.max_offset())
    }

    pub fn pointer_move(&mut self) {
        self.exact = true;
    }

    pub fn ensure_visible(&mut self, index: usize) -> bool {
        let exact = std::mem::take(&mut self.exact);
        if self.viewport_len == 0 {
            return false;
        }
        let before = self.offset;

        let ctx = if !exact && self.viewport_len > 2 * self.context {
            self.context
        } else {
            0
        };
        if index < self.offset + ctx {
            self.offset = index.saturating_sub(ctx);
        } else if index + ctx >= self.offset + self.viewport_len {
            self.offset = index + ctx + 1 - self.viewport_len;
        }
        self.clamp();
        self.offset != before
    }

    pub fn reveal_span(&mut self, start: usize, len: usize) -> bool {
        if self.viewport_len == 0 {
            return false;
        }
        let len = len.max(1);
        let before = self.offset;
        if len >= self.viewport_len {
            self.offset = start;
        } else {
            let ctx = if self.viewport_len >= len + 2 * self.context {
                self.context
            } else {
                0
            };
            if start < self.offset + ctx {
                self.offset = start.saturating_sub(ctx);
            } else if start + len + ctx > self.offset + self.viewport_len {
                self.offset = start + len + ctx - self.viewport_len;
            }
        }
        self.clamp();
        self.offset != before
    }

    pub fn follow(&mut self, cursor: usize) -> bool {
        let key = (cursor, self.viewport_len);
        if self.follow_cursor == FollowCursor::OnMove && self.followed == Some(key) {
            return false;
        }
        self.followed = Some(key);
        self.ensure_visible(cursor)
    }

    pub fn visible_range(&self) -> std::ops::Range<usize> {
        let end = (self.offset + self.viewport_len).min(self.content_len);
        self.offset..end
    }

    pub fn thumb(&self, track_len: usize) -> (usize, usize) {
        if !self.overflows() || track_len == 0 {
            return (0, track_len);
        }
        let len = ((self.viewport_len * track_len) / self.content_len).max(1);
        let len = len.min(track_len);
        let max_off = self.max_offset();
        let start = ((self.offset * (track_len - len)) + max_off / 2)
            .checked_div(max_off)
            .unwrap_or(0);
        (start.min(track_len - len), len)
    }

    pub fn offset_for_thumb_start(&self, start: usize, track_len: usize) -> usize {
        if !self.overflows() || track_len == 0 {
            return 0;
        }
        let (_, len) = self.thumb(track_len);
        let usable = track_len.saturating_sub(len).max(1);
        let start = start.min(usable);
        (start * self.max_offset() + usable / 2) / usable
    }

    pub fn offset_for_track_pos(&self, pos: usize, track_len: usize) -> usize {
        if !self.overflows() || track_len == 0 {
            return 0;
        }
        let (_, len) = self.thumb(track_len);
        self.offset_for_thumb_start(pos.saturating_sub(len / 2), track_len)
    }

    pub fn press_track(&mut self, pos: usize, track_len: usize) -> bool {
        self.paging = false;
        if !self.overflows() || track_len == 0 {
            self.grab = None;
            return false;
        }
        let (start, len) = self.thumb(track_len);
        if (start..start + len).contains(&pos) {
            self.grab = Some(pos - start);
            return false;
        }
        self.grab = Some(len / 2);
        self.scroll_to(self.offset_for_track_pos(pos, track_len))
    }

    pub fn drag_track(&mut self, pos: usize, track_len: usize) -> bool {
        if !self.overflows() || track_len == 0 {
            return false;
        }
        let Some(grab) = self.grab else {
            if self.paging {
                return false;
            }
            return self.press_track(pos, track_len);
        };
        let target = self.offset_for_thumb_start(pos.saturating_sub(grab), track_len);
        self.scroll_to(target)
    }

    pub fn release_track(&mut self) {
        self.grab = None;
        self.paging = false;
    }

    pub fn press_track_with(&mut self, pos: usize, track_len: usize, click: TrackClick) -> bool {
        if click == TrackClick::Jump || !self.overflows() || track_len == 0 {
            return self.press_track(pos, track_len);
        }
        let (start, len) = self.thumb(track_len);
        if (start..start + len).contains(&pos) {
            self.grab = Some(pos - start);
            return false;
        }

        self.grab = None;
        self.paging = true;
        if pos < start {
            self.page_up()
        } else {
            self.page_down()
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FollowCursor {
    #[default]
    OnMove,

    Always,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarVisibility {
    #[default]
    Auto,

    Always,

    Never,
}

impl ScrollbarVisibility {
    pub fn shows(self, scroll: &ScrollState) -> bool {
        match self {
            ScrollbarVisibility::Auto => scroll.overflows(),
            ScrollbarVisibility::Always => scroll.viewport_len > 0,
            ScrollbarVisibility::Never => false,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TrackClick {
    Jump,

    #[default]
    Page,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct AppPolicy {
    click: TrackClick,
    hscroll: HScroll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HScroll {
    pub enabled: bool,

    pub bar: ScrollbarVisibility,

    pub step: usize,

    pub track_click: TrackClick,
}

impl Default for HScroll {
    fn default() -> Self {
        Self {
            enabled: true,
            bar: ScrollbarVisibility::Auto,
            step: 4,
            track_click: TrackClick::Page,
        }
    }
}

impl HScroll {
    pub fn off() -> Self {
        Self {
            enabled: false,
            bar: ScrollbarVisibility::Never,
            ..Self::default()
        }
    }

    pub fn bar(mut self, bar: ScrollbarVisibility) -> Self {
        self.bar = bar;
        self
    }

    pub fn step(mut self, step: usize) -> Self {
        self.step = step.max(1);
        self
    }

    pub fn track_click(mut self, click: TrackClick) -> Self {
        self.track_click = click;
        self
    }

    pub fn shows_bar(&self, scroll: &ScrollState) -> bool {
        self.enabled && self.bar.shows(scroll)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ScrollPane {
    pub v: ScrollState,

    pub h: ScrollState,

    pub h_cfg: HScroll,

    pub track_click: Option<TrackClick>,

    pub columns: bool,

    v_track: Rect,

    h_track: Rect,

    owner: Option<WidgetId>,

    held: Option<Axis>,

    col_widths: Vec<u16>,
    col_gap: u16,
    col_avail: u16,

    col_reveal: Option<(usize, usize)>,

    app: Cell<AppPolicy>,
}

impl std::ops::Deref for ScrollPane {
    type Target = ScrollState;
    fn deref(&self) -> &ScrollState {
        &self.v
    }
}

impl std::ops::DerefMut for ScrollPane {
    fn deref_mut(&mut self) -> &mut ScrollState {
        &mut self.v
    }
}

impl From<ScrollState> for ScrollPane {
    fn from(v: ScrollState) -> Self {
        Self {
            v,
            ..Self::default()
        }
    }
}

impl ScrollPane {
    pub fn new(content_len: usize) -> Self {
        ScrollState::new(content_len).into()
    }

    pub fn set_content(&mut self, len: usize) {
        self.v.set_content(len);
    }

    pub fn set_viewport(&mut self, len: usize) {
        self.v.set_viewport(len);
    }

    pub fn scroll_by(&mut self, delta: isize) -> bool {
        self.v.scroll_by(delta)
    }

    pub fn scroll_to(&mut self, offset: usize) -> bool {
        self.v.scroll_to(offset)
    }

    pub fn ensure_visible(&mut self, index: usize) -> bool {
        self.v.ensure_visible(index)
    }

    pub fn follow(&mut self, cursor: usize) -> bool {
        self.v.follow(cursor)
    }

    pub fn horizontal(mut self, policy: HScroll) -> Self {
        self.h_cfg = policy;
        self
    }

    pub fn track_click(mut self, click: TrackClick) -> Self {
        self.track_click = Some(click);
        self
    }

    pub fn columns(mut self, columns: bool) -> Self {
        self.columns = columns;
        self
    }

    pub fn begin(&mut self, app_click: TrackClick) {
        self.note_policy(app_click, self.app_hscroll());
        self.v_track = Rect::ZERO;
        self.h_track = Rect::ZERO;
    }

    pub fn note_policy(&self, click: TrackClick, hscroll: HScroll) {
        self.app.set(AppPolicy { click, hscroll });
    }

    pub fn app_hscroll(&self) -> HScroll {
        self.app.get().hscroll
    }

    pub fn v_click(&self) -> TrackClick {
        self.track_click.unwrap_or(self.app.get().click)
    }

    pub fn v_track(&self) -> Rect {
        self.v_track
    }

    pub fn h_track(&self) -> Rect {
        self.h_track
    }

    pub fn set_track(&mut self, owner: WidgetId, axis: Axis, track: Rect) {
        self.owner = Some(owner);
        match axis {
            Axis::Vertical => self.v_track = track,
            Axis::Horizontal => self.h_track = track,
        }
    }

    pub fn clear_track(&mut self, axis: Axis) {
        match axis {
            Axis::Vertical => self.v_track = Rect::ZERO,
            Axis::Horizontal => self.h_track = Rect::ZERO,
        }
    }

    pub fn held(&self) -> Option<Axis> {
        self.held
    }

    pub fn axis(&mut self, axis: Axis) -> &mut ScrollState {
        match axis {
            Axis::Vertical => &mut self.v,
            Axis::Horizontal => &mut self.h,
        }
    }

    pub fn layout_columns(&mut self, widths: Vec<u16>, gap: u16, avail: u16) -> usize {
        self.col_widths = widths;
        self.col_gap = gap;
        self.col_avail = avail;
        if let Some((index, left)) = self.col_reveal {
            if self.h.offset == left {
                self.place_column(index);
                self.col_reveal = Some((index, self.h.offset));
            } else {
                self.col_reveal = None;
            }
        }
        self.columns_fit(self.h.offset)
    }

    pub fn reveal_column(&mut self, index: usize) -> bool {
        let moved = self.place_column(index);
        self.col_reveal = Some((index, self.h.offset));
        moved
    }

    fn columns_fit(&self, from: usize) -> usize {
        let mut used = 0u16;
        let mut fit = 0;
        for &w in self.col_widths.iter().skip(from) {
            let need = if fit == 0 {
                w
            } else {
                w.saturating_add(self.col_gap)
            };
            if used.saturating_add(need) > self.col_avail {
                break;
            }
            used += need;
            fit += 1;
        }
        fit
    }

    fn place_column(&mut self, index: usize) -> bool {
        let n = self.col_widths.len();
        if n == 0 || self.col_avail == 0 {
            return false;
        }
        let index = index.min(n - 1);
        let before = self.h.offset;
        let mut offset = before.min(n - 1);
        if index < offset {
            offset = index;
        }

        while offset < index && offset + self.columns_fit(offset) <= index {
            offset += 1;
        }

        while offset > 0 && offset - 1 + self.columns_fit(offset - 1) >= n {
            offset -= 1;
        }
        self.h.offset = offset;
        offset != before
    }

    pub fn h_step(&self) -> usize {
        if self.columns {
            1
        } else {
            self.h_cfg.step.max(1)
        }
    }

    pub fn h_scrolls(&self) -> bool {
        self.h_cfg.enabled && self.h.overflows()
    }

    pub fn wheel(&mut self, axis: Axis, delta: i32) -> Response<()> {
        let moved = match axis {
            Axis::Vertical => {
                if !self.v.overflows() {
                    return Response::ignored();
                }

                let page = self.v.viewport_len.max(1) as isize;
                self.v.scroll_by((delta as isize).clamp(-page, page))
            }
            Axis::Horizontal => {
                if !self.h_scrolls() {
                    return Response::ignored();
                }
                self.col_reveal = None;
                let step = self.h_step() as isize;
                self.h.scroll_by(delta.signum() as isize * step)
            }
        };
        changed(moved)
    }

    pub fn press(&mut self, pos: Position) -> Response<()> {
        let axis = if self.h_track.contains(pos) {
            Axis::Horizontal
        } else if self.v_track.contains(pos) {
            Axis::Vertical
        } else {
            self.held = None;
            return Response::ignored();
        };
        self.press_on(axis, pos)
    }

    pub fn press_on(&mut self, axis: Axis, pos: Position) -> Response<()> {
        let (track, click) = match axis {
            Axis::Vertical => (self.v_track, self.v_click()),
            Axis::Horizontal => (self.h_track, self.h_cfg.track_click),
        };
        if track.is_empty() {
            self.held = None;
            return Response::ignored();
        }
        self.held = Some(axis);
        if axis == Axis::Horizontal {
            self.col_reveal = None;
        }
        let (cell, len) = track_cell(track, axis, pos);
        changed(self.axis(axis).press_track_with(cell, len, click))
    }

    pub fn drag(&mut self, pos: Position) -> Response<()> {
        let axis = self.held.unwrap_or(Axis::Vertical);
        self.drag_on(axis, pos)
    }

    pub fn drag_on(&mut self, axis: Axis, pos: Position) -> Response<()> {
        let track = match axis {
            Axis::Vertical => self.v_track,
            Axis::Horizontal => self.h_track,
        };
        if track.is_empty() {
            return Response::consumed();
        }
        self.held = Some(axis);
        if axis == Axis::Horizontal {
            self.col_reveal = None;
        }
        let (cell, len) = track_cell(track, axis, pos);
        changed(self.axis(axis).drag_track(cell, len))
    }

    pub fn release(&mut self) {
        self.v.release_track();
        self.h.release_track();
        self.held = None;
    }

    pub fn axis_of(&self, bar_id: WidgetId) -> Option<Axis> {
        let owner = self.owner?;
        if bar_id == owner.sub("scrollbar") {
            Some(Axis::Vertical)
        } else if bar_id == owner.sub("hscrollbar") {
            Some(Axis::Horizontal)
        } else {
            None
        }
    }

    pub fn bar_of(&self, bar_id: WidgetId) -> Option<Bar> {
        let axis = self.axis_of(bar_id)?;
        let track = match axis {
            Axis::Vertical => self.v_track,
            Axis::Horizontal => self.h_track,
        };
        Some(Bar {
            id: bar_id,
            owner: self.owner?,
            axis,
            track,
        })
    }

    pub fn bar(&mut self, bar: &Bar, pos: Position, pressed: bool) -> Response<()> {
        self.set_track(bar.owner, bar.axis, bar.track);
        if pressed {
            self.press_on(bar.axis, pos)
        } else {
            self.drag_on(bar.axis, pos)
        }
    }
}

fn track_cell(track: Rect, axis: Axis, pos: Position) -> (usize, usize) {
    match axis {
        Axis::Vertical => (
            pos.y
                .saturating_sub(track.y)
                .min(track.height.saturating_sub(1)) as usize,
            track.height as usize,
        ),
        Axis::Horizontal => (
            pos.x
                .saturating_sub(track.x)
                .min(track.width.saturating_sub(1)) as usize,
            track.width as usize,
        ),
    }
}

fn changed(moved: bool) -> Response<()> {
    if moved {
        Response::changed()
    } else {
        Response::consumed()
    }
}

pub trait Scrollable {
    fn pane(&mut self) -> &mut ScrollPane;

    fn after_scroll(&mut self, _axis: Axis, _moved: bool) -> bool {
        false
    }

    fn wheel_first(&mut self, _axis: Axis, _delta: i32) -> Option<Response<()>> {
        None
    }

    fn wheel(&mut self, axis: Axis, delta: i32) -> Response<()> {
        if let Some(r) = self.wheel_first(axis, delta) {
            return r;
        }
        let r = self.pane().wheel(axis, delta);
        settle(self, axis, r)
    }

    fn bar_press(&mut self, pos: Position) -> Response<()> {
        let r = self.pane().press(pos);
        match self.pane().held() {
            Some(axis) => settle(self, axis, r),
            None => r,
        }
    }

    fn bar_drag(&mut self, pos: Position) -> Response<()> {
        let axis = self.pane().held().unwrap_or(Axis::Vertical);
        let r = self.pane().drag_on(axis, pos);
        settle(self, axis, r)
    }

    fn bar_input(&mut self, bar: &Bar, pos: Position, pressed: bool) -> Response<()> {
        let r = self.pane().bar(bar, pos, pressed);
        settle(self, bar.axis, r)
    }
}

fn settle<S: Scrollable + ?Sized>(w: &mut S, axis: Axis, r: Response<()>) -> Response<()> {
    if w.after_scroll(axis, r.is_changed()) {
        r.repaint()
    } else {
        r
    }
}

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

use crate::list_core::ListCore;
use termrock_controls::scrollbar;
use termrock_core::event::{Key, Outcome};
use termrock_core::hit::Axis;
use termrock_core::id::{ItemKey, Part, PartRef, WidgetId};
use termrock_core::keymap::{Actions, KeyBinding, KeyMap, Nav, NavKeys, lookup};
use termrock_core::list::{Cursor, Navigable, RowSource};
use termrock_core::response::Response;
use termrock_core::scroll::{ScrollPane, Scrollable};
use termrock_core::selection::ClickPolicy;
use termrock_runtime::ctx::RenderCtx;
use termrock_runtime::measure::terminal_extent;
use termrock_runtime::reveal::{Reveal, RevealLine};
use termrock_runtime::row::{RowLook, RowUi};
use termrock_theme::{Theme, Tone};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prop {
    pub label: String,
    pub value: String,
    pub tone: Tone,
    pub wrap: bool,

    pub copyable: bool,
}

impl Prop {
    pub fn new(label: &str, value: impl Into<String>) -> Self {
        Self {
            label: label.to_owned(),
            value: value.into(),
            tone: Tone::Normal,
            wrap: false,
            copyable: false,
        }
    }
    pub fn copyable(mut self) -> Self {
        self.copyable = true;
        self
    }
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
    /// Marks a value that may take several rows. `render`, `measure` and
    /// `InfoDialog` wrap it. A `PropsList` keeps every prop on one row and shows
    /// the rest of a long value through its `reveal`.
    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sheet {
    pub label_max_share: u16,

    pub value_min: u16,
}

impl Default for Sheet {
    fn default() -> Self {
        Self {
            label_max_share: 40,
            value_min: 12,
        }
    }
}

const STACK_INDENT: u16 = 2;

struct Row<'a> {
    prop: &'a Prop,
    label: Option<&'a str>,
    value: String,
    value_x: u16,
}

fn column(widths: impl Iterator<Item = usize> + Clone, width: u16, sheet: Sheet) -> Option<u16> {
    let avail = usize::from(width);
    let value_min = usize::from(sheet.value_min);
    let full = widths.clone().max().unwrap_or(2);
    if avail.saturating_sub(full) >= value_min {
        return Some(terminal_extent(full));
    }
    let cap = avail * usize::from(sheet.label_max_share.min(100)) / 100;

    let label_w = widths.filter(|w| *w <= cap).max().unwrap_or(cap.max(1));
    (avail.saturating_sub(label_w) >= value_min).then_some(terminal_extent(label_w))
}

fn label_column(props: &[Prop], width: u16, sheet: Sheet) -> Option<u16> {
    column(
        props
            .iter()
            .map(|p| termrock_text::cells::width(&p.label).saturating_add(2)),
        width,
        sheet,
    )
}

fn value_rows<'a>(p: &'a Prop, label: Option<&'a str>, x: u16, vw: usize, out: &mut Vec<Row<'a>>) {
    if p.wrap {
        let lines = termrock_text::cells::wrap(&p.value, vw.max(4));
        for (i, line) in lines.into_iter().enumerate() {
            out.push(Row {
                prop: p,
                label: if i == 0 { label } else { None },
                value: line,
                value_x: x,
            });
        }
    } else {
        out.push(Row {
            prop: p,
            label,
            value: termrock_text::cells::truncate(&p.value, vw),
            value_x: x,
        });
    }
}

fn rows(props: &[Prop], width: u16, sheet: Sheet) -> Vec<Row<'_>> {
    let column = label_column(props, width, sheet);
    let mut out = vec![];
    for p in props {
        match column {
            Some(label_w)
                if termrock_text::cells::width(&p.label).saturating_add(2)
                    <= usize::from(label_w) =>
            {
                let vw = width.saturating_sub(label_w) as usize;
                value_rows(p, Some(&p.label), label_w, vw, &mut out);
            }

            _ => {
                out.push(Row {
                    prop: p,
                    label: Some(&p.label),
                    value: String::new(),
                    value_x: 0,
                });
                let x = column.unwrap_or(STACK_INDENT.min(width.saturating_sub(1)));
                let vw = width.saturating_sub(x) as usize;
                value_rows(p, None, x, vw, &mut out);
            }
        }
    }
    out
}

pub fn measure(props: &[Prop], width: u16) -> u16 {
    measure_with(props, width, Sheet::default())
}

pub fn measure_with(props: &[Prop], width: u16, sheet: Sheet) -> u16 {
    terminal_extent(rows(props, width, sheet).len())
}

pub fn width(props: &[Prop]) -> u16 {
    width_with(props, Sheet::default())
}

pub fn width_with(props: &[Prop], sheet: Sheet) -> u16 {
    if props.is_empty() {
        return 0;
    }
    let w = termrock_text::cells::width;
    let label = props
        .iter()
        .map(|p| w(&p.label).saturating_add(2))
        .max()
        .unwrap_or(2);
    let value = props
        .iter()
        .map(|p| {
            if p.wrap {
                p.value.split_whitespace().map(w).max().unwrap_or(0)
            } else {
                w(&p.value)
            }
        })
        .max()
        .unwrap_or(0);
    terminal_extent(label.saturating_add(value.max(usize::from(sheet.value_min))))
}

pub fn render(area: Rect, buf: &mut Buffer, t: &Theme, props: &[Prop], bg: Color) -> u16 {
    render_with(area, buf, t, props, bg, Sheet::default())
}

pub fn render_with(
    area: Rect,
    buf: &mut Buffer,
    t: &Theme,
    props: &[Prop],
    bg: Color,
    sheet: Sheet,
) -> u16 {
    let area = area.intersection(*buf.area());
    if area.is_empty() {
        return 0;
    }
    let mut y = area.y;
    for row in rows(props, area.width, sheet) {
        if y >= area.bottom() {
            break;
        }
        if let Some(label) = row.label {
            let shown = termrock_text::cells::truncate(label, area.width as usize);
            buf.set_string(area.x, y, &shown, t.muted().bg(bg));
            let cut = Rect::new(area.x, y, termrock_text::cells::width(&shown) as u16, 1);
            termrock_runtime::reveal::queue_cut(cut, label);
        }
        let style = ratatui::style::Style::new()
            .fg(t.tone(row.prop.tone))
            .bg(bg);
        if !row.value.is_empty() {
            buf.set_string(area.x + row.value_x, y, &row.value, style);
            if !row.prop.wrap {
                let cut = Rect::new(
                    area.x + row.value_x,
                    y,
                    termrock_text::cells::width(&row.value) as u16,
                    1,
                );
                termrock_runtime::reveal::queue_cut(cut, &row.prop.value);
            }
        }
        y += 1;
    }
    y - area.y
}

fn natural(props: &[Prop], avail: ratatui::layout::Size) -> ratatui::layout::Size {
    ratatui::layout::Size::new(avail.width, measure(props, avail.width))
}

pub fn render_in(
    view: &mut termrock_viewport::scroll_view::ScrollView,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    props: &[Prop],
    bg: Color,
) -> u16 {
    let area = area.intersection(*buf.area());
    if area.is_empty() || props.is_empty() {
        return 0;
    }
    let size = view.fit(area.as_size(), |a| natural(props, a));
    let area = Rect::new(area.x, area.y, area.width, size.height.min(area.height));
    let t = ctx.theme;
    view.render(area, size, buf, ctx, |r, b, _| {
        render(r, b, t, props, bg);
    });
    area.height
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropsAction {
    Copy(usize),
    Activate(usize),
}

#[derive(Debug, Clone, Copy)]
struct Geometry {
    row_w: u16,
    label_w: u16,
    value_right: u16,
}

#[derive(Debug, Clone)]
pub struct PropsList {
    pub id: WidgetId,
    pub props: Vec<Prop>,

    pub core: ListCore,
    pub area: Rect,

    pub scrollbar_gap: u16,

    pub sheet: Sheet,

    pub keymap: KeyMap<PropsCmd>,

    /// Whether a prop marked `Prop::wrap` shows on as many rows as its value
    /// needs. Off, the value stays on its row and the reveal shows the rest.
    wrapping: bool,

    reveal_cursor: bool,
}

impl std::ops::Deref for PropsList {
    type Target = ListCore;
    fn deref(&self) -> &ListCore {
        &self.core
    }
}

impl std::ops::DerefMut for PropsList {
    fn deref_mut(&mut self) -> &mut ListCore {
        &mut self.core
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropsCmd {
    Copy,
}

impl Actions for PropsCmd {
    fn label(self) -> &'static str {
        match self {
            PropsCmd::Copy => "Copy",
        }
    }

    fn default_keymap() -> KeyMap<Self> {
        vec![(KeyBinding::char('y'), PropsCmd::Copy)]
    }
}

impl PropsList {
    pub fn keymap(mut self, keymap: KeyMap<PropsCmd>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.core.nav = nav;
        self
    }

    pub fn content(mut self, content: crate::content::ContentState) -> Self {
        self.core.content = content;
        self
    }

    pub fn empty(mut self, state: termrock_feedback::empty::EmptyState) -> Self {
        self.core.empty = Some(state);
        self
    }

    pub fn skip_when_empty(mut self, on: bool) -> Self {
        self.core.skip_when_empty = on;
        self
    }

    pub fn new(id: WidgetId, props: Vec<Prop>) -> Self {
        let n = props.len();
        Self {
            id,
            props,
            core: ListCore {
                type_ahead: false,
                ..ListCore::new(n)
            },
            area: Rect::ZERO,
            scrollbar_gap: scrollbar::GAP,
            sheet: Sheet::default(),
            keymap: PropsCmd::default_keymap(),
            wrapping: false,
            reveal_cursor: false,
        }
    }

    /// `InfoDialog`'s list wraps its `Prop::wrap` values on rows of their own.
    /// Library plumbing, not API: it is `#[doc(hidden)] pub` because `InfoDialog`
    /// in `junie-tui` calls it across the crate boundary (docs/decisions.md,
    /// #494). An app shows a long value through `reveal`, or draws it with
    /// `props::render`.
    #[doc(hidden)]
    pub fn wrapping(mut self) -> Self {
        self.wrapping = true;
        self
    }

    pub fn track_click(mut self, click: termrock_core::scroll::TrackClick) -> Self {
        self.core.scroll.track_click = Some(click);
        self
    }

    pub fn scrollbar_gap(mut self, gap: u16) -> Self {
        self.scrollbar_gap = gap;
        self
    }

    pub fn sheet(mut self, sheet: Sheet) -> Self {
        self.sheet = sheet;
        self
    }

    pub fn reveal(mut self, config: Reveal) -> Self {
        self.core.reveal.config = Some(config);
        self
    }

    pub fn set_props(&mut self, props: Vec<Prop>) {
        self.props = props;
        self.core.cursor = self.core.cursor.min(self.props.len().saturating_sub(1));
        self.core.scroll.set_content(self.props.len());
    }

    pub fn row_id(&self, i: usize) -> WidgetId {
        self.id.part(Self::row_part(i))
    }

    fn row_part(i: usize) -> PartRef {
        PartRef::item(Part::ROW, ItemKey::index(i))
    }

    #[doc(hidden)]
    pub fn row_of(&self, id: WidgetId) -> Option<usize> {
        self.id.item_of(id, Part::ROW, 0..self.props.len())
    }

    fn set_cursor(&mut self, i: usize) {
        self.core.cursor = i.min(self.props.len().saturating_sub(1));
        self.core.scroll.ensure_visible(self.core.cursor);
        self.reveal_cursor = true;
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<PropsAction>) {
        if self.props.is_empty() {
            return (Outcome::Ignored, None);
        }
        if let Some(o) = self.core.reveal.on_key(key) {
            return (o, None);
        }
        if let Some(PropsCmd::Copy) = lookup(&self.keymap, key) {
            if self.props[self.core.cursor].copyable {
                return (Outcome::Changed, Some(PropsAction::Copy(self.core.cursor)));
            }
            return (Outcome::Consumed, None);
        }
        let nav = self
            .nav
            .action(key)
            .filter(|n| key.plain() || matches!(n, Nav::PageUp | Nav::PageDown | Nav::End));
        if let Some(nav) = nav {
            let moved = self
                .cursor()
                .apply(nav, self, self.core.scroll.viewport_len);
            return match moved.index() {
                Some(i) => {
                    self.set_cursor(i);
                    (Outcome::Changed, None)
                }
                None => (Outcome::Ignored, None),
            };
        }
        match key.code {
            KeyCode::Enter => (
                Outcome::Changed,
                Some(PropsAction::Activate(self.core.cursor)),
            ),
            _ => (Outcome::Ignored, None),
        }
    }

    pub fn on_click(&mut self, row: usize) -> (Outcome, Option<PropsAction>) {
        self.on_click_count(row, 1)
    }

    pub fn on_click_count(&mut self, row: usize, count: u8) -> (Outcome, Option<PropsAction>) {
        if row >= self.props.len() {
            return (Outcome::Consumed, None);
        }
        let again = self.core.cursor == row;
        self.core.scroll.pointer_move();
        self.set_cursor(row);
        let act = self.core.click.activates(count.max(1), again);
        (Outcome::Changed, act.then_some(PropsAction::Activate(row)))
    }

    pub fn click(mut self, policy: ClickPolicy) -> Self {
        self.core.click = policy;
        self
    }

    fn hint_width(p: &Prop, focused: bool) -> u16 {
        if p.copyable && focused { 8 } else { 0 }
    }

    fn geometry(&self, body: Rect, content: Rect) -> Geometry {
        let row_w = body.width;

        let value_right = content.right().min(body.right().saturating_sub(1));

        let label_area = row_w.saturating_sub(2);
        let widths = self
            .props
            .iter()
            .map(|p| termrock_text::cells::width(&p.label).saturating_add(2));
        let label_w = column(widths, label_area, self.sheet).unwrap_or_else(|| {
            (label_area as u32 * self.sheet.label_max_share.min(100) as u32 / 100) as u16
        });
        Geometry {
            row_w,
            label_w: label_w.max(3),
            value_right,
        }
    }

    fn value_width(geo: Geometry, x: u16, p: &Prop) -> usize {
        let vx = x.saturating_add(2).saturating_add(geo.label_w);
        geo.value_right
            .saturating_sub(vx.saturating_add(Self::hint_width(p, p.copyable))) as usize
    }

    /// The lines of a prop that wraps; `None` for one that stays on a row.
    fn wrapped_lines(&self, p: &Prop, geo: Geometry, x: u16) -> Option<Vec<String>> {
        (self.wrapping && p.wrap)
            .then(|| termrock_text::cells::wrap(&p.value, Self::value_width(geo, x, p).max(4)))
    }

    /// The rows the wrapping props take beyond their first.
    fn wrap_extra(&self, geo: Geometry, x: u16) -> usize {
        self.props
            .iter()
            .filter_map(|p| self.wrapped_lines(p, geo, x))
            .map(|lines| lines.len().saturating_sub(1))
            .sum()
    }

    fn rows_of(&self, geo: Geometry, x: u16, i: usize) -> usize {
        self.wrapped_lines(&self.props[i], geo, x)
            .map_or(1, |lines| lines.len().max(1))
    }

    /// How many props from `offset` fit in `height` rows, each at the rows it
    /// takes. Props scrolled out of the window cost nothing.
    fn props_fitting(&self, geo: Geometry, x: u16, offset: usize, height: usize) -> usize {
        let mut rows = 0;
        let mut fitting = 0;
        for i in offset..self.props.len() {
            let h = self.rows_of(geo, x, i);
            if fitting > 0 && rows + h > height {
                break;
            }
            rows += h;
            fitting += 1;
        }
        fitting.max(1)
    }

    fn first_showing(&self, geo: Geometry, x: u16, cursor: usize, height: usize) -> usize {
        let mut rows = 0;
        let mut first = cursor;
        for i in (0..=cursor).rev() {
            rows += self.rows_of(geo, x, i);
            if i != cursor && rows > height {
                break;
            }
            first = i;
        }
        first
    }

    fn fit_viewport(&mut self, geo: Geometry, area: Rect, reserved: usize, reveal: bool) {
        let height = (area.height as usize).saturating_sub(reserved).max(1);
        if reveal && self.core.cursor < self.props.len() {
            let first = self.first_showing(geo, area.x, self.core.cursor, height);
            self.core.scroll.offset = self.core.scroll.offset.max(first);
        }
        let fitting = self.props_fitting(geo, area.x, self.core.scroll.offset, height);
        self.core.scroll.set_viewport(fitting);
    }

    /// The height that shows every prop whole in a list `width` columns wide,
    /// for `InfoDialog` to size its list. Library plumbing, not API: it assumes
    /// no scrollbar, and an app sizes a sheet with `props::measure_with`.
    #[doc(hidden)]
    pub fn height_at(&self, width: u16) -> u16 {
        let area = Rect::new(0, 0, width, 1);
        let geo = self.geometry(area, area);
        terminal_extent(self.props.len().saturating_add(self.wrap_extra(geo, 0)))
    }

    fn inline_rows(
        &mut self,
        ctx: &mut RenderCtx,
        geo: Geometry,
        room: u16,
    ) -> Option<Vec<RevealLine>> {
        let p = self.props.get(self.core.cursor)?;
        if self.wrapping && p.wrap {
            return None;
        }
        let vx = self.area.x + 2 + geo.label_w;
        let vw = geo
            .value_right
            .saturating_sub(vx + Self::hint_width(p, true));
        let rid = self.row_id(self.core.cursor);
        let value = p.value.clone();
        self.core.reveal.inline(ctx, rid, vw, &value, true, room)
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        self.core.scroll.begin(ctx.track_click);
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }
        self.area = area;
        let t = ctx.theme;
        let focused = ctx.interaction.focused(self.id);
        self.core.reveal.begin(ctx);
        self.core.scroll.set_content(self.props.len());
        let offset = self.core.scroll.offset;
        self.core.scroll.set_viewport(area.height as usize);
        let n = self.props.len();

        if self.core.is_focus_stop(n, focused) {
            ctx.control(self.id, area, false);
            ctx.scrollable(self.id, area);
        }
        if (!self.core.content.is_ready() || self.core.empty.is_some())
            && self.core.draw_content(n, area, buf, ctx, bg, None)
        {
            return;
        }
        let reveal = std::mem::take(&mut self.reveal_cursor);
        let mut has_sb = self.core.scroll.overflows();
        let mut sp = scrollbar::split(area, has_sb, self.scrollbar_gap);
        let mut geo = self.geometry(sp.body, sp.content);
        let wraps = self.wrap_extra(geo, area.x) > 0;
        if wraps {
            self.core.scroll.offset = offset;
            self.fit_viewport(geo, area, 0, reveal);
            if self.core.scroll.overflows() != has_sb {
                has_sb = self.core.scroll.overflows();
                sp = scrollbar::split(area, has_sb, self.scrollbar_gap);
                geo = self.geometry(sp.body, sp.content);
                self.fit_viewport(geo, area, 0, reveal);
            }
        }

        let mut expanded = None;
        if focused {
            expanded = self.inline_rows(ctx, geo, area.height);
            if let Some(rows) = &expanded {
                let extra = rows.len().saturating_sub(1);
                if wraps {
                    self.fit_viewport(geo, area, extra, reveal);
                } else {
                    self.core
                        .scroll
                        .set_viewport((area.height as usize).saturating_sub(extra).max(1));
                }
                self.core.scroll.ensure_visible(self.core.cursor);
                if self.core.scroll.overflows() != has_sb {
                    has_sb = self.core.scroll.overflows();
                    sp = scrollbar::split(area, has_sb, self.scrollbar_gap);
                    geo = self.geometry(sp.body, sp.content);
                    expanded = self.inline_rows(ctx, geo, area.height);
                }
            }
        }
        let Geometry {
            row_w,
            label_w,
            value_right,
        } = geo;
        let mut y = area.y;
        let mut cursor_rows = Vec::new();
        for i in self.core.scroll.visible_range() {
            if y >= area.bottom() {
                break;
            }
            let p = &self.props[i];
            let rid = self.row_id(i);
            let mut s = ctx.child_state(self.id, rid, i == self.core.cursor);
            s.focused = focused && i == self.core.cursor;
            let wrapped = self.wrapped_lines(p, geo, area.x).map(|lines| {
                lines
                    .into_iter()
                    .map(|text| RevealLine { text, more: false })
                    .collect::<Vec<_>>()
            });
            let block = wrapped
                .as_ref()
                .or_else(|| expanded.as_ref().filter(|_| i == self.core.cursor));
            let h = block
                .map_or(1, |r| r.len() as u16)
                .clamp(1, area.bottom() - y);
            let row = Rect::new(area.x, y, row_w, h);
            if i == self.core.cursor {
                cursor_rows.extend(row.y..row.bottom());
            }
            let rp = RowUi::paint(ctx, buf, row, s, RowLook::Bar, bg);
            let st = rp.style();
            buf.set_string(
                row.x + 2,
                y,
                termrock_text::cells::truncate(&p.label, label_w.saturating_sub(2) as usize),
                st.fg(t.text_muted).remove_modifier(Modifier::BOLD),
            );
            let vx = row.x + 2 + label_w;
            let hint_w = Self::hint_width(p, s.focused);
            let vw = value_right.saturating_sub(vx + hint_w) as usize;
            let vs = st.fg(t.tone(p.tone));
            if let Some(lines) = block {
                for (k, line) in lines.iter().take(h as usize).enumerate() {
                    let ls = if line.more { st.fg(t.text_muted) } else { vs };
                    let text = termrock_text::cells::truncate(&line.text, vw);
                    buf.set_string(vx, y + k as u16, text, ls);
                }
            } else {
                let cell = Rect::new(vx, y, vw as u16, 1);
                rp.label(ctx, buf, &mut self.core.reveal, rid, cell, &p.value, vs);
            }
            if hint_w > 0 {
                buf.set_string(
                    row.right().saturating_sub(7),
                    y,
                    "y copy",
                    st.fg(t.text_faint).remove_modifier(Modifier::BOLD),
                );
            }
            ctx.register_part(self.id, Self::row_part(i), row);
            y += h;
        }
        if let Some(sb) = sp.track {
            termrock_runtime::fade::scroll_edges_except(
                buf,
                ctx,
                sp.body,
                &self.core.scroll,
                &cursor_rows,
            );
            self.core.scroll.draw_v(sb, buf, ctx, self.id, focused);
        }
    }
}

impl Scrollable for PropsList {
    fn pane(&mut self) -> &mut ScrollPane {
        &mut self.core.scroll
    }

    fn wheel_first(&mut self, axis: Axis, delta: i32) -> Option<Response<()>> {
        if axis != Axis::Vertical {
            return None;
        }
        self.core.reveal.on_wheel(delta).map(Response::from)
    }
}

impl RowSource for PropsList {
    fn len(&self) -> usize {
        self.props.len()
    }
    fn label(&self, i: usize) -> &str {
        &self.props[i].label
    }
}

impl Navigable for PropsList {
    fn cursor(&self) -> Cursor {
        self.core.cursor_model()
    }
}

impl termrock_runtime::specimen::Specimen for PropsList {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: termrock_runtime::specimen::SpecimenState) -> bool {
        use termrock_runtime::specimen::SpecimenState::*;
        matches!(state, Rest | Focused | Loading | Error | Overflow | Narrow)
    }

    fn specimen_height(&self, state: termrock_runtime::specimen::SpecimenState) -> u16 {
        use termrock_runtime::specimen::SpecimenState::*;
        if matches!(state, Loading | Error) {
            return 1;
        }
        self.props.len().max(1) as u16
    }

    fn specimen(
        &self,
        state: termrock_runtime::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        let mut list = self.clone();
        if state == termrock_runtime::specimen::SpecimenState::Overflow
            && let Some(p) = list.props.first_mut()
        {
            p.value = format!("{} and a value far longer than its row", p.value);
        }
        list.core.content = crate::content::specimen(state);
        list.render(area, buf, ctx, bg);
    }
}

impl termrock_core::pointer::MouseTarget for PropsList {
    type Action = PropsAction;

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &termrock_core::pointer::PointerEvent,
    ) -> termrock_core::response::Response<PropsAction> {
        use termrock_core::intent::Phase;
        if let Some(r) = termrock_core::pointer::scroll_input(self, ev) {
            return r.lift();
        }
        match ev.index_of(Part::ROW) {
            Some(row) if ev.is(Phase::Click) => self.on_click_count(row, ev.count()).into(),
            _ => termrock_core::response::Response::ignored(),
        }
    }
}

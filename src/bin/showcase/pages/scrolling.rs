use std::collections::BTreeSet;
use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::hit::Axis;
use junie_tui::core::id::{Part, PartRef, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::NavKeys;
use junie_tui::core::list::{
    Cursor, KeyStep, Moved, NavProfile, Navigable, RowFn, RowSource, TypeAheadPrecedence,
};
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::scroll::{
    FollowCursor, HScroll, ScrollPane, ScrollState, ScrollbarVisibility, TrackClick,
};
use junie_tui::theme::{CursorStyle, Theme};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::StackChild;
use junie_tui::ui::scroll_view::{Axes, ScrollView};
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::ui::text::{fit, slice_cells, truncate};
use junie_tui::widgets::grid::{CellKind, CellValue, ColumnSpec, DataGrid, GridRows, RowTotal};
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::panel::{Panel, ScrollPanel};
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::select::Select;
use junie_tui::widgets::table::{Cell, Column, DataTable};
use junie_tui::widgets::tree::{TreeNode, TreeView};

const ID: WidgetId = WidgetId::of("scrolling");

const HAND: WidgetId = ID.sub("hand");

const FITS: WidgetId = ID.sub("fits");
const OVER: WidgetId = ID.sub("over");
const NEST_OUTER: WidgetId = ID.sub("nest_outer");
const NEST_INNER: WidgetId = ID.sub("nest_inner");
const VIRTUAL: WidgetId = ID.sub("virtual");

pub struct VirtualRows(Vec<String>);

impl VirtualRows {
    fn new(len: usize) -> Self {
        Self((1..=len).map(|i| format!("Row {i:05}")).collect())
    }
}

impl RowSource for VirtualRows {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn label(&self, i: usize) -> &str {
        &self.0[i]
    }
}

fn retired(i: usize) -> bool {
    i % 10 == 9
}

const SERVICES: &[&str] = &[
    "api", "auth", "billing", "cache", "db", "edge", "feeds", "gateway", "jobs", "logs",
];
const REGIONS: &[&str] = &[
    "ap-south-1",
    "ap-southeast-2",
    "ca-central-1",
    "eu-central-1",
    "eu-north-1",
    "eu-west-1",
    "eu-west-2",
    "me-south-1",
    "sa-east-1",
    "us-east-1",
    "us-east-2",
    "us-west-2",
];

const HAND_COLS: &[&str] = &["Host", "Region", "Latency", "CPU", "Memory", "Uptime"];
const HAND_W: usize = 14;

fn hand_row(i: usize) -> [String; 6] {
    [
        format!(
            "{}-{:02}",
            SERVICES[i % SERVICES.len()],
            i / SERVICES.len() + 1
        ),
        REGIONS[i % REGIONS.len()].to_owned(),
        format!("{} ms", 8 + (i * 37) % 90),
        format!("{}%", (i * 53) % 100),
        format!("{:.1} GiB", 1.0 + ((i * 29) % 60) as f32 / 10.0),
        format!("{} d", 1 + (i * 17) % 300),
    ]
}

pub struct ScrollingPage {
    template: Template,
    opts: PageValues,
    sections: crate::pages::layout::Sections,
    pub prose: ScrollPanel,
    pub list: ListBox,
    pub log: ScrollPanel,

    pub hand: ScrollPane,

    pub fits: ScrollPane,
    pub over: ScrollPane,

    pub nest_outer: ScrollView,
    pub nest_inner: ScrollView,

    pub virtual_rows: ScrollView,
    pub virtual_source: VirtualRows,
    pub virtual_cursor: usize,
    pub virtual_painted: usize,

    pub region: Select,
    pub tree: TreeView,
    pub table: DataTable,
    pub grid: DataGrid,

    pub flagged: BTreeSet<usize>,

    pub last: Option<(KeyStep, Moved)>,
}

fn prose_style(t: &Theme, _l: &str) -> ratatui::style::Style {
    t.secondary()
}

impl ScrollingPage {
    pub fn new() -> Self {
        let mut text: Vec<String> = Vec::new();
        for _ in 0..3 {
            text.extend(crate::data::prose().split('\n').map(str::to_owned));
            text.push(String::new());
        }
        let log = ScrollPanel::new(ID.sub("log"), crate::data::log_lines(400)).tail(true);
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::Scrolling)),
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
            prose: ScrollPanel::new(ID.sub("prose"), text).wrap(true),
            list: ListBox::new(ID.sub("list"), vec![], SelectMode::Single),
            log,
            hand: ScrollPane::default(),
            fits: ScrollPane::default(),
            over: ScrollPane::default(),
            nest_outer: ScrollView::new(NEST_OUTER).axes(Axes::VERTICAL),
            nest_inner: ScrollView::new(NEST_INNER).axes(Axes::VERTICAL),
            virtual_rows: ScrollView::new(VIRTUAL).axes(Axes::VERTICAL),
            virtual_source: VirtualRows::new(0),
            virtual_cursor: 0,
            virtual_painted: 0,
            region: Select::new(ID.sub("region"), "Region", REGIONS, 5),
            tree: TreeView::new(ID.sub("tree"), vec![]),
            table: DataTable::try_new(ID.sub("table"), vec![], vec![])
                .expect("rectangular fixture"),
            grid: DataGrid::new(ID.sub("grid"), vec![]),
            flagged: BTreeSet::new(),
            last: None,
        };
        page.rebuild();
        page
    }

    fn profile(&self) -> NavProfile {
        match self.opts.value("scrolling.profile") {
            "vim" => NavProfile::Vim,
            _ => NavProfile::Desktop,
        }
    }

    fn precedence(&self) -> TypeAheadPrecedence {
        match self.opts.value("scrolling.precedence") {
            "type-ahead" => TypeAheadPrecedence::TypeAhead,
            _ => TypeAheadPrecedence::Shortcuts,
        }
    }

    fn timeout(&self) -> Duration {
        match self.opts.value("scrolling.timeout") {
            "0.5s" => Duration::from_millis(500),
            "2s" => Duration::from_secs(2),
            _ => Duration::from_secs(1),
        }
    }

    fn type_ahead(&self) -> bool {
        match self.opts.value("scrolling.type_ahead") {
            "on" => true,
            "off" => false,
            _ => self.profile().type_ahead(),
        }
    }

    fn track_click(&self) -> Option<TrackClick> {
        match self.opts.value("scrolling.track_click") {
            "page" => Some(TrackClick::Page),
            "jump" => Some(TrackClick::Jump),
            _ => None,
        }
    }

    fn rebuild(&mut self) {
        let o = &self.opts;
        let nav = self.profile().nav();
        let type_ahead = self.type_ahead();
        let precedence = self.precedence();
        let timeout = self.timeout();
        let context = o.number("scrolling.context").unwrap_or(1) as usize;
        let follow = match o.value("scrolling.follow") {
            "always" => FollowCursor::Always,
            _ => FollowCursor::OnMove,
        };

        let items = (1..=120)
            .map(|i| {
                let idx = i - 1;
                let item = ListItem::new(&format!("Row {i:03}"));
                if retired(idx) {
                    item.meta("retired").disabled(true)
                } else if self.flagged.contains(&idx) {
                    item.meta("flagged")
                } else {
                    item
                }
            })
            .collect();
        let mut list = ListBox::new(ID.sub("list"), items, SelectMode::Single)
            .nav(nav)
            .type_ahead(type_ahead)
            .type_ahead_timeout(timeout)
            .type_ahead_precedence(precedence)
            .wrap(o.on("scrolling.wrap"))
            .skip_disabled(o.on("scrolling.skip_disabled"));
        if let Some(click) = self.track_click() {
            list = list.track_click(click);
        }
        list.cursor = self.list.cursor;
        list.scroll.v = list.scroll.v.with_context(context);
        list.scroll.v.offset = self.list.scroll.offset;
        self.list = list;

        let bar = match o.value("scrolling.hbar") {
            "always" => ScrollbarVisibility::Always,
            "never" => ScrollbarVisibility::Never,
            _ => ScrollbarVisibility::Auto,
        };
        let mut hand = ScrollPane::default()
            .horizontal(HScroll::default().bar(bar))
            .columns(o.on("scrolling.columns"));
        if let Some(click) = self.track_click() {
            hand = hand.track_click(click);
        }
        hand.v = ScrollState::new(120)
            .with_context(context)
            .with_follow_cursor(follow);
        hand.v.offset = self.hand.v.offset;
        self.hand = hand;

        self.region = Select::new(ID.sub("region"), "Region", REGIONS, self.region.selected)
            .nav(nav)
            .type_ahead(type_ahead)
            .type_ahead_timeout(timeout)
            .type_ahead_precedence(precedence);

        let services = SERVICES
            .iter()
            .map(|s| TreeNode::leaf_meta(s, "service"))
            .collect();
        let mut tree = TreeView::new(ID.sub("tree"), services)
            .nav(nav)
            .type_ahead(type_ahead)
            .type_ahead_timeout(timeout)
            .type_ahead_precedence(precedence);
        tree.cursor = self.tree.cursor;
        self.tree = tree;

        let rows = REGIONS.iter().map(|r| vec![Cell::new(*r)]).collect();
        let mut table = DataTable::try_new(
            ID.sub("table"),
            vec![Column::new("Region", Constraint::Min(10))],
            rows,
        )
        .expect("rectangular fixture")
        .nav(nav)
        .type_ahead_timeout(timeout)
        .type_ahead_precedence(precedence);
        table.cursor_row = self.table.cursor_row;
        self.table = table;

        let mut grid = DataGrid::new(
            ID.sub("grid"),
            vec![ColumnSpec::new("Service", CellKind::Text)],
        )
        .nav(nav)
        .type_ahead_timeout(timeout)
        .type_ahead_precedence(precedence);
        grid.try_set_rows(GridRows {
            rows: SERVICES
                .iter()
                .map(|s| vec![CellValue::Text((*s).to_owned())])
                .collect(),
            total: RowTotal::Exact(SERVICES.len()),
            more: false,
        })
        .expect("rectangular fixture");
        grid.cursor = self.grid.cursor;
        self.grid = grid;
    }

    fn toggle_flag(&mut self) -> Outcome {
        let i = self.list.cursor;
        if retired(i) {
            return Outcome::Consumed;
        }
        if !self.flagged.remove(&i) {
            self.flagged.insert(i);
        }
        self.list.items[i].meta = self.flagged.contains(&i).then(|| "flagged".to_owned());
        Outcome::Changed
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let has = |id| ctx.interaction.has_focus(id);
        let focus = if [self.prose.id, self.list.id, self.log.id]
            .into_iter()
            .any(has)
        {
            Some(0)
        } else if [self.region.id, self.tree.id, self.table.id, self.grid.id]
            .into_iter()
            .any(has)
        {
            Some(2)
        } else {
            None
        };

        let top = area.height.saturating_sub(10 + 8 + 3).max(10);
        let l = self.sections.layout(
            area,
            &[
                StackChild::fixed(top),
                StackChild::fixed(10),
                StackChild::fixed(8),
            ],
            focus,
            buf,
            ctx,
        );
        match l.slot(0) {
            Some(r) => self.render_scrollers(r, buf, ctx),
            None => [self.prose.id, self.list.id, self.log.id]
                .into_iter()
                .for_each(|id| ctx.focus_stop(id)),
        }
        if let Some(r) = l.slot(1) {
            self.render_mechanics(r, buf, ctx);
        }
        match l.slot(2) {
            Some(r) => self.render_collections(r, buf, ctx),
            None => [self.region.id, self.tree.id, self.table.id, self.grid.id]
                .into_iter()
                .for_each(|id| ctx.focus_stop(id)),
        }
    }

    fn thirds(area: Rect) -> [Rect; 3] {
        let third = area.width / 3;
        [
            Rect::new(area.x, area.y, third.saturating_sub(1), area.height),
            Rect::new(
                area.x + third + 1,
                area.y,
                third.saturating_sub(1),
                area.height,
            ),
            Rect::new(
                area.x + 2 * third + 2,
                area.y,
                area.width.saturating_sub(2 * third + 2),
                area.height,
            ),
        ]
    }

    fn render_scrollers(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let cols = Self::thirds(area);

        let pad = if cols[0].width < 22 { 1 } else { 2 };

        let panel = Panel::card(Some("Wrapped text"))
            .pad_x(pad)
            .focused(ctx.interaction.focused(self.prose.id))
            .title_min(4);
        let bg = panel.bg(t);
        let inner = panel.render(cols[0], buf, t);
        self.prose.render(inner, buf, ctx, bg, prose_style);
        panel.draw_meta(
            cols[0],
            buf,
            t,
            &scrollbar::position_label(&self.prose.scroll),
        );

        let panel = Panel::card(Some("Long list"))
            .pad_x(pad)
            .focused(ctx.interaction.focused(self.list.id))
            .title_min(4);
        let bg = panel.bg(t);
        let inner = panel.render(cols[1], buf, t);
        self.list.render(inner, buf, ctx, bg);
        panel.draw_meta(
            cols[1],
            buf,
            t,
            &scrollbar::position_label(&self.list.scroll),
        );

        let lf = ctx.interaction.focused(self.log.id);
        let panel = Panel::card(Some("Log")).pad_x(pad).focused(lf).title_min(4);
        let bg = panel.bg(t);
        let inner = panel.render(cols[2], buf, t);
        self.log
            .render(inner, buf, ctx, bg, crate::pages::panels::log_style);
        let pos = scrollbar::position_label(&self.log.scroll);
        let meta = if self.log.follow {
            format!("{pos} · following")
        } else {
            pos
        };
        panel.draw_meta(cols[2], buf, t, &meta);
    }

    fn render_mechanics(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let (hand, div) = (area.width * 3 / 8, area.width / 4);
        let x1 = area.x + hand + 1;
        let x2 = x1 + div + 1;
        let col = |x: u16, w: u16| Rect::new(x, area.y, w, area.height);
        self.render_hand(col(area.x, hand.saturating_sub(1)), buf, ctx);
        let middle = col(x1, div.saturating_sub(1));
        if self.virtual_len() > 0 {
            self.render_virtual(middle, buf, ctx);
        } else if self.opts.on("scrolling.nested") {
            self.render_nested(middle, buf, ctx);
        } else {
            self.render_dividers(middle, buf, ctx);
        }
        self.render_cursor(col(x2, area.right().saturating_sub(x2)), buf, ctx);
    }

    fn render_hand(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let panel = Panel::card(Some("Hand-drawn")).title_min(4);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        self.hand.begin(ctx.track_click);
        if inner.height < 3 || inner.width < 8 {
            return;
        }

        let header = Rect::new(inner.x, inner.y, inner.width, 1);
        let shows_h = self.hand.h_cfg.bar != ScrollbarVisibility::Never;

        let region = Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1);
        let bars = scrollbar::Bars::split(region, true, shows_h);
        let body = Rect::new(region.x, region.y, region.width, bars.content.height);
        let text_w = bars.content.width as usize;
        let columns = self.hand.columns;
        let content_w = HAND_COLS.len() * HAND_W;
        if columns {
            self.hand.h.set_content(HAND_COLS.len());
            self.hand.h.set_viewport((text_w / HAND_W).max(1));
        } else {
            self.hand.h.set_content(content_w);
            self.hand.h.set_viewport(text_w);
        }
        self.hand.v.set_content(120);
        self.hand.v.set_viewport(body.height as usize);

        self.hand.follow(self.list.cursor);
        let line = |cells: &[String]| -> String {
            let full: String = cells.iter().map(|c| fit(c, HAND_W)).collect();
            if columns {
                let from = self.hand.h.offset * HAND_W;
                slice_cells(&full, from, text_w)
            } else {
                slice_cells(&full, self.hand.h.offset, text_w)
            }
        };
        let titles: Vec<String> = HAND_COLS.iter().map(|s| (*s).to_owned()).collect();
        buf.set_string(header.x, header.y, line(&titles), t.muted().bg(bg));
        for (k, i) in self.hand.v.visible_range().enumerate() {
            let style = if i == self.list.cursor {
                t.primary().bg(bg)
            } else {
                t.secondary().bg(bg)
            };
            buf.set_string(body.x, body.y + k as u16, line(&hand_row(i)), style);
        }
        ctx.scrollable(HAND, body);
        self.hand.draw_with_fade(body, buf, ctx, HAND, false, &[]);
        if let Some(hb) = bars.horizontal {
            self.hand.draw_h(hb, buf, ctx, HAND, false);
        }
    }

    fn render_dividers(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let panel = Panel::card(Some("Divider or bar")).title_min(4);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        self.fits.begin(ctx.track_click);
        self.over.begin(ctx.track_click);
        if inner.height < 3 || inner.width < 12 {
            return;
        }
        let half = inner.width / 2;
        let rows = inner.height - 1;
        let panes: [(&mut ScrollPane, WidgetId, &str, usize, Rect); 2] = [
            (
                &mut self.fits,
                FITS,
                "fits",
                rows as usize,
                Rect::new(inner.x, inner.y, half.saturating_sub(1), inner.height),
            ),
            (
                &mut self.over,
                OVER,
                "over",
                30,
                Rect::new(
                    inner.x + half + 1,
                    inner.y,
                    inner.width - half - 1,
                    inner.height,
                ),
            ),
        ];
        for (pane, id, caption, n, r) in panes {
            buf.set_string(
                r.x,
                r.y,
                truncate(caption, r.width as usize),
                t.muted().bg(bg),
            );
            let body = Rect::new(r.x, r.y + 1, r.width, rows);
            pane.v.set_content(n);
            pane.v.set_viewport(rows as usize);

            let split = scrollbar::split(body, true, scrollbar::GAP);
            for (k, i) in pane.v.visible_range().enumerate() {
                let text = truncate(&format!("item {}", i + 1), split.content.width as usize);
                buf.set_string(body.x, body.y + k as u16, text, t.secondary().bg(bg));
            }
            ctx.scrollable(id, body);
            if let Some(col) = split.track {
                pane.draw_divider(col, buf, ctx, id, false);
            }
        }
    }

    fn render_nested(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let panel = Panel::card(Some("Nested bars")).title_min(4);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        if inner.height < 4 || inner.width < 8 {
            return;
        }
        let (outer, nested) = (&mut self.nest_outer, &mut self.nest_inner);
        let content = Size::new(inner.width, inner.height + 6);
        outer.render(inner, content, buf, ctx, |frame, buf, ctx| {
            buf.set_string(frame.x, frame.y, "outer content", t.muted().bg(bg));
            let area = Rect::new(
                frame.x,
                frame.y + 2,
                frame.width,
                inner.height.saturating_sub(1),
            );
            nested.render(area, Size::new(area.width, 20), buf, ctx, |rows, buf, _| {
                for i in 0..rows.height {
                    let text = truncate(&format!("nested row {}", i + 1), rows.width as usize);
                    buf.set_string(rows.x, rows.y + i, text, t.secondary().bg(bg));
                }
            });
        });
    }

    fn virtual_len(&self) -> usize {
        self.virtual_source.len()
    }

    fn render_virtual(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let rows = self.virtual_len();
        let panel = Panel::card(Some("Virtual")).title_min(4);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        ctx.focus_stop(VIRTUAL);
        if inner.height < 3 || inner.width < 8 || rows == 0 {
            return;
        }
        let footer = inner.height >= 5;
        let body = Rect::new(
            inner.x,
            inner.y,
            inner.width,
            inner.height - u16::from(footer),
        );
        let cursor = self.virtual_cursor.min(rows - 1);
        let target = Rect::new(0, u16::try_from(cursor).unwrap_or(u16::MAX), 1, 1);
        let screen = *buf.area();
        let content = Size::new(inner.width, u16::try_from(rows).unwrap_or(u16::MAX));
        let mut painted = 0;
        let state = ctx.state(VIRTUAL);
        let (view, source) = (&mut self.virtual_rows, &self.virtual_source);
        view.render_window(
            screen,
            body,
            content,
            Some(target),
            buf,
            ctx,
            |window, buf, ctx| {
                for index in window.content_rows() {
                    painted += 1;
                    let rect = window.project_content_rect(Rect::new(
                        0,
                        u16::try_from(index).unwrap_or(u16::MAX),
                        window.viewport.width,
                        1,
                    ));
                    let mut s = state;
                    s.focused = state.focused && index == cursor;
                    let look = t.cursor_row(s, bg, CursorStyle::Fill);
                    let style = if index == cursor {
                        look.style
                    } else {
                        t.secondary().bg(bg)
                    };
                    for x in rect.left()..rect.right() {
                        buf[(x, rect.y)].set_symbol(" ").set_style(style);
                    }
                    if index == cursor {
                        buf.set_string(rect.x, rect.y, look.gutter, look.gutter_style);
                    }
                    let label =
                        truncate(source.label(index), rect.width.saturating_sub(2) as usize);
                    buf.set_string(rect.x + 2, rect.y, label, style);
                    ctx.hits.register_part(
                        VIRTUAL,
                        PartRef::item(Part::ROW, source.key(index)),
                        rect,
                    );
                }
            },
        );
        self.virtual_painted = painted;
        if footer {
            let work = self.virtual_rows.last_work();
            let line = format!("scratch {} cells", work.allocated);
            buf.set_string(
                inner.x,
                inner.bottom() - 1,
                truncate(&line, inner.width as usize),
                t.faint().bg(bg),
            );
        }
        panel.draw_meta(area, buf, t, &format!("{painted} of {rows}"));
    }

    fn virtual_key(&mut self, key: &Key) -> Outcome {
        let rows = self.virtual_len();
        if rows == 0 {
            return Outcome::Ignored;
        }
        let last = rows - 1;
        let page = usize::from(self.virtual_rows.last_layout().viewport.height.max(1));
        let at = self.virtual_cursor.min(last);
        let to = match key.code {
            KeyCode::Up if key.plain() => at.saturating_sub(1),
            KeyCode::Down if key.plain() => (at + 1).min(last),
            KeyCode::PageUp if key.plain() => at.saturating_sub(page),
            KeyCode::PageDown if key.plain() => (at + page).min(last),
            KeyCode::Home if key.plain() => 0,
            KeyCode::End if key.plain() => last,
            _ => return self.virtual_rows.on_key(key).unwrap_or(Outcome::Ignored),
        };
        if to == self.virtual_cursor {
            return Outcome::Consumed;
        }
        self.virtual_cursor = to;
        Outcome::Changed
    }

    fn render_cursor(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let panel = Panel::card(Some("Cursor")).title_min(4);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        let c: Cursor = self.list.cursor();
        let axis = match c.axis {
            Axis::Vertical => "vertical",
            Axis::Horizontal => "horizontal",
        };
        let on = |b: bool| if b { "on" } else { "off" };

        let items = &self.list.items;
        let enabled = |i: usize| !items[i].disabled;
        let label = |i: usize| items[i].label.as_str();
        let rows = RowFn {
            len: items.len(),
            enabled: &enabled,
            label: &label,
        };
        let open = (0..rows.len()).filter(|&i| rows.enabled(i)).count();
        let last = match self.last {
            Some((step, moved)) => format!("{} → {}", step_text(step), moved_text(moved)),
            None => "no key yet".to_owned(),
        };
        let precedence = match self.precedence() {
            TypeAheadPrecedence::Shortcuts => "shortcuts first",
            TypeAheadPrecedence::TypeAhead => "typing first",
        };
        let profile = match self.profile() {
            NavProfile::Desktop => "Desktop",
            NavProfile::Vim => "Vim",
        };
        let keys = if self.list.type_ahead {
            format!(
                "{profile} · {} · {precedence}",
                self.opts.value("scrolling.timeout")
            )
        } else {
            format!("{profile} · no type-ahead")
        };
        let lines = [
            format!("row {} · {axis}", c.index + 1),
            format!(
                "wrap {} · skip disabled {}",
                on(c.wrap),
                on(c.skip_disabled)
            ),
            format!("RowFn: {open} of {} open", rows.len()),
            format!("Last: {last}"),
            "KeyStep: Nav · Type".to_owned(),
            "Moved: To · Stayed · Ignored".to_owned(),
            keys,
        ];
        for (k, l) in lines.iter().enumerate() {
            let y = inner.y + k as u16;
            if y >= inner.bottom() {
                break;
            }
            let style = if k == 4 || k == 5 {
                t.faint()
            } else {
                t.secondary()
            };
            buf.set_string(inner.x, y, truncate(l, inner.width as usize), style.bg(bg));
        }
    }

    fn render_collections(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let panel = Panel::card(Some("Type to jump · select, tree, table, grid")).title_min(4);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        if inner.is_empty() {
            for id in [self.region.id, self.tree.id, self.table.id, self.grid.id] {
                ctx.focus_stop(id);
            }
            return;
        }
        let w = inner.width / 4;
        let cell = |i: u16| Rect::new(inner.x + i * w, inner.y, w.saturating_sub(2), inner.height);
        let r = cell(0);
        self.region
            .render(Rect::new(r.x, r.y, r.width, 2.min(r.height)), buf, ctx, bg);
        self.tree.render(cell(1), buf, ctx, bg);
        self.table.render(cell(2), buf, ctx, bg);
        let g = cell(3);
        self.grid.render(
            Rect::new(g.x, g.y, inner.right().saturating_sub(g.x), g.height),
            buf,
            ctx,
            bg,
        );
    }

    fn on_list_key(&mut self, key: &Key) -> Outcome {
        let before = self.list.cursor;
        let step = KeyStep::of(key, &self.list.nav, self.list.type_ahead);
        let o = self.list.on_key(key);

        if o == Outcome::Ignored && key.plain() && key.code == KeyCode::Char('f') {
            return self.toggle_flag();
        }
        if let Some(step) = step {
            let moved = match o {
                Outcome::Ignored => Moved::Ignored,
                _ if self.list.cursor != before => Moved::To(self.list.cursor),
                _ => Moved::Stayed(self.list.cursor),
            };
            self.last = Some((step, moved));
        }
        o
    }

    fn panes(&mut self) -> [(&mut ScrollPane, WidgetId); 3] {
        [
            (&mut self.hand, HAND),
            (&mut self.fits, FITS),
            (&mut self.over, OVER),
        ]
    }
}

fn step_text(step: KeyStep) -> String {
    match step {
        KeyStep::Nav(nav) => format!("Nav({nav:?})"),
        KeyStep::Type(c) => format!("Type('{c}')"),
    }
}

fn moved_text(moved: Moved) -> String {
    match moved {
        Moved::To(i) => format!("To(row {})", i + 1),
        Moved::Stayed(i) => format!("Stayed(row {})", i + 1),
        Moved::Ignored => "Ignored".to_owned(),
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let lines =
        |n: usize| -> Vec<String> { (1..=n).map(|i| format!("line {i} of the panel")).collect() };
    StateGrid::new(id)
        .label_width(14)
        .column_width(24)
        .narrow_width(14)
        .rows(&[Rest, Hover, Pressed, Empty, Overflow, Narrow])
        .column("Lines", ScrollPanel::new(id.child(0), lines(3)))
        .column(
            "Wrapped",
            ScrollPanel::new(
                id.child(1),
                vec!["A wrapped paragraph that runs longer than its row.".to_owned()],
            )
            .wrap(true),
        )
        .column(
            "Log tail",
            ScrollPanel::new(id.child(2), crate::data::log_lines(3)).tail(true),
        )
}

impl ScrollingPage {
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Outcome {
        let click = ev.is(Phase::Click);
        if let Some(r) = self.region.route(ev) {
            if ev.id == self.region.id && (click || ev.is(Phase::Press)) {
                cx.focus.focus(self.region.id);
            }
            return r.outcome();
        }
        if let Some(r) = self.list.route(ev) {
            if click && ev.index_of(Part::ROW).is_some() {
                cx.focus.focus(self.list.id);
            }
            return r.outcome();
        }
        for p in [&mut self.prose, &mut self.log] {
            if let Some(r) = p.route(ev) {
                return r.outcome();
            }
        }
        if let Some(r) = self.tree.route(ev) {
            if click && self.tree.row_at(ev).is_some() {
                cx.focus.focus(self.tree.id);
            }
            return r.outcome();
        }
        if let Some(r) = self.table.route(ev) {
            if click && !scrollbar::is_bar_of(ev.id, self.table.id) {
                cx.focus.focus(self.table.id);
            }
            return r.outcome();
        }
        if let Some(r) = self.grid.route(ev) {
            if click {
                cx.focus.focus(self.grid.id);
            }
            return r.outcome();
        }

        if ev.on(VIRTUAL) {
            if click && let Some(index) = ev.index_of(Part::ROW) {
                self.virtual_cursor = index;
                cx.focus.focus(VIRTUAL);
                return Outcome::Changed;
            }
            if let Some(r) = self.virtual_rows.pointer(ev) {
                return r.outcome();
            }
        }
        if let Some((axis, delta)) = ev.wheel() {
            for (pane, owner) in self.panes() {
                let over = ev.id == owner || scrollbar::is_bar_of(ev.id, owner);
                if !over {
                    continue;
                }
                return match axis {
                    Axis::Vertical => pane
                        .wheel(Axis::Vertical, delta)
                        .outcome()
                        .or(Outcome::Consumed),
                    Axis::Horizontal if owner == HAND => {
                        pane.wheel(Axis::Horizontal, delta).outcome()
                    }
                    Axis::Horizontal => Outcome::Ignored,
                };
            }
            return Outcome::Ignored;
        }
        for view in [&mut self.nest_inner, &mut self.nest_outer] {
            if let Some(r) = view.pointer(ev) {
                return r.outcome();
            }
        }
        for (pane, owner) in self.panes() {
            if !scrollbar::is_bar_of(ev.id, owner) {
                continue;
            }
            return match ev.phase() {
                Some(Phase::Press) => pane.press(ev.pos()).outcome().or(Outcome::Consumed),
                Some(Phase::Drag) => pane.drag(ev.pos()).outcome(),

                Some(Phase::Click | Phase::Release) => {
                    pane.release();
                    Outcome::Consumed
                }
                _ => Outcome::Ignored,
            };
        }
        Outcome::Ignored
    }
}

impl Page for ScrollingPage {
    fn title(&self) -> &'static str {
        "Scrolling"
    }
    fn blurb(&self) -> &'static str {
        "Wheel under the pointer, keys on the focused container, thumb shows where you are, edges fade toward hidden rows"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => self.render_playground(body, buf, ctx),
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        if let Some(o) = self.sections.handle(ev) {
            return o;
        }
        match ev {
            PageEvent::Tick => {
                if self.log.lines().len() < 2000 && self.log.follow {
                    let n = self.log.lines().len();
                    if let Some(line) = crate::data::log_lines(n + 1).pop() {
                        self.log.push(line);
                    }
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            PageEvent::Key(key) => {
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                if f == VIRTUAL {
                    return self.virtual_key(key);
                }
                if f == self.prose.id {
                    return self.prose.on_key(key);
                }
                if f == self.log.id {
                    return self.log.on_key(key);
                }
                if f == self.list.id {
                    return self.on_list_key(key);
                }
                if f == self.region.id {
                    return self.region.on_key(key).0;
                }
                if f == self.tree.id {
                    return self.tree.on_key(key).outcome();
                }
                if f == self.table.id {
                    return self.table.on_key(key).outcome();
                }
                if f == self.grid.id {
                    return self.grid.on_key(key).outcome();
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => self.pointer(ev, cx),
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self) -> bool {
        self.log.follow && self.log.lines().len() < 2000
    }

    fn layer_open(&self) -> bool {
        self.template.popup_open() || self.region.open
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }

        let nav_hints = |nav: &NavKeys, type_ahead: bool, step: &'static str| -> Vec<Hint> {
            let mut v = Vec::new();
            if let Some(k) = nav.move_hint(type_ahead) {
                v.push((k, step));
            }
            if let Some(k) = nav.page_hint() {
                v.push((k, "Page"));
            }
            if let Some(k) = nav.ends_hint(type_ahead) {
                v.push((k, "Ends"));
            }
            v
        };
        if focus == Some(self.log.id) {
            let mut v = nav_hints(&self.log.nav, false, "Scroll");
            if let Some(k) = junie_tui::core::keymap::hint_key(
                &self.log.keymap,
                junie_tui::widgets::panel::PanelCmd::Follow,
            ) {
                v.push((k, "Follow"));
            }
            v
        } else if focus == Some(self.list.id) {
            let mut v = nav_hints(&self.list.nav, self.list.type_ahead, "Move");

            if !self.list.type_ahead || self.precedence() == TypeAheadPrecedence::Shortcuts {
                v.push(("f", "Flag"));
            }
            v
        } else if focus == Some(VIRTUAL) {
            vec![("↑↓", "Move"), ("PgUp/PgDn", "Page"), ("Home/End", "Ends")]
        } else if focus == Some(self.prose.id) {
            nav_hints(&self.prose.nav, false, "Scroll")
        } else if focus == Some(self.region.id) {
            vec![("Enter", "Open"), ("↑↓", "Choose")]
        } else if focus == Some(self.tree.id) {
            nav_hints(&self.tree.nav, self.tree.type_ahead, "Move")
        } else if focus == Some(self.table.id) {
            nav_hints(&self.table.nav, self.table.type_ahead, "Move")
        } else if focus == Some(self.grid.id) {
            nav_hints(&self.grid.nav, self.grid.type_ahead, "Move")
        } else {
            vec![("Tab", "Next")]
        }
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
            let rows = self.opts.number("scrolling.window").map_or(0, usize::from);
            if rows != self.virtual_len() {
                self.virtual_source = VirtualRows::new(rows);
                self.virtual_cursor = 0;
                self.virtual_rows.reset();
            }
            self.rebuild();
        }
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use junie_tui::core::focus::FocusRing;
    use junie_tui::core::hit::HitRegistry;
    use junie_tui::theme::ColorLevel;
    use junie_tui::ui::ctx::Interaction;

    fn render(page: &mut ScrollingPage) -> (Buffer, HitRegistry) {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let area = Rect::new(0, 0, 120, 40);
        let mut buf = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        page.render(area, &mut buf, &mut ctx);
        (buf, hits)
    }

    fn text(buf: &Buffer) -> String {
        buf.content().iter().map(|c| c.symbol()).collect()
    }

    #[test]
    fn nested_bars_switch_is_listed_on_the_options_tab_and_off_by_default() {
        let opt = crate::options::find("scrolling.nested").expect("--opt can set it");
        assert_eq!(opt.default, 0);
        let view = crate::options::OptionsView::new(
            ID.sub("options"),
            &[crate::options::Group {
                heading: "Scrolling",
                opts: crate::options::page_options(PageId::Scrolling),
                empty: "",
            }],
        );
        assert!(view.control_id("scrolling.nested").is_some());
    }

    #[test]
    fn nested_bars_are_off_by_default() {
        let mut page = ScrollingPage::new();
        let (buf, hits) = render(&mut page);
        assert!(!text(&buf).contains("Nested bars"));
        assert!(hits.bar(scrollbar::id_for(NEST_INNER)).is_none());
    }

    #[test]
    fn nested_bars_show_a_partially_clipped_child_scrollbar() {
        let mut page = ScrollingPage::new();
        page.set_option("scrolling.nested", 1);
        let (buf, hits) = render(&mut page);
        assert!(text(&buf).contains("Nested bars"));

        let full = page
            .nest_inner
            .last_layout()
            .v_track
            .expect("the child overflows, so it has a track");
        let shown = hits
            .bar(scrollbar::id_for(NEST_INNER))
            .expect("part of the child's track is visible");
        assert!(
            shown.track.height < full.height,
            "the visible fragment ({}) is shorter than the full track ({})",
            shown.track.height,
            full.height
        );
        assert!(hits.bar(scrollbar::id_for(NEST_OUTER)).is_some());
    }

    #[test]
    fn virtual_rows_switch_is_listed_on_the_options_tab_and_off_by_default() {
        let opt = crate::options::find("scrolling.window").expect("--opt can set it");
        assert_eq!(opt.default, 0);
        let view = crate::options::OptionsView::new(
            ID.sub("options"),
            &[crate::options::Group {
                heading: "Scrolling",
                opts: crate::options::page_options(PageId::Scrolling),
                empty: "",
            }],
        );
        assert!(view.control_id("scrolling.window").is_some());
        let mut page = ScrollingPage::new();
        let (buf, _) = render(&mut page);
        assert!(!text(&buf).contains("Virtual"));
    }

    #[test]
    fn virtual_rows_paint_and_allocate_the_viewport_for_any_row_count() {
        let mut seen = Vec::new();
        for (value, rows) in [(1, 100usize), (2, 1000), (3, 10000)] {
            let mut page = ScrollingPage::new();
            page.set_option("scrolling.window", value);
            let (buf, hits) = render(&mut page);
            assert!(text(&buf).contains("Virtual"));
            let layout = page.virtual_rows.last_layout();
            let viewport = usize::from(layout.viewport.width) * usize::from(layout.viewport.height);
            let work = page.virtual_rows.last_work();
            assert!(page.virtual_painted <= usize::from(layout.viewport.height));
            assert!(
                work.allocated <= viewport && work.filled <= viewport && work.copied <= viewport
            );
            assert_eq!(
                page.virtual_rows.pane.v.content_len,
                rows.max(usize::from(layout.viewport.height))
            );
            let first = hits
                .part_of(VIRTUAL)
                .expect("rows register hits against the container");
            assert_eq!(first.owner, VIRTUAL);
            seen.push((page.virtual_painted, work.allocated));
        }
        assert!(
            seen.iter().all(|s| *s == seen[0]),
            "cost ignores the row count: {seen:?}"
        );
    }

    #[test]
    fn virtual_rows_keys_move_the_cursor_and_reveal_the_last_row() {
        let mut page = ScrollingPage::new();
        page.set_option("scrolling.window", 3);
        render(&mut page);
        let key = |code| Key {
            code,
            mods: ratatui::crossterm::event::KeyModifiers::NONE,
        };
        assert_eq!(page.virtual_key(&key(KeyCode::Down)), Outcome::Changed);
        assert_eq!(page.virtual_cursor, 1);
        assert_eq!(page.virtual_key(&key(KeyCode::End)), Outcome::Changed);
        let (buf, _) = render(&mut page);
        assert!(text(&buf).contains("Row 10000"), "the last row is revealed");
        assert!(!text(&buf).contains("Row 00001"));
        assert_eq!(page.virtual_key(&key(KeyCode::Home)), Outcome::Changed);
        let (buf, _) = render(&mut page);
        assert!(text(&buf).contains("Row 00001"));
    }
}

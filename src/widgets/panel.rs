use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;
use ratatui::widgets::{Block, Borders, Widget};

use crate::core::event::{Key, Outcome};
use crate::core::hit::Axis;
use crate::core::id::WidgetId;
use crate::core::keymap::{Actions, KeyBinding, KeyMap, Nav, NavKeys, lookup};
use crate::core::response::Response;
use crate::core::scroll::{HScroll, ScrollPane, Scrollable};
use crate::theme::Theme;
use crate::ui::ctx::{RenderCtx, fill};
use crate::ui::edit_menu::{
    EditCaps, EditMenu, EditMenuHost, edit_menu_key, edit_menu_pointer, edit_menu_route,
};
use crate::ui::measure::terminal_extent;
use crate::widgets::field_common::PointerSelect;
use crate::widgets::scrollbar;
use crate::widgets::text_select::{self, TextSelect};
use crate::widgets::viewport::CellPos;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    Card,
    Framed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelFocus {
    #[default]
    Title,

    Bar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CardHeading {
    Plain,

    #[default]
    Rule,

    Band,

    Emphasis,
}

impl CardHeading {
    pub const ALL: [CardHeading; 4] = [
        CardHeading::Plain,
        CardHeading::Rule,
        CardHeading::Band,
        CardHeading::Emphasis,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CardHeading::Plain => "plain",
            CardHeading::Rule => "rule",
            CardHeading::Band => "band",
            CardHeading::Emphasis => "emphasis",
        }
    }
}

pub const TITLE_MIN: u16 = 12;

const META_MIN: u16 = 4;

fn cut_meta(meta: &str, room: u16, ellipsis: &str) -> Option<String> {
    if room < META_MIN {
        return None;
    }
    let cut = crate::ui::text::truncate(meta, room as usize);
    let body = cut.trim_end_matches('…');
    let next = meta[body.len()..].chars().next();
    let numeric = |c: char| c.is_ascii_digit() || matches!(c, '–' | '-' | '/' | ',' | '.' | '~');
    let mid_number = body.chars().last().is_some_and(numeric) && next.is_some_and(numeric);
    let body = if mid_number {
        body.rsplit_once(' ').map(|(head, _)| head).unwrap_or("")
    } else {
        body
    }
    .trim_end_matches([' ', '·']);
    (crate::ui::text::width(body) as u16 + 1 >= META_MIN).then(|| format!("{body}{ellipsis}"))
}

pub struct Panel<'a> {
    pub title: Option<&'a str>,
    pub kind: PanelKind,
    pub focused: bool,

    pub meta: Option<&'a str>,
    pub badge: Option<(&'a str, crate::theme::BadgeKind)>,
    pub bg_override: Option<Color>,

    pub focus_mark: PanelFocus,

    pub title_min: u16,

    pub meta_short: Option<&'a str>,

    pub pad_x: u16,

    pub heading: Option<CardHeading>,
}

pub const CARD_PAD: u16 = 2;

impl<'a> Panel<'a> {
    pub fn card(title: Option<&'a str>) -> Self {
        Self {
            title,
            kind: PanelKind::Card,
            focused: false,
            meta: None,
            badge: None,
            bg_override: None,
            focus_mark: PanelFocus::Title,
            title_min: TITLE_MIN,
            meta_short: None,
            pad_x: CARD_PAD,
            heading: None,
        }
    }
    pub fn framed(title: Option<&'a str>) -> Self {
        Self {
            title,
            kind: PanelKind::Framed,
            focused: false,
            meta: None,
            badge: None,
            bg_override: None,
            focus_mark: PanelFocus::Title,
            title_min: TITLE_MIN,
            meta_short: None,
            pad_x: CARD_PAD,
            heading: None,
        }
    }
    pub fn focused(mut self, f: bool) -> Self {
        self.focused = f;
        self
    }
    pub fn focus_mark(mut self, mark: PanelFocus) -> Self {
        self.focus_mark = mark;
        self
    }
    pub fn meta(mut self, m: &'a str) -> Self {
        self.meta = Some(m);
        self
    }

    pub fn meta_short(mut self, m: &'a str) -> Self {
        self.meta_short = Some(m);
        self
    }

    pub fn pad_x(mut self, cells: u16) -> Self {
        self.pad_x = cells;
        self
    }

    pub fn heading_style(mut self, style: CardHeading) -> Self {
        self.heading = Some(style);
        self
    }

    pub(crate) fn heading_for(&self, t: &Theme) -> CardHeading {
        match self.kind {
            PanelKind::Card => self.heading.unwrap_or(t.card_heading),
            PanelKind::Framed => CardHeading::Plain,
        }
    }

    fn head_bg(&self, t: &Theme) -> Color {
        let bg = self.bg(t);
        match self.heading_for(t) {
            CardHeading::Band if self.title.is_some() => t.ladder(t.plane_of(bg).up()).rest,
            _ => bg,
        }
    }

    fn title_span(&self, area: Rect) -> (u16, u16) {
        let pad = match self.kind {
            PanelKind::Card => self.pad_x,
            PanelKind::Framed => 2,
        };
        (area.x + pad, area.width.saturating_sub(pad * 2))
    }

    pub fn title_min(mut self, cells: u16) -> Self {
        self.title_min = cells;
        self
    }

    pub fn bg(&self, t: &Theme) -> Color {
        if let Some(bg) = self.bg_override {
            return bg;
        }
        match self.kind {
            PanelKind::Card => t.surface,
            PanelKind::Framed => t.canvas,
        }
    }

    pub fn draw_meta(&self, area: Rect, buf: &mut Buffer, t: &Theme, meta: &str) {
        let area = area.intersection(*buf.area());
        if area.is_empty() || (self.kind == PanelKind::Framed && area.width <= 4) {
            return;
        }
        let bg = self.head_bg(t);
        let late = Panel {
            title: self.title,
            kind: self.kind,
            focused: self.focused,
            meta: Some(meta),
            badge: self.badge,
            bg_override: self.bg_override,
            focus_mark: self.focus_mark,
            title_min: self.title_min,
            meta_short: self.meta_short,
            pad_x: self.pad_x,
            heading: self.heading,
        };
        let (x, w) = self.title_span(area);
        let row = Rect::new(x, area.y, w, 1);
        fill(buf, row, Style::new().bg(bg));
        late.title_row(row.x, row.y, row.width, buf, t, bg);
    }

    pub fn mark_clipped(&self, inner: Rect, buf: &mut Buffer, t: &Theme) {
        let inner = inner.intersection(*buf.area());
        if inner.is_empty() {
            return;
        }
        let bg = self.bg(t);
        let (x, y) = (inner.right() - 1, inner.bottom() - 1);
        buf.set_string(x, y, t.glyphs.ellipsis, t.faint().bg(bg));
    }

    pub fn try_render(&self, area: Rect, buf: &mut Buffer, t: &Theme) -> Option<Rect> {
        let inner = self.render(area, buf, t);
        (!inner.is_empty()).then_some(inner)
    }

    pub fn outer(&self, content: ratatui::layout::Size) -> ratatui::layout::Size {
        let w = |s: &str| crate::ui::text::width(s);
        let title = self.title.map(w).unwrap_or(0);
        let meta = self.meta.filter(|m| !m.is_empty()).map(w).unwrap_or(0);
        let (width, height) = (usize::from(content.width), usize::from(content.height));
        match self.kind {
            PanelKind::Card => {
                let pad = usize::from(self.pad_x) * 2;
                let row = title + if meta > 0 { meta + 1 } else { 0 };
                let head = if self.title.is_some() { 3 } else { 2 };
                ratatui::layout::Size::new(
                    terminal_extent((width + pad).max(row + pad)),
                    terminal_extent(height + head),
                )
            }
            PanelKind::Framed => {
                let row = if meta > 0 {
                    title + meta + 5
                } else {
                    title + 2
                };
                ratatui::layout::Size::new(
                    terminal_extent((width + 5).max(row + 4)),
                    terminal_extent(height + 2),
                )
            }
        }
    }

    pub fn inner(&self, outer: ratatui::layout::Size) -> ratatui::layout::Size {
        let (w, h) = match self.kind {
            PanelKind::Card => (self.pad_x * 2, if self.title.is_some() { 3 } else { 2 }),
            PanelKind::Framed => (5, 2),
        };
        ratatui::layout::Size::new(
            outer.width.saturating_sub(w),
            outer.height.saturating_sub(h),
        )
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, t: &Theme) -> Rect {
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return area;
        }
        let bg = self.bg(t);
        match self.kind {
            PanelKind::Card => {
                fill(buf, area, Style::new().bg(bg));
                let inner = area.inner(ratatui::layout::Margin::new(self.pad_x, 1));
                let heading = self.heading_for(t);
                let head_bg = self.head_bg(t);
                if heading == CardHeading::Band && self.title.is_some() {
                    fill(
                        buf,
                        Rect::new(area.x, area.y, area.width, 1),
                        Style::new().bg(head_bg),
                    );
                }
                if self.focused && self.title.is_some() && self.focus_mark == PanelFocus::Bar {
                    let s = crate::ui::ctx::VisualState {
                        focused: true,
                        ..Default::default()
                    };
                    buf.set_string(
                        area.x + self.pad_x.saturating_sub(1),
                        area.y,
                        t.gutter_symbol(s),
                        t.gutter(s, head_bg, false),
                    );
                }
                let (x, w) = self.title_span(area);
                self.title_row(x, area.y, w, buf, t, head_bg);
                if heading == CardHeading::Rule && self.title.is_some() && area.height > 2 {
                    let rule = t.glyphs.h_divider.repeat(w as usize);
                    buf.set_string(x, area.y + 1, rule, Style::new().fg(t.rule_on(bg)).bg(bg));
                }
                if self.title.is_some() {
                    Rect::new(
                        inner.x,
                        inner.y + 1,
                        inner.width,
                        inner.height.saturating_sub(1),
                    )
                } else {
                    inner
                }
            }
            PanelKind::Framed => {
                fill(buf, area, Style::new().bg(bg));
                let block = Block::new()
                    .borders(Borders::ALL)
                    .border_set(t.glyphs.border)
                    .border_style(t.border(self.focused).bg(bg));
                block.render(area, buf);
                if area.width > 4 {
                    self.title_row(area.x + 2, area.y, area.width.saturating_sub(4), buf, t, bg);
                }
                let inner = area.inner(ratatui::layout::Margin::new(1, 1));
                Rect::new(
                    inner.x + 2,
                    inner.y,
                    inner.width.saturating_sub(3),
                    inner.height,
                )
            }
        }
    }

    fn title_style(&self, t: &Theme, bg: Color) -> Style {
        use crate::theme::ColorLevel;
        use ratatui::style::Modifier;
        let mono = t.level == ColorLevel::Mono;
        let base = if self.focused {
            t.title().bg(bg)
        } else {
            t.secondary().bg(bg)
        };
        match self.heading_for(t) {
            CardHeading::Plain | CardHeading::Rule => base,
            CardHeading::Band => {
                let fg = t.legible(base.fg.unwrap_or(t.text_primary), bg);
                let s = base.fg(fg);
                if mono {
                    s.add_modifier(Modifier::UNDERLINED)
                } else {
                    s
                }
            }
            CardHeading::Emphasis => {
                let s = base.add_modifier(Modifier::BOLD);
                if mono && self.focused {
                    s.add_modifier(Modifier::UNDERLINED)
                } else {
                    s
                }
            }
        }
    }

    fn meta_style(&self, t: &Theme, bg: Color) -> Style {
        match self.heading_for(t) {
            CardHeading::Band => Style::new().fg(t.legible(t.text_faint, bg)).bg(bg),
            _ => t.faint().bg(bg),
        }
    }

    fn title_row(&self, x: u16, y: u16, w: u16, buf: &mut Buffer, t: &Theme, bg: Color) {
        if w == 0 {
            return;
        }
        let mut cx = x;
        let framed = self.kind == PanelKind::Framed;
        let pad: u16 = if framed { 2 } else { 0 };

        let title_min = self
            .title
            .map(|t| (crate::ui::text::width(t) as u16).min(self.title_min))
            .unwrap_or(0);
        let ell = t.glyphs.ellipsis;
        let meta = self.meta.filter(|m| !m.is_empty()).and_then(|m| {
            let room = w.saturating_sub(pad + if title_min > 0 { title_min + 1 } else { 0 });
            let fits = |m: &str| crate::ui::text::width(m) as u16 <= room;
            if fits(m) {
                return Some(m.to_owned());
            }
            let short = self
                .meta_short
                .map(str::to_owned)
                .unwrap_or_else(|| m.replace(" of ", "/"));

            let head = short.split(" · ").next().unwrap_or_default().to_owned();
            if fits(&short) {
                Some(short)
            } else if fits(&head) && !head.is_empty() {
                Some(head)
            } else {
                cut_meta(&short, room, ell)
            }
        });
        let meta_w = meta
            .as_ref()
            .map(|m| crate::ui::text::width(m) as u16 + pad)
            .unwrap_or(0);
        if let Some(title) = self.title {
            let style = self.title_style(t, bg);
            let room = if meta_w > 0 {
                w.saturating_sub(meta_w + 1 + pad)
            } else {
                w.saturating_sub(pad)
            };
            let full = title;
            let title = crate::ui::text::truncate(full, room as usize);

            let cut = Rect::new(
                cx + u16::from(framed),
                y,
                crate::ui::text::width(&title) as u16,
                1,
            );
            crate::ui::reveal::queue_cut(cut, full);
            let title = if framed { format!(" {title} ") } else { title };
            buf.set_string(cx, y, &title, style);
            cx += crate::ui::text::width(&title) as u16;
        }
        let mut right = x + w;

        if let Some(meta) = meta.filter(|m| !m.is_empty()) {
            let text = if framed { format!(" {meta} ") } else { meta };
            let tw = crate::ui::text::width(&text) as u16;
            if right >= cx + tw + u16::from(cx > x) {
                right = right.saturating_sub(tw);
                buf.set_string(right, y, &text, self.meta_style(t, bg));
            }
        }
        if let Some((badge, kind)) = self.badge {
            let text = format!(" {badge} ");
            let bw = crate::ui::text::width(&text) as u16;
            if right > cx + bw + 1 {
                right = right.saturating_sub(bw + 1);
                buf.set_string(right, y, &text, t.badge(kind));
            }
        }
    }
}

use ratatui::style::Style;

#[derive(Debug, Clone)]
pub struct ScrollPanel {
    pub id: WidgetId,
    pub lines: Vec<String>,

    pub scroll: ScrollPane,

    pub follow: bool,

    pub tail: bool,
    pub wrap: bool,
    pub area: Rect,

    wrapped_cache: (u16, Vec<String>, Vec<usize>),

    pub horizontal: Option<HScroll>,

    pub nav: NavKeys,

    pub keymap: KeyMap<PanelCmd>,

    pub pointer_select: PointerSelect,

    select: TextSelect<CellPos>,

    select_w: u16,

    pub edit_menu: EditMenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelCmd {
    Follow,

    Copy,

    SelectAll,
}

impl Actions for PanelCmd {
    fn label(self) -> &'static str {
        match self {
            PanelCmd::Follow => "Follow",
            PanelCmd::Copy => "Copy",
            PanelCmd::SelectAll => "Select all",
        }
    }

    fn default_keymap() -> KeyMap<Self> {
        let mut map = vec![
            (KeyBinding::char('f'), PanelCmd::Follow),
            (KeyBinding::cmd('c'), PanelCmd::Copy),
            (KeyBinding::char('y'), PanelCmd::Copy),
        ];
        map.extend(crate::core::keymap::select_all_bindings(
            PanelCmd::SelectAll,
        ));
        map
    }
}

impl ScrollPanel {
    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn keymap(mut self, keymap: KeyMap<PanelCmd>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn new(id: WidgetId, lines: Vec<String>) -> Self {
        Self {
            id,
            lines,
            scroll: ScrollPane::default(),
            follow: false,
            tail: false,
            wrap: false,
            area: Rect::ZERO,
            wrapped_cache: (0, vec![], vec![]),
            horizontal: None,
            nav: NavKeys::default(),
            keymap: PanelCmd::default_keymap(),
            pointer_select: PointerSelect::DEFAULT,
            select: TextSelect::default(),
            select_w: 0,
            edit_menu: EditMenu::default(),
        }
    }

    pub fn context_menu(mut self, on: bool) -> Self {
        self.edit_menu.enabled = on;
        self
    }

    pub fn pointer_select(mut self, policy: PointerSelect) -> Self {
        self.pointer_select = policy;
        self
    }

    pub fn horizontal(mut self, policy: HScroll) -> Self {
        self.horizontal = Some(policy);
        self
    }

    fn h_enabled(&self) -> bool {
        !self.wrap && self.scroll.h_cfg.enabled
    }

    fn scroll_h(&mut self, delta: isize) -> Outcome {
        self.wheel(Axis::Horizontal, delta as i32).outcome()
    }

    pub fn wrap(mut self, w: bool) -> Self {
        self.wrap = w;
        self
    }

    pub fn tail(mut self, t: bool) -> Self {
        self.tail = t;
        self.follow = t;
        self
    }

    pub fn push(&mut self, line: String) {
        self.lines.push(line);
        self.wrapped_cache.0 = 0;
    }

    fn settle_follow(&mut self) {
        self.follow = self.tail && self.scroll.at_end();
    }

    pub fn on_key(&mut self, key: &Key) -> Outcome {
        if let Some(o) = edit_menu_key(self, key) {
            return o;
        }

        let cmd = lookup(&self.keymap, key).or_else(|| {
            (key.ctrl_char('c') && self.selection().is_some()).then_some(PanelCmd::Copy)
        });
        match cmd {
            Some(PanelCmd::SelectAll) => return self.select_all(),
            Some(PanelCmd::Copy) => {
                let Some(text) = self.selected_text() else {
                    return Outcome::Ignored;
                };
                crate::core::edit::clipboard::copy(&text);
                return Outcome::Consumed;
            }
            _ => {}
        }
        if key.is(ratatui::crossterm::event::KeyCode::Esc) && key.plain() {
            return self.clear_selection();
        }
        if let Some(PanelCmd::Follow) = lookup(&self.keymap, key)
            && self.tail
        {
            let was = self.follow;
            if was {
                self.follow = false;
            } else {
                self.scroll.jump_end();
                self.follow = true;
            }
            return Outcome::Changed;
        }
        let Some(nav) = self.nav.action(key).filter(|_| key.plain()) else {
            return Outcome::Ignored;
        };
        let moved = match nav {
            Nav::Up => self.scroll.scroll_by(-1),
            Nav::Down => self.scroll.scroll_by(1),
            Nav::PageUp => self.scroll.page_up(),
            Nav::PageDown => self.scroll.page_down(),
            Nav::Home => self.scroll.jump_start(),
            Nav::End => self.scroll.jump_end(),

            Nav::Left => return self.scroll_h(-1),
            Nav::Right => return self.scroll_h(1),
        };
        let was = self.follow;
        self.settle_follow();
        if moved || was != self.follow {
            Outcome::Changed
        } else {
            Outcome::Consumed
        }
    }

    pub fn measure(&mut self, area: Rect) {
        self.area = area;
        let text_w = area.width.saturating_sub(2);
        if self.wrap && self.wrapped_cache.0 != text_w {
            let mut out = Vec::new();
            let mut src = Vec::new();
            for (i, l) in self.lines.iter().enumerate() {
                let rows = crate::ui::text::wrap(l, text_w as usize);
                src.extend(std::iter::repeat_n(i, rows.len()));
                out.extend(rows);
            }
            self.wrapped_cache = (text_w, out, src);
        }

        if self.select_w != text_w {
            if self.wrap {
                self.select = TextSelect::default();
            }
            self.select_w = text_w;
        }
        let len = if self.wrap {
            self.wrapped_cache.1.len()
        } else {
            self.lines.len()
        };

        let widest = if self.h_enabled() {
            self.lines
                .iter()
                .map(|l| crate::ui::text::width(l))
                .max()
                .unwrap_or(0)
        } else {
            0
        };
        self.scroll.h.set_content(widest);
        self.scroll.h.set_viewport(text_w as usize);
        let has_hb =
            self.h_enabled() && area.height > 1 && self.scroll.h_cfg.shows_bar(&self.scroll.h);
        self.scroll.set_content(len);
        self.scroll
            .set_viewport(area.height.saturating_sub(u16::from(has_hb)) as usize);
        if self.follow {
            self.scroll.jump_end();
        }
    }

    pub fn track_click(mut self, click: crate::core::scroll::TrackClick) -> Self {
        self.scroll.track_click = Some(click);
        self
    }

    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
        style_line: fn(&Theme, &str) -> Style,
    ) {
        self.render_panel(area, buf, ctx, bg, style_line);

        self.edit_menu.render(self.area, buf, ctx);
    }

    fn render_panel(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
        style_line: fn(&Theme, &str) -> Style,
    ) {
        self.scroll.begin(ctx.track_click);
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }
        self.scroll.h_cfg = self.horizontal.unwrap_or(ctx.hscroll);
        self.measure(area);
        let t = ctx.theme;

        let focused = ctx
            .interaction
            .focused_under(self.id, self.edit_menu.is_open());
        let text_w = area.width.saturating_sub(2);
        let lines: &Vec<String> = if self.wrap {
            &self.wrapped_cache.1
        } else {
            &self.lines
        };
        ctx.control(self.id, area, false);
        ctx.scrollable(self.id, area);

        let rows_h = (self.scroll.viewport_len as u16).min(area.height);
        ctx.autoscroll(Rect::new(area.x, area.y, area.width, rows_h));
        let sel = self.select.range();
        if sel.is_some() {
            ctx.selection(self.id);
        }
        let hoff = if self.h_enabled() {
            self.scroll.h.offset
        } else {
            0
        };
        for (i, li) in self.scroll.visible_range().enumerate() {
            let y = area.y + i as u16;
            let line = &lines[li];
            let st = style_line(t, line).bg(bg);
            let text = if hoff > 0 {
                crate::ui::text::fit(
                    &crate::ui::text::slice_cells(line, hoff, text_w as usize),
                    text_w as usize,
                )
            } else {
                crate::ui::text::fit(line, text_w as usize)
            };
            buf.set_string(area.x, y, &text, st);

            if let Some((a, b)) = sel
                && (a.line..=b.line).contains(&li)
            {
                let from = if li == a.line { a.col } else { 0 };
                let to = if li == b.line {
                    b.col
                } else {
                    crate::ui::text::width(line) + 1
                };
                for col in from.max(hoff)..to {
                    let x = col - hoff;
                    if x >= text_w as usize {
                        break;
                    }
                    buf[(area.x + x as u16, y)].set_style(t.selection());
                }
            }
        }
        if self.scroll.overflows() {
            let body = Rect::new(area.x, area.y, area.width, rows_h);
            self.scroll
                .draw_with_fade(body, buf, ctx, self.id, focused, &[]);
        }
        if rows_h < area.height && self.h_enabled() {
            let y = area.y + rows_h;
            fill(
                buf,
                Rect::new(area.x, y, area.width, 1),
                Style::new().bg(bg),
            );
            let w = area
                .width
                .saturating_sub(u16::from(self.scroll.overflows()));
            let hb = Rect::new(area.x, y, w, 1);
            self.scroll.draw_h(hb, buf, ctx, self.id, focused);
        }
    }
}

impl ScrollPanel {
    fn rows(&self) -> &[String] {
        if self.wrap {
            &self.wrapped_cache.1
        } else {
            &self.lines
        }
    }

    fn source_of(&self, row: usize) -> usize {
        if self.wrap {
            self.wrapped_cache.2.get(row).copied().unwrap_or(row)
        } else {
            row
        }
    }

    fn pos_at(&self, pos: Position) -> Option<CellPos> {
        let rows = self.rows();
        let rows_h = (self.scroll.viewport_len as u16).min(self.area.height);
        if self.area.is_empty() || rows.is_empty() || rows_h == 0 {
            return None;
        }
        let y = pos.y.clamp(self.area.y, self.area.y + rows_h - 1) - self.area.y;
        let line = (y as usize + self.scroll.offset).min(rows.len() - 1);
        let text_w = self.area.width.saturating_sub(2).max(1);
        let x = pos.x.saturating_sub(self.area.x).min(text_w - 1) as usize;
        let hoff = if self.h_enabled() {
            self.scroll.h.offset
        } else {
            0
        };
        let col = (x + hoff).min(crate::ui::text::width(&rows[line]));
        Some(CellPos { line, col })
    }

    fn word_at(&self, p: CellPos) -> Option<(CellPos, CellPos)> {
        let row = self.rows().get(p.line)?;
        let cells = text_select::plain_cells(row);
        let at = text_select::plain_cell_at(&cells, p.col);
        let (s, e) = text_select::word_bounds(&cells, at, |c| text_select::is_word(c.g))?;
        let col = |i| text_select::plain_col_of(row, &cells, i);
        Some((
            CellPos {
                line: p.line,
                col: col(s),
            },
            CellPos {
                line: p.line,
                col: col(e),
            },
        ))
    }

    fn line_at(&self, p: CellPos) -> Option<(CellPos, CellPos)> {
        let rows = self.rows();
        rows.get(p.line)?;
        let src = self.source_of(p.line);
        let mut first = p.line;
        while first > 0 && self.source_of(first - 1) == src {
            first -= 1;
        }
        let mut last = p.line;
        while last + 1 < rows.len() && self.source_of(last + 1) == src {
            last += 1;
        }
        Some((
            CellPos {
                line: first,
                col: 0,
            },
            CellPos {
                line: last,
                col: crate::ui::text::width(&rows[last]),
            },
        ))
    }

    pub fn selection(&self) -> Option<(CellPos, CellPos)> {
        self.select.range()
    }

    pub fn selected_text(&self) -> Option<String> {
        let (a, b) = self.selection()?;
        let rows = self.rows();
        let mut out = String::new();
        let last = b.line.min(rows.len().saturating_sub(1));
        for (li, row) in rows.iter().enumerate().take(last + 1).skip(a.line) {
            if li > a.line {
                out.push(if self.source_of(li) == self.source_of(li - 1) {
                    ' '
                } else {
                    '\n'
                });
            }
            let from = if li == a.line { a.col } else { 0 };
            let to = if li == b.line { b.col } else { usize::MAX };
            out.push_str(text_select::plain_slice(row, from, to).trim_end_matches(' '));
        }
        Some(out)
    }

    pub fn select_all(&mut self) -> Outcome {
        let rows = self.rows();
        let Some(last) = rows.len().checked_sub(1) else {
            return Outcome::Consumed;
        };
        let all = (
            CellPos { line: 0, col: 0 },
            CellPos {
                line: last,
                col: crate::ui::text::width(&rows[last]),
            },
        );
        if self.select.sel == Some(all) {
            return Outcome::Consumed;
        }
        self.select.set(all.0, all.1);
        Outcome::Changed
    }

    pub fn clear_selection(&mut self) -> Outcome {
        self.select.clear()
    }

    fn on_press_with(&mut self, pos: Position, count: u8, mods: KeyModifiers) -> Outcome {
        let at = self.pos_at(pos);
        let word = at.and_then(|p| self.word_at(p));
        let line = at.and_then(|p| self.line_at(p));
        self.select
            .press(self.pointer_select, at, count, mods, word, line)
    }

    fn on_drag(&mut self, pos: Position) -> Outcome {
        if self.select.press.is_none() {
            return Outcome::Ignored;
        }
        let rows_h = (self.scroll.viewport_len as u16).min(self.area.height);
        let bottom = self.area.y + rows_h;
        if pos.y < self.area.y {
            self.scroll.scroll_by(-((self.area.y - pos.y) as isize));
            self.settle_follow();
        } else if pos.y >= bottom {
            self.scroll.scroll_by((pos.y + 1 - bottom) as isize);
            self.settle_follow();
        }
        let head = self.pos_at(pos);
        let word = if self.select.by_word() {
            head.and_then(|p| self.word_at(p))
        } else {
            None
        };
        self.select.drag(self.pointer_select, head, pos, word)
    }

    fn on_click_with(&mut self, pos: Position, count: u8) -> Outcome {
        let at = self.pos_at(pos);
        let word = at.and_then(|p| self.word_at(p));
        let line = at.and_then(|p| self.line_at(p));
        self.select
            .click(self.pointer_select, pos, count, word, line)
    }
}

impl Scrollable for ScrollPanel {
    fn pane(&mut self) -> &mut ScrollPane {
        &mut self.scroll
    }

    fn wheel_first(&mut self, axis: Axis, _delta: i32) -> Option<Response<()>> {
        (axis == Axis::Horizontal && !self.h_enabled()).then(Response::ignored)
    }

    fn after_scroll(&mut self, axis: Axis, _moved: bool) -> bool {
        if axis != Axis::Vertical {
            return false;
        }
        let was = self.follow;
        self.settle_follow();
        was != self.follow
    }
}

impl crate::ui::specimen::Specimen for Panel<'static> {
    fn specimen_id(&self) -> WidgetId {
        WidgetId::of("panel-specimen")
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        matches!(state, Rest | Focused | Overflow | Narrow)
    }

    fn specimen_height(&self, _state: crate::ui::specimen::SpecimenState) -> u16 {
        4
    }

    fn specimen(
        &self,
        state: crate::ui::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        _bg: Color,
    ) {
        use crate::ui::specimen::SpecimenState::*;
        let t = ctx.theme;
        let panel = Panel {
            title: self.title,
            kind: self.kind,
            focused: state == Focused,
            meta: if state == Overflow {
                Some("a meta far longer than the title row")
            } else {
                self.meta
            },
            badge: self.badge,
            bg_override: self.bg_override,
            focus_mark: self.focus_mark,
            title_min: self.title_min,
            meta_short: self.meta_short,
            pad_x: self.pad_x,
            heading: self.heading,
        };
        let inner = panel.render(area, buf, t);
        if !inner.is_empty() {
            let text = crate::ui::text::truncate("content", inner.width as usize);
            buf.set_string(inner.x, inner.y, text, t.secondary().bg(panel.bg(t)));
        }
    }
}

impl crate::ui::specimen::Specimen for ScrollPanel {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        matches!(state, Rest | Hover | Pressed | Empty | Overflow | Narrow)
    }

    fn specimen_height(&self, _state: crate::ui::specimen::SpecimenState) -> u16 {
        4
    }

    fn specimen(
        &self,
        state: crate::ui::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        use crate::ui::specimen::SpecimenState::*;
        let mut panel = self.clone();
        match state {
            Empty => panel.lines.clear(),

            Hover | Pressed | Overflow => {
                let n = panel.lines.len();
                panel
                    .lines
                    .extend((n + 1..=12.max(n)).map(|i| format!("line {i}")));
            }
            _ => {}
        }
        let bar = scrollbar::id_for(panel.id);
        match state {
            Hover => ctx.interaction.hover = Some(bar),
            Pressed => {
                ctx.interaction.hover = Some(bar);
                ctx.interaction.pressed = Some(bar);
            }
            _ => {}
        }
        panel.render(area, buf, ctx, bg, |t, _| t.secondary());
    }
}

impl crate::core::pointer::MouseTarget for ScrollPanel {
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
            Some(Phase::Press) => self.on_press_with(ev.pos(), ev.count(), ev.mods()).into(),
            Some(Phase::Drag) => self.on_drag(ev.pos()).into(),
            Some(Phase::Click) => self.on_click_with(ev.pos(), ev.count()).into(),
            _ => Response::ignored(),
        }
    }

    fn route(&mut self, ev: &crate::core::pointer::PointerEvent) -> Option<Response<()>> {
        edit_menu_route(self, ev)
    }
}

impl EditMenuHost for ScrollPanel {
    fn edit_menu(&mut self) -> &mut EditMenu {
        &mut self.edit_menu
    }
    fn edit_owner(&self) -> WidgetId {
        self.id
    }
    fn edit_caps(&self) -> EditCaps {
        EditCaps::read_only(
            self.select.range().is_some(),
            self.lines.iter().any(|l| !l.is_empty()),
        )
    }
    fn edit_shortcut(&self, cmd: crate::core::edit::TextCmd) -> Option<String> {
        use crate::core::edit::TextCmd;
        let cmd = match cmd {
            TextCmd::Copy => PanelCmd::Copy,
            TextCmd::SelectAll => PanelCmd::SelectAll,
            _ => return None,
        };
        crate::core::keymap::label_for(&self.keymap, cmd)
    }
    fn edit_target(&self, ev: &crate::core::pointer::PointerEvent) -> bool {
        ev.id == self.id
    }
    fn edit_run(&mut self, cmd: crate::core::edit::TextCmd) -> bool {
        use crate::core::edit::TextCmd;
        match cmd {
            TextCmd::SelectAll => {
                self.select_all();
            }
            TextCmd::Copy => {
                if let Some(t) = self.selected_text() {
                    crate::core::edit::clipboard::copy(&t);
                }
            }
            _ => {}
        }
        false
    }
}

#[cfg(test)]
mod saturation_tests {
    use ratatui::layout::Size;

    use super::Panel;

    /// A saturated intrinsic width reaches the panel as `u16::MAX`. Adding the
    /// border and padding must saturate too, not overflow.
    #[test]
    fn outer_saturates_a_saturated_content_width() {
        let card = Panel::card(Some("Trails")).outer(Size::new(u16::MAX, 0));
        assert_eq!(card.width, u16::MAX);
        let framed = Panel::framed(Some("Explorer")).outer(Size::new(u16::MAX, 0));
        assert_eq!(framed.width, u16::MAX);
        let tall = Panel::card(None).outer(Size::new(0, u16::MAX));
        assert_eq!(tall.height, u16::MAX);
    }
}

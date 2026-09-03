// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/app.rs (MIT).

//! Catalog application shell: layout, event routing, navigation, inspector.

use std::ops::ControlFlow;
use std::time::Duration;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use termrock::input::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use termrock::interaction::{
    InteractionElement, InteractionLayer, InteractionScene, LayerDismissPolicy, LayerKind,
    SemanticRole,
};
use termrock::runtime::FrameTick;
use termrock::style::{BadgeKind, ColorCapability, DesignSystem, JunieTheme};

use crate::catalog::{CatalogProfile, NavEntry, PageId, nav_entries};
use crate::ctx::{Interaction, RenderCtx};
use crate::draw::fill;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent, Request};
use crate::pages;
use crate::text;

pub const MIN_WIDTH: u16 = 72;
pub const MIN_HEIGHT: u16 = 20;

pub const NAV: WidgetId = WidgetId::of("app.nav");
pub const HEADER_HELP: WidgetId = WidgetId::of("app.header.help");
pub const HEADER_INSPECT: WidgetId = WidgetId::of("app.header.inspect");
pub const HELP_DIALOG: WidgetId = WidgetId::of("app.help");
pub const HELP_CLOSE: WidgetId = WidgetId::of("app.help.close");

const HELP_TEXT: &str = "Tab / Shift+Tab   move keyboard focus\n\
                         ↑ ↓ ← →           move inside the focused control\n\
                         Enter / Space     activate · start editing\n\
                         Esc               cancel editing · back to navigation\n\
                         [ ]               previous / next page\n\
                         0                 jump to navigation\n\
                         i                 toggle state inspector\n\
                         q                 quit\n\n\
                         Mouse: hover to preview, click to focus and activate, wheel to scroll, drag the scrollbar thumb.";

pub use crate::ctx::LayerId;

/// Live metadata for the currently mounted catalog page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageMetadata {
    pub title: &'static str,
    pub description: &'static str,
    pub interactive: bool,
    pub interaction_kind: &'static str,
    pub captures_text_input: bool,
    pub hints: Vec<Hint>,
    pub animating: bool,
}

#[derive(Debug, Default, Clone, Copy)]
struct ShellLayout {
    header: Rect,
    sidebar: Rect,
    main: Rect,
    inspector: Rect,
    footer: Rect,
    too_small: bool,
}

/// Canonical catalog application.
pub struct App {
    pub profile: CatalogProfile,
    pub theme: JunieTheme,
    pub system: DesignSystem,
    pages: Vec<Box<dyn Page>>,
    pub page: PageId,
    pub nav_cursor: usize,
    pub focus: Option<WidgetId>,
    pub scene: InteractionScene<WidgetId, LayerId, ()>,
    pub hover: Option<WidgetId>,
    pub pressed: Option<WidgetId>,
    pub mouse: Option<Position>,
    pub hover_suppressed: bool,
    pub help_open: bool,
    pub inspector: bool,
    pub size: (u16, u16),
    pub tick: u64,
    pub elapsed_ms: u64,
    pub last_key: Option<String>,
    pub status: Option<(String, u64)>,
    pub flash: Option<(WidgetId, u64)>,
    pub quit: bool,
    /// Hardware cursor from the last `render`. None = hidden.
    pub last_cursor: Option<Position>,
    layout: ShellLayout,
    saved_focus: Option<WidgetId>,
    scroll_hits: Vec<(WidgetId, Rect)>,
}

impl App {
    /// Build the catalog for a profile and colour capability.
    #[must_use]
    pub fn new(profile: CatalogProfile, level: ColorCapability) -> Self {
        let theme = JunieTheme::for_level(level);
        let system = DesignSystem::junie().capability(level);
        let nav = nav_entries(profile);
        let pages: Vec<Box<dyn Page>> = nav.iter().map(|e| pages::mount(e.id)).collect();
        let mut scene = InteractionScene::new();
        scene.ensure_root(root_layer());
        scene.focus(NAV);
        Self {
            profile,
            theme,
            system,
            pages,
            page: PageId::OVERVIEW,
            nav_cursor: 0,
            focus: Some(NAV),
            scene,
            hover: None,
            pressed: None,
            mouse: None,
            hover_suppressed: false,
            help_open: false,
            inspector: false,
            size: (0, 0),
            tick: 0,
            elapsed_ms: 0,
            last_key: None,
            status: None,
            flash: None,
            quit: false,
            last_cursor: None,
            layout: ShellLayout::default(),
            saved_focus: None,
            scroll_hits: Vec::new(),
        }
    }

    #[must_use]
    pub fn nav(&self) -> &'static [NavEntry] {
        nav_entries(self.profile)
    }

    /// Read metadata from the mounted page implementation.
    #[must_use]
    pub fn page_metadata(&self) -> PageMetadata {
        let page = &self.pages[self.page.index(self.nav())];
        PageMetadata {
            title: page.title(),
            description: page.blurb(),
            interactive: page.interactive(),
            interaction_kind: page.interaction_kind(),
            captures_text_input: page.captures_text_input(),
            hints: page.hints(self.focus),
            animating: page.animating(),
        }
    }

    pub fn goto(&mut self, page: PageId) {
        if self.page != page {
            self.page = page;
            self.nav_cursor = page.index(self.nav());
            if self.focus != Some(NAV) {
                self.focus = None;
            }
        }
    }

    #[must_use]
    pub fn animating(&self) -> bool {
        let page = &self.pages[self.page.index(self.nav())];
        page.animating() || self.flash.is_some()
    }

    #[must_use]
    pub fn tick_interval(&self) -> Duration {
        if self.animating() {
            Duration::from_millis(80)
        } else {
            Duration::from_millis(400)
        }
    }

    #[must_use]
    pub fn next_deadline(&self) -> Option<std::time::Instant> {
        if self.animating() || self.status.is_some() {
            Some(std::time::Instant::now() + self.tick_interval())
        } else {
            None
        }
    }

    fn interaction(&self) -> Interaction {
        let flash = match self.flash {
            Some((id, at)) if self.elapsed_ms.saturating_sub(at) < 140 => Some(id),
            _ => None,
        };
        Interaction {
            focus: self.focus,
            hover: self.hover,
            pressed: self.pressed,
            flash,
            focus_hidden: false,
            hover_suppressed: self.hover_suppressed,
            tick: self.tick,
        }
    }

    fn set_status(&mut self, s: String) {
        self.status = Some((s, self.elapsed_ms));
    }

    /// Drive one input event. Returns Break when the app should exit.
    pub fn handle_event(&mut self, event: Event, tick: FrameTick) -> ControlFlow<()> {
        self.elapsed_ms = u64::try_from(tick.elapsed().as_millis()).unwrap_or(u64::MAX);
        match event {
            Event::Resize { width, height } => {
                self.size = (width, height);
            }
            Event::Paste(text) => {
                if self.help_open {
                    return ControlFlow::Continue(());
                }
                self.dispatch(PageEvent::Paste(text));
            }
            Event::Key(key) => {
                if key.kind == KeyEventKind::Release {
                    return ControlFlow::Continue(());
                }
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.quit = true;
                    return ControlFlow::Break(());
                }
                self.last_key = Some(describe_key(&key));
                self.hover_suppressed = true;
                self.on_key(key);
            }
            Event::Mouse(m) => {
                self.on_mouse(m);
            }
            Event::FocusGained | Event::FocusLost | Event::Unknown => {}
            _ => {}
        }
        if self.quit {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }

    /// Advance animation / status clocks.
    pub fn on_tick(&mut self, tick: FrameTick) {
        self.elapsed_ms = u64::try_from(tick.elapsed().as_millis()).unwrap_or(u64::MAX);
        if self.animating() {
            self.tick = self.tick.wrapping_add(1);
        }
        if let Some((_, at)) = self.flash
            && self.elapsed_ms.saturating_sub(at) >= 140
        {
            self.flash = None;
        }
        if let Some((_, at)) = &self.status
            && self.elapsed_ms.saturating_sub(*at) > 4000
        {
            self.status = None;
        }
        if !self.help_open {
            self.dispatch(PageEvent::Tick);
        }
    }

    fn dispatch(&mut self, ev: PageEvent) -> Route {
        let i = self.page.index(nav_entries(self.profile));
        let mut cx = PageCtx {
            focus: &mut self.focus,
            requests: Vec::new(),
        };
        let out = self.pages[i].handle(&ev, &mut cx);
        let requests = std::mem::take(&mut cx.requests);
        for r in requests {
            match r {
                Request::Status(s) => self.set_status(s),
                Request::FocusNext => self.focus_next(),
                Request::FocusPrev => self.focus_prev(),
            }
        }
        out
    }

    fn on_key(&mut self, key: KeyEvent) -> Route {
        if self.layout.too_small {
            if is_char(&key, 'q') {
                self.quit = true;
            }
            return Route::Consumed;
        }
        if self.help_open {
            if key.code == KeyCode::Esc
                || key.code == KeyCode::Enter
                || is_char(&key, ' ')
                || is_char(&key, 'q')
            {
                self.close_help();
            }
            return Route::Changed;
        }
        if self.focus == Some(NAV) {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') if plain(&key) => {
                    self.nav_cursor = self.nav_cursor.saturating_sub(1);
                    return Route::Changed;
                }
                KeyCode::Down | KeyCode::Char('j') if plain(&key) => {
                    self.nav_cursor = (self.nav_cursor + 1).min(self.nav().len() - 1);
                    return Route::Changed;
                }
                KeyCode::Home | KeyCode::Char('g') if plain(&key) => {
                    self.nav_cursor = 0;
                    return Route::Changed;
                }
                KeyCode::End | KeyCode::Char('G') => {
                    self.nav_cursor = self.nav().len() - 1;
                    return Route::Changed;
                }
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right | KeyCode::Char('l')
                    if plain(&key) || key.code == KeyCode::Enter =>
                {
                    let target = self.nav()[self.nav_cursor].id;
                    self.goto(target);
                    if matches!(key.code, KeyCode::Right | KeyCode::Char('l')) {
                        self.focus = None;
                        self.focus_next();
                        if self.focus == Some(NAV) {
                            self.focus_next();
                        }
                    }
                    return Route::Changed;
                }
                _ => {}
            }
        }
        let activating = matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) && plain(&key);
        let was_editing = self.pages[self.page.index(self.nav())].editing();
        let out = self.dispatch(PageEvent::Key(key));
        if out.consumed() {
            if activating
                && !was_editing
                && out == Route::Changed
                && let Some(f) = self.focus
            {
                self.flash(f);
            }
            return out;
        }
        match key.code {
            KeyCode::Tab
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    || key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                self.focus_prev();
                Route::Changed
            }
            KeyCode::BackTab => {
                self.focus_prev();
                Route::Changed
            }
            KeyCode::Tab => {
                self.focus_next();
                Route::Changed
            }
            KeyCode::Char('q') if plain(&key) => {
                self.quit = true;
                Route::Consumed
            }
            KeyCode::Char('?') => {
                self.open_help();
                Route::Changed
            }
            KeyCode::Char('i') if plain(&key) => {
                self.inspector = !self.inspector;
                Route::Changed
            }
            KeyCode::Char(']') => {
                let next = (self.page.index(self.nav()) + 1) % self.nav().len();
                self.goto(self.nav()[next].id);
                Route::Changed
            }
            KeyCode::Char('[') => {
                let n = self.nav().len();
                let prev = (self.page.index(self.nav()) + n - 1) % n;
                self.goto(self.nav()[prev].id);
                Route::Changed
            }
            KeyCode::Char('0') if plain(&key) => {
                self.focus = Some(NAV);
                Route::Changed
            }
            KeyCode::Esc => {
                if self.focus != Some(NAV) {
                    self.focus = Some(NAV);
                    Route::Changed
                } else {
                    Route::Consumed
                }
            }
            _ => Route::Ignored,
        }
    }

    fn open_help(&mut self) {
        self.saved_focus = self.focus;
        self.focus = Some(HELP_CLOSE);
        self.help_open = true;
        self.hover = None;
        self.pressed = None;
    }

    fn close_help(&mut self) {
        self.help_open = false;
        self.focus = self.saved_focus.take();
    }

    fn focus_next(&mut self) {
        let order = self.focus_ids();
        if order.is_empty() {
            return;
        }
        let next = match self
            .focus
            .and_then(|f| order.iter().position(|&id| id == f))
        {
            Some(i) => order[(i + 1) % order.len()],
            None => order[0],
        };
        self.focus = Some(next);
    }

    fn focus_prev(&mut self) {
        let order = self.focus_ids();
        if order.is_empty() {
            return;
        }
        let prev = match self
            .focus
            .and_then(|f| order.iter().position(|&id| id == f))
        {
            Some(0) | None => *order.last().unwrap(),
            Some(i) => order[i - 1],
        };
        self.focus = Some(prev);
    }

    fn focus_ids(&self) -> Vec<WidgetId> {
        self.scene.focus_order().into_iter().copied().collect()
    }

    fn on_mouse(&mut self, m: MouseEvent) -> Route {
        self.mouse = Some(m.position);
        match m.kind {
            MouseEventKind::Moved => {
                let was = self.hover;
                let suppressed = self.hover_suppressed;
                self.hover_suppressed = false;
                self.hover = self.hit(m.position);
                if self.hover != was || suppressed {
                    Route::Changed
                } else {
                    Route::Ignored
                }
            }
            MouseEventKind::Drag(_) => {
                self.hover = self.hit(m.position);
                if let Some(pressed) = self.pressed {
                    if self.help_open {
                        return Route::Consumed;
                    }
                    return self.dispatch(PageEvent::Drag {
                        pressed,
                        pos: m.position,
                    });
                }
                Route::Ignored
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let hit = self.hit(m.position);
                self.pressed = hit;
                self.hover = hit;
                let Some(id) = hit else {
                    if self.help_open {
                        return Route::Consumed;
                    }
                    return Route::Ignored;
                };
                if self.help_open {
                    return Route::Changed;
                }
                if let Some(i) = self.nav_index_at(id) {
                    self.focus = Some(NAV);
                    self.nav_cursor = i;
                    return Route::Changed;
                }
                if self.scene.get(&id).is_some_and(|e| e.focusable) {
                    self.focus = Some(id);
                }
                Route::Changed
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let hit = self.hit(m.position);
                let pressed = self.pressed.take();
                let Some(id) = hit else {
                    if self.help_open && pressed.is_none() {
                        self.close_help();
                    }
                    return Route::Changed;
                };
                if pressed != Some(id) {
                    return Route::Changed;
                }
                if self.help_open {
                    if id == HELP_CLOSE {
                        self.close_help();
                    }
                    return Route::Changed;
                }
                if id == HEADER_HELP {
                    self.open_help();
                    return Route::Changed;
                }
                if id == HEADER_INSPECT {
                    self.inspector = !self.inspector;
                    return Route::Changed;
                }
                if let Some(i) = self.nav_index_at(id) {
                    self.nav_cursor = i;
                    self.goto(self.nav()[i].id);
                    self.focus = Some(NAV);
                    return Route::Changed;
                }
                self.flash(id);
                self.dispatch(PageEvent::Click {
                    id,
                    pos: m.position,
                })
                .or(Route::Changed)
            }
            MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight => Route::Ignored,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let delta = if m.kind == MouseEventKind::ScrollUp {
                    -3
                } else {
                    3
                };
                if self.help_open {
                    return Route::Consumed;
                }
                let Some(id) = self.hit_scroll(m.position) else {
                    return Route::Ignored;
                };
                if id == NAV || self.nav_index_at(id).is_some() {
                    return Route::Consumed;
                }
                self.dispatch(PageEvent::Wheel { id, delta })
            }
            _ => Route::Ignored,
        }
    }

    fn hit(&self, pos: Position) -> Option<WidgetId> {
        self.scene.hit_test(pos).map(|e| e.id)
    }

    fn hit_scroll(&self, pos: Position) -> Option<WidgetId> {
        self.scroll_hits
            .iter()
            .rev()
            .find(|(_, r)| r.contains(pos))
            .map(|(id, _)| *id)
            .or_else(|| self.hit(pos))
    }

    fn nav_index_at(&self, id: WidgetId) -> Option<usize> {
        (0..self.nav().len()).find(|&i| NAV.child(i) == id)
    }

    pub fn flash(&mut self, id: WidgetId) {
        self.flash = Some((id, self.elapsed_ms));
    }

    /// Render one frame into the terminal.
    pub fn render(&mut self, frame: &mut Frame<'_>, tick: FrameTick) {
        self.elapsed_ms = u64::try_from(tick.elapsed().as_millis()).unwrap_or(self.elapsed_ms);
        let area = frame.area();
        self.size = (area.width, area.height);
        self.scene.begin_frame();
        self.scene.ensure_root(root_layer());
        if self.help_open {
            self.scene.push_layer(dialog_layer());
        }
        self.scroll_hits.clear();
        let mut scene = std::mem::take(&mut self.scene);
        let mut scroll_hits = std::mem::take(&mut self.scroll_hits);
        let theme = self.theme;
        let system = self.system.clone();
        let interaction = self.interaction();
        let cursor;
        {
            let buf = frame.buffer_mut();
            let layer = if self.help_open {
                LayerId::Dialog
            } else {
                LayerId::Root
            };
            let mut ctx = RenderCtx {
                theme: &theme,
                system: &system,
                interaction,
                scene: &mut scene,
                layer,
                cursor: None,
                inert: false,
                scroll_hits: &mut scroll_hits,
            };
            self.draw(area, buf, &mut ctx);
            cursor = ctx.cursor;
        }
        self.scene = scene;
        self.scroll_hits = scroll_hits;
        self.scene.reconcile();
        if !self.help_open {
            if !self.layout.too_small
                && !self
                    .focus
                    .is_some_and(|c| self.scene.focus_order().iter().any(|id| **id == c))
            {
                self.focus = self.scene.focus_order().first().copied().copied();
            }
        }
        if let Some(id) = self.focus {
            let _ = self.scene.focus(id);
        }
        self.last_cursor = cursor;
        if let Some(pos) = cursor {
            frame.set_cursor_position(pos);
        }
    }

    fn compute_layout(&self, area: Rect) -> ShellLayout {
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            return ShellLayout {
                too_small: true,
                ..Default::default()
            };
        }
        let header = Rect::new(area.x, area.y, area.width, 1);
        let footer = Rect::new(area.x, area.bottom() - 1, area.width, 1);
        let body = Rect::new(
            area.x,
            area.y + 2,
            area.width,
            area.height.saturating_sub(4),
        );
        let wide = area.width >= 110;
        let sidebar_w = if wide { 24 } else { 19 };
        let inspector_w = if self.inspector && area.width >= 100 {
            30
        } else {
            0
        };
        let sidebar = Rect::new(body.x, body.y, sidebar_w, body.height);
        let main_x = body.x + sidebar_w + 2;
        let main_w = body
            .width
            .saturating_sub(sidebar_w + 2 + inspector_w + if inspector_w > 0 { 2 } else { 0 });
        let main = Rect::new(main_x, body.y, main_w, body.height);
        let inspector = if inspector_w > 0 {
            Rect::new(main.right() + 2, body.y, inspector_w, body.height)
        } else {
            Rect::ZERO
        };
        ShellLayout {
            header,
            sidebar,
            main,
            inspector,
            footer,
            too_small: false,
        }
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = self.theme;
        fill(buf, area, t.base());
        let layout = self.compute_layout(area);
        self.layout = layout;
        if layout.too_small {
            self.draw_too_small(area, buf);
            return;
        }
        let page_inert = self.help_open;
        ctx.inert = false;
        ctx.layer = LayerId::Root;
        self.draw_header(layout.header, buf, ctx);
        self.draw_sidebar(layout.sidebar, buf, ctx);
        ctx.inert = page_inert;
        self.draw_main(layout.main, buf, ctx);
        if !layout.inspector.is_empty() {
            self.draw_inspector(layout.inspector, buf, ctx);
        }
        ctx.inert = false;
        self.draw_footer(layout.footer, buf, ctx);
        if self.help_open {
            ctx.inert = false;
            ctx.layer = LayerId::Dialog;
            self.draw_help(area, buf, ctx);
        }
    }

    fn draw_too_small(&self, area: Rect, buf: &mut Buffer) {
        let t = self.theme;
        let ident = self.profile.identity();
        let title = ident.too_small_title();
        let size = format!(
            "Need {MIN_WIDTH}×{MIN_HEIGHT}, have {}×{}",
            area.width, area.height
        );
        let lines = [
            (title.as_str(), t.title()),
            ("Terminal too small", t.secondary()),
            (size.as_str(), t.muted()),
            ("q Quit", t.faint()),
        ];
        let y0 = area.y + area.height.saturating_sub(5) / 2;
        for (i, (text, style)) in lines.iter().enumerate() {
            let w = crate::text::width(text) as u16;
            let x = area.x + area.width.saturating_sub(w) / 2;
            let y = y0 + i as u16 + if i == 3 { 1 } else { 0 };
            if y < area.bottom() {
                buf.set_string(x, y, text, style.bg(t.canvas));
            }
        }
    }

    fn draw_header(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = self.theme;
        let ident = self.profile.identity();
        let mut x = area.x + 1;
        buf.set_string(x, area.y, ident.mark, t.accent_fg());
        x += 2;
        buf.set_string(x, area.y, ident.name, t.title());
        x += text::width(ident.name) as u16 + 1;
        buf.set_string(x, area.y, ident.product, t.secondary());
        x += text::width(ident.product) as u16 + 1;
        let entry = &self.nav()[self.page.index(self.nav())];
        let crumb = format!("/ {} / {}", entry.section, entry.label);
        buf.set_string(x, area.y, &crumb, t.muted());
        let crumb_w = text::width(&crumb) as u16;
        let cap = format!("{} · {}×{}", color_label(t.level), self.size.0, self.size.1);
        let mut rx = area.right().saturating_sub(1);
        let help = " ? Help ";
        let insp = if self.inspector {
            " i Inspector · on "
        } else {
            " i Inspector "
        };
        for (label, id) in [(help, HEADER_HELP), (insp, HEADER_INSPECT)] {
            let w = text::width(label) as u16;
            rx = rx.saturating_sub(w);
            let hovered = ctx.interaction.hovered(id);
            let style = if hovered {
                t.primary().bg(t.surface)
            } else {
                t.muted()
            };
            buf.set_string(rx, area.y, label, style);
            ctx.clickable(id, Rect::new(rx, area.y, w, 1));
            rx = rx.saturating_sub(1);
        }
        let cw = text::width(&cap) as u16;
        if rx > x + crumb_w + cw + 2 {
            buf.set_string(rx.saturating_sub(cw + 1), area.y, &cap, t.faint());
        }
    }

    fn draw_sidebar(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = self.theme;
        let focused = ctx.interaction.focused(NAV);
        let mut y = area.y;
        let mut section = "";
        let nav = self.nav();
        let sections = nav
            .iter()
            .fold(Vec::new(), |mut sections: Vec<&str>, entry| {
                if !sections.contains(&entry.section) {
                    sections.push(entry.section);
                }
                sections
            })
            .len() as u16;
        let compact = area.height < nav.len() as u16 + sections * 2 - 1;
        for (i, e) in nav.iter().enumerate() {
            if e.section != section && !compact {
                if y > area.y {
                    y += 1;
                }
                if y >= area.bottom() {
                    break;
                }
                buf.set_string(area.x + 3, y, e.section, t.faint());
                section = e.section;
                y += 1;
            } else if e.section != section && compact && y > area.y {
                section = e.section;
            }
            if y >= area.bottom() {
                break;
            }
            let row = Rect::new(area.x, y, area.width, 1);
            let rid = NAV.child(i);
            let mut s = ctx.state(rid);
            s.focused = focused && i == self.nav_cursor;
            let current = e.id == self.page;
            let st = t.row(s, t.canvas);
            fill(buf, row, st);
            buf.set_string(row.x, y, "▎", t.gutter(s, st.bg.unwrap_or(t.canvas), false));
            if current {
                let ms = st.fg(t.accent);
                buf.set_string(row.x + 1, y, "›", ms);
            }
            let label_style = if current || s.focused || s.hovered {
                st.fg(t.text_primary)
            } else {
                st.fg(t.text_secondary)
            };
            buf.set_string(
                row.x + 3,
                y,
                &text::fit(e.label, area.width.saturating_sub(4) as usize),
                label_style,
            );
            ctx.clickable(rid, row);
            y += 1;
        }
        if !ctx.inert {
            let _ = ctx.scene.register(
                InteractionElement::control(NAV, LayerId::Root, area)
                    .role(SemanticRole::List)
                    .focusable(true),
            );
            ctx.scrollable(NAV, area);
        }
    }

    fn draw_main(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = self.theme;
        let i = self.page.index(self.nav());
        let page = self.pages[i].as_mut();
        buf.set_string(area.x, area.y, page.title(), t.title());
        let tw = text::width(page.title()) as u16;
        if area.width > tw + 4 {
            let blurb = text::truncate(page.blurb(), (area.width - tw - 3) as usize);
            buf.set_string(area.x + tw + 2, area.y, &blurb, t.muted());
        }
        let body = Rect::new(
            area.x,
            area.y + 2,
            area.width,
            area.height.saturating_sub(2),
        );
        page.render(body, buf, ctx);
    }

    fn draw_inspector(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = self.theme;
        let (inner, bg) = layout::card(area, buf, &t, Some("State"), None, false);
        let page = &self.pages[self.page.index(self.nav())];
        let mode = if self.help_open {
            "MODAL"
        } else if page.editing() {
            "EDIT"
        } else {
            "NAV"
        };
        let fmt_rect = |r: Option<Rect>| match r {
            Some(r) => format!("{}·{} {}×{}", r.x, r.y, r.width, r.height),
            None => "—".to_owned(),
        };
        let focus_area = self.focus.and_then(|f| self.scene.get(&f).map(|e| e.area));
        let hover_area = self.hover.and_then(|h| self.scene.get(&h).map(|e| e.area));
        let rows: Vec<(&str, String)> = vec![
            ("mode", mode.to_owned()),
            ("focus", fmt_rect(focus_area)),
            (
                "hover",
                if self.hover_suppressed {
                    "suppressed".into()
                } else {
                    fmt_rect(hover_area)
                },
            ),
            (
                "pressed",
                fmt_rect(
                    self.pressed
                        .and_then(|p| self.scene.get(&p).map(|e| e.area)),
                ),
            ),
            (
                "mouse",
                self.mouse
                    .map(|p| format!("{}·{}", p.x, p.y))
                    .unwrap_or("—".into()),
            ),
            ("last key", self.last_key.clone().unwrap_or("—".into())),
            (
                "focus ring",
                format!("{} stops", self.scene.focus_order().len()),
            ),
            ("hit regions", format!("{}", self.scene.elements().len())),
            ("tick", format!("{}", self.tick)),
            ("colors", color_label(t.level).to_owned()),
        ];
        for (i, (k, v)) in rows.iter().enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.bottom() {
                break;
            }
            buf.set_string(inner.x, y, format!("{k:<11}"), t.muted().bg(bg));
            let vs = t.primary().bg(bg);
            buf.set_string(
                inner.x + 12,
                y,
                &text::truncate(v, inner.width.saturating_sub(12) as usize),
                vs,
            );
        }
        let _ = ctx;
    }

    fn draw_footer(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = self.theme;
        let page = &self.pages[self.page.index(self.nav())];
        let mut x = area.x + 1;
        let mut hints: Vec<(String, String)> = Vec::new();
        if self.help_open {
            hints.push(("Enter".into(), "Confirm".into()));
            hints.push(("Esc".into(), "Cancel".into()));
        } else if page.overlaying() {
            // Keep the dialog footer; only the status sentence is ours.
        } else if self.focus == Some(NAV) {
            hints.push(("↑ ↓".into(), "Move".into()));
            hints.push(("Enter".into(), "Open".into()));
            hints.push(("Tab".into(), "Into page".into()));
            hints.push(("q".into(), "Quit".into()));
        } else {
            for (k, v) in page.hints(self.focus) {
                hints.push((k.into(), v.into()));
            }
            if !page.editing() {
                hints.push(("Tab".into(), "Next".into()));
            }
        }
        if page.editing() && !self.help_open {
            let badge = " EDIT ";
            buf.set_string(x, area.y, badge, t.badge(BadgeKind::Edit));
            x += badge.len() as u16 + 2;
        }
        let right_reserved = self
            .status
            .as_ref()
            .map(|(s, _)| text::width(s) as u16)
            .unwrap_or(14);
        for (k, v) in &hints {
            let kw = text::width(k) as u16;
            let w = kw + 1 + text::width(v) as u16 + 2;
            if x + w + right_reserved > area.right() {
                break;
            }
            buf.set_string(x, area.y, k, t.key_hint_key());
            buf.set_string(x + kw + 1, area.y, v, t.key_hint_action());
            x += w;
        }
        if let Some((s, _)) = &self.status {
            let w = text::width(s) as u16;
            if area.right() > w + 1 {
                buf.set_string(area.right() - w - 1, area.y, s, t.secondary());
            }
        }
        let _ = ctx;
    }

    fn draw_help(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = self.theme;
        // Backdrop: every cell except the footer row.
        let dim = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(1));
        for y in dim.y..dim.bottom() {
            for x in dim.x..dim.right() {
                if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                    let st = ratatui::style::Style::new()
                        .fg(cell.fg)
                        .bg(cell.bg)
                        .add_modifier(cell.modifier);
                    cell.set_style(t.backdrop(st));
                }
            }
        }
        let width = 70u16.min(area.width.saturating_sub(4)).max(20);
        let lines: Vec<&str> = HELP_TEXT.lines().collect();
        let height = (lines.len() as u16 + 6).min(area.height.saturating_sub(2));
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;
        let frame = Rect::new(x, y, width, height);
        let (inner, bg) = layout::framed(frame, buf, &t, Some("Keyboard & mouse"), true);
        let mut ly = inner.y;
        for line in lines {
            if ly >= inner.bottom().saturating_sub(2) {
                break;
            }
            buf.set_string(inner.x, ly, line, t.primary().bg(bg));
            ly += 1;
        }
        let close = " Close ";
        let cw = text::width(close) as u16;
        let bx = inner.right().saturating_sub(cw);
        let by = inner.bottom().saturating_sub(1);
        let style = t.button(
            termrock::style::ButtonKind::Secondary,
            ctx.state(HELP_CLOSE),
            bg,
        );
        let gs = t.gutter(ctx.state(HELP_CLOSE), style.bg.unwrap_or(bg), false);
        if by < inner.bottom() && bx >= inner.x {
            buf.set_string(bx, by, "▎", gs);
            buf.set_string(bx + 1, by, "Close ", style);
            ctx.control(HELP_CLOSE, Rect::new(bx, by, cw, 1), false);
        }
    }
}

fn root_layer() -> InteractionLayer<LayerId, WidgetId> {
    InteractionLayer {
        id: LayerId::Root,
        kind: LayerKind::Root,
        owns_input: true,
        esc: LayerDismissPolicy::Ignore,
        outside: LayerDismissPolicy::Ignore,
        focus_return: None,
    }
}

fn dialog_layer() -> InteractionLayer<LayerId, WidgetId> {
    InteractionLayer {
        id: LayerId::Dialog,
        kind: LayerKind::Card,
        owns_input: true,
        esc: LayerDismissPolicy::Dismissible,
        outside: LayerDismissPolicy::Dismissible,
        focus_return: None,
    }
}

fn color_label(level: ColorCapability) -> &'static str {
    // Source ColorLevel::label — not TermRock ColorCapability::label.
    match level {
        ColorCapability::Truecolor => "truecolor",
        ColorCapability::Indexed256 => "256 colors",
        ColorCapability::Ansi16 => "16 colors",
        ColorCapability::Monochrome => "no color",
        _ => "truecolor",
    }
}

fn plain(key: &KeyEvent) -> bool {
    key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT
}

fn is_char(key: &KeyEvent, c: char) -> bool {
    key.code == KeyCode::Char(c) && plain(key)
}

fn describe_key(k: &KeyEvent) -> String {
    let mut s = String::new();
    if k.modifiers.contains(KeyModifiers::CONTROL) {
        s.push_str("Ctrl+");
    }
    if k.modifiers.contains(KeyModifiers::ALT) {
        s.push_str("Alt+");
    }
    if k.modifiers.contains(KeyModifiers::SHIFT) && !matches!(k.code, KeyCode::Char(_)) {
        s.push_str("Shift+");
    }
    match k.code {
        KeyCode::Char(' ') => s.push_str("Space"),
        KeyCode::Char(c) => s.push(c),
        KeyCode::BackTab => s.push_str("Shift+Tab"),
        other => s.push_str(&format!("{other:?}")),
    }
    s
}

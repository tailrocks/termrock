// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/sidebars.rs (MIT).

//! Sections, current item, focus cursor, hover, collapsed mode.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use termrock::input::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use termrock::widgets::{
    Button, ButtonState, ButtonVariant, NavItem, NavigationList, NavigationListOutcome,
    NavigationListState,
};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};
use crate::text;

const ID: WidgetId = WidgetId::of("sidebars");
const NAV: WidgetId = ID.sub("nav");
const COLLAPSE: WidgetId = ID.sub("collapse");

fn nav_items() -> Vec<NavItem<&'static str>> {
    vec![
        NavItem::section("sec-workspace", "Workspace"),
        NavItem::new("tasks", "Tasks").icon("T").badge("3"),
        NavItem::new("runs", "Runs").icon("R"),
        NavItem::new("branches", "Branches").icon("B"),
        NavItem::section("sec-project", "Project"),
        NavItem::new("members", "Members").icon("M"),
        NavItem::new("environment", "Environment").icon("E"),
        NavItem::new("billing", "Billing").icon("$").enabled(false),
        NavItem::section("sec-preferences", "Preferences"),
        NavItem::new("keyboard", "Keyboard").icon("K"),
        NavItem::new("appearance", "Appearance").icon("A"),
    ]
}

fn label_of(id: &str) -> &'static str {
    nav_items()
        .into_iter()
        .find(|i| i.id == id)
        .map(|i| match i.id {
            "tasks" => "Tasks",
            "runs" => "Runs",
            "branches" => "Branches",
            "members" => "Members",
            "environment" => "Environment",
            "billing" => "Billing",
            "keyboard" => "Keyboard",
            "appearance" => "Appearance",
            _ => "Tasks",
        })
        .unwrap_or("Tasks")
}

pub struct SidebarsPage {
    nav: NavigationListState<&'static str>,
    collapse: ButtonState,
    collapsed: bool,
    capture_cursor: Option<Position>,
    nav_focus_initialized: bool,
}

impl SidebarsPage {
    #[must_use]
    pub fn new() -> Self {
        let mut nav = NavigationListState::new(Some("tasks"));
        nav.set_route_and_focus("tasks");
        Self {
            nav,
            collapse: ButtonState::new(),
            collapsed: false,
            capture_cursor: None,
            nav_focus_initialized: false,
        }
    }
}

impl Page for SidebarsPage {
    fn title(&self) -> &'static str {
        "Sidebars"
    }
    fn blurb(&self) -> &'static str {
        "Sections, current item, focus cursor, hover, collapsed mode; text first, no icons"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let w = if self.collapsed { 6 } else { 24 } + 4;
        let side = Rect::new(area.x, area.y, w, area.height.min(20));
        let (inner, bg) = layout::card(side, buf, t, None, None, false);
        // Source untitled card inner is `(side.x, side.y+1, side.width, h-2)`:
        // nav origin is the card edge, not the +2 caption inset.
        let nav_area = Rect::new(side.x, inner.y, side.width, inner.height.saturating_sub(2));
        let items = nav_items();
        let nav_focused = ctx.interaction.focused(NAV);
        self.nav.set_focused(nav_focused);
        if nav_focused && !self.nav_focus_initialized {
            self.nav_focus_initialized = true;
            let _ = self
                .nav
                .handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &items);
        }
        NavigationList::new(&items, ctx.system)
            .rail(self.collapsed)
            .show_filter(false)
            .paint(nav_area, buf, &mut self.nav);
        ctx.control(NAV, nav_area, false);
        self.capture_cursor =
            nav_focused.then(|| Position::new(side.right(), nav_area.y.saturating_add(2)));

        let collapse_label = if self.collapsed { "›" } else { "Collapse" };
        self.collapse.focused = ctx.interaction.focused(COLLAPSE);
        self.collapse.hovered = ctx.interaction.hovered(COLLAPSE);
        self.collapse.activation.set_accepts_input(true);
        let br = Rect::new(side.x + 1, inner.bottom().saturating_sub(1), side.width, 1);
        let _ = Button::new(collapse_label, ctx.system)
            .variant(ButtonVariant::Secondary)
            .container(bg)
            .paint(br, buf, &mut self.collapse);
        ctx.control(COLLAPSE, br, false);

        let content = Rect::new(
            side.right() + 2,
            area.y,
            area.width.saturating_sub(w + 2),
            area.height,
        );
        let current = self.nav.route().copied().map(label_of).unwrap_or("Tasks");
        let (inner, bg) = layout::card(
            Rect::new(content.x, content.y, content.width, content.height.min(20)),
            buf,
            t,
            Some(current),
            None,
            false,
        );
        let lines = [
            "One focus stop. ↑ ↓ move the cursor, Enter opens.",
            "",
            "›  current item · persists when focus leaves",
            "▎  keyboard cursor · only while focused",
            "░  hover · follows the pointer",
            "",
            "Disabled items are skipped and ignore the pointer.",
            "Collapsed mode keeps rows and markers, initials only.",
        ];
        let mut y = inner.y;
        for (i, l) in lines.iter().enumerate() {
            let accent = (2..=4).contains(&i);
            for (j, wl) in text::wrap(l, inner.width as usize).iter().enumerate() {
                if y >= inner.bottom() {
                    break;
                }
                if accent && j == 0 {
                    let (glyph, rest) =
                        wl.split_at(wl.chars().next().map(|c| c.len_utf8()).unwrap_or(0));
                    buf.set_string(inner.x, y, glyph, t.accent_fg().bg(bg));
                    buf.set_string(inner.x + 1, y, rest, t.secondary().bg(bg));
                } else {
                    buf.set_string(inner.x, y, wl, t.secondary().bg(bg));
                }
                y += 1;
            }
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        let items = nav_items();
        match ev {
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                if cx.focus_id() == Some(NAV) {
                    self.nav.set_focused(true);
                    return match self.nav.handle_key(*key, &items) {
                        NavigationListOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if cx.focus_id() == Some(COLLAPSE)
                    && matches!(key.code, KeyCode::Enter | KeyCode::Char(' '))
                {
                    self.collapsed = !self.collapsed;
                    return Route::Changed;
                }
                Route::Ignored
            }
            PageEvent::Click { id, pos } => {
                if *id == NAV {
                    cx.set_focus(NAV);
                    self.nav.set_focused(true);
                    let ev = MouseEvent {
                        kind: MouseEventKind::Down(MouseButton::Left),
                        position: *pos,
                        modifiers: termrock::input::KeyModifiers::NONE,
                    };
                    let _ = self.nav.handle_mouse(ev, &items);
                    return Route::Changed;
                }
                if *id == COLLAPSE {
                    cx.set_focus(COLLAPSE);
                    self.collapsed = !self.collapsed;
                    return Route::Changed;
                }
                Route::Ignored
            }
            _ => Route::Ignored,
        }
    }

    fn hints(&self, _focus: Option<WidgetId>) -> Vec<Hint> {
        vec![("↑ ↓", "Move"), ("Enter", "Open")]
    }

    fn capture_cursor(&self) -> Option<Position> {
        self.capture_cursor
    }
}

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

use crate::core::event::{Key, Outcome};
use crate::core::hints::{HasHints, HintCx};
use crate::core::id::WidgetId;
use crate::core::keymap::{Actions, KeyBinding, KeyMap, Nav, NavKeys, lookup};
use crate::core::scroll::{ScrollPane, Scrollable};
use crate::ui::ctx::RenderCtx;
use crate::ui::modal::Frame;
use crate::ui::popup::Placement;
use crate::ui::text::{truncate, width};
use crate::widgets::button::Button;
use crate::widgets::keyhint::Hint;
use crate::widgets::scrollbar;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpCmd {
    Close,
}

impl Actions for HelpCmd {
    fn label(self) -> &'static str {
        match self {
            HelpCmd::Close => "Close",
        }
    }

    fn default_keymap() -> KeyMap<Self> {
        vec![
            (KeyBinding::char('?'), HelpCmd::Close),
            (KeyBinding::char('q'), HelpCmd::Close),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HelpSection {
    pub title: String,

    pub rows: Vec<(String, String)>,
}

impl HelpSection {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            rows: vec![],
        }
    }

    pub fn row(mut self, keys: impl Into<String>, action: impl Into<String>) -> Self {
        let keys = keys.into();
        if crate::core::keyboard::label_deliverable(&keys) {
            self.rows.push((keys, action.into()));
        }
        self
    }

    pub fn bindings(mut self, bindings: &[crate::core::hints::Binding]) -> Self {
        for b in bindings.iter().filter(|b| !b.action.is_empty()) {
            match self.rows.iter_mut().find(|(_, a)| *a == b.action) {
                Some((keys, _)) => {
                    if !keys.split(' ').any(|k| k == b.label) {
                        keys.push(' ');
                        keys.push_str(&b.label);
                    }
                }
                None => self.rows.push((b.label.to_string(), b.action.to_string())),
            }
        }
        self
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

pub struct HelpOverlay {
    pub close_button: crate::widgets::icon_button::IconButton,

    pub dismiss: crate::ui::popup::Dismiss,
    pub id: WidgetId,
    pub scope: String,
    pub sections: Vec<(String, Vec<(String, String)>)>,

    pub scroll: ScrollPane,
    pub area: Rect,
    pub closed: bool,

    pub title: String,

    pub hint: String,

    pub nav: NavKeys,

    pub keymap: KeyMap<HelpCmd>,

    pub max_percent: u16,

    pub done: Button,

    strings: crate::theme::Strings,
}

impl std::fmt::Debug for HelpOverlay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HelpOverlay")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("scope", &self.scope)
            .field("sections", &self.sections.len())
            .field("closed", &self.closed)
            .finish_non_exhaustive()
    }
}

impl HasHints for HelpOverlay {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        let mut v = vec![];
        if let Some(k) = self.nav.move_hint(false) {
            v.push(Hint::new(k, self.strings.hint_scroll));
        }
        if self.dismiss.escape {
            v.push(Hint::new("Esc", self.strings.hint_close));
        }
        v
    }
}

impl HelpOverlay {
    pub fn close_button(mut self, button: crate::widgets::icon_button::IconButton) -> Self {
        self.close_button = button;
        self
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn keymap(mut self, keymap: KeyMap<HelpCmd>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn new(id: WidgetId, scope: &str, sections: Vec<(&str, Vec<(&str, &str)>)>) -> Self {
        Self {
            dismiss: crate::ui::popup::Dismiss::LIGHT,
            close_button: crate::widgets::icon_button::IconButton::close(crate::ui::modal::CLOSE),
            id,
            scope: scope.to_owned(),
            sections: sections
                .into_iter()
                .map(|(t, rows)| {
                    (
                        t.to_owned(),
                        rows.into_iter()
                            .filter(|(k, _)| crate::core::keyboard::label_deliverable(k))
                            .map(|(k, a)| (k.to_owned(), a.to_owned()))
                            .collect(),
                    )
                })
                .collect(),
            scroll: ScrollPane::default(),
            area: Rect::ZERO,
            closed: false,
            title: crate::theme::Strings::DEFAULT.help_title.into(),
            hint: String::new(),
            nav: NavKeys::default(),
            keymap: HelpCmd::default_keymap(),
            max_percent: 80,
            done: Button::secondary(id.sub("done"), crate::theme::Strings::DEFAULT.done),
            strings: crate::theme::Strings::DEFAULT,
        }
    }

    pub fn from_keymaps(id: WidgetId, scope: &str, sections: Vec<HelpSection>) -> Self {
        let mut h = Self::new(id, scope, vec![]);
        h.sections = sections
            .into_iter()
            .filter(|s| !s.is_empty())
            .map(|s| (s.title, s.rows))
            .collect();
        h
    }

    pub fn max_percent(mut self, percent: u16) -> Self {
        self.max_percent = percent.clamp(20, 100);
        self
    }

    pub fn on_click(&mut self, id: WidgetId) -> Outcome {
        if id == self.done.id && self.done.on_click()
            || id == crate::ui::modal::CLOSE && self.dismiss.shows_close()
        {
            self.closed = true;
            return Outcome::Changed;
        }
        Outcome::Consumed
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = title.to_owned();
        self
    }

    pub fn dismiss(mut self, dismiss: crate::ui::popup::Dismiss) -> Self {
        self.dismiss = dismiss;
        self
    }

    pub fn on_key(&mut self, key: &Key) -> Outcome {
        if let Some(nav) = self.nav.action(key) {
            match nav {
                Nav::Down => self.scroll.scroll_by(1),
                Nav::Up => self.scroll.scroll_by(-1),
                Nav::PageDown => self.scroll.page_down(),
                Nav::PageUp => self.scroll.page_up(),
                Nav::Home => self.scroll.jump_start(),
                Nav::End => self.scroll.jump_end(),
                Nav::Left | Nav::Right => return Outcome::Consumed,
            };
            return Outcome::Changed;
        }
        if let Some(HelpCmd::Close) = lookup(&self.keymap, key) {
            self.closed = true;
            return Outcome::Changed;
        }
        match key.code {
            KeyCode::Esc if !self.dismiss.escape => Outcome::Consumed,
            KeyCode::Esc | KeyCode::Enter => {
                self.closed = true;
                Outcome::Changed
            }
            _ => Outcome::Consumed,
        }
    }

    pub fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let s = ctx.theme.strings;
        let old = std::mem::replace(&mut self.strings, s);
        crate::theme::Strings::follow(&mut self.title, old.help_title, s.help_title);
        crate::theme::Strings::follow(&mut self.hint, old.help_footer, s.help_footer);
        self.scroll.begin(ctx.track_click);
        crate::theme::Strings::follow(&mut self.done.label, old.done, s.done);
        let pct = |n: u16| (u32::from(n) * u32::from(self.max_percent) / 100) as u16;
        let w = screen.width.saturating_sub(4).min(pct(screen.width));
        let h = screen.height.saturating_sub(3).min(pct(screen.height));
        let (area, inner) = Frame::new(w, h)
            .title(&self.title)
            .meta(Some(&self.scope))
            .placement(Placement::Center)
            .overlay(&*self)
            .render(screen, buf, ctx);
        self.area = area;
        if inner.is_empty() {
            return;
        }
        let t = ctx.theme;
        let bg = t.surface_elevated;

        let cols = (inner.width / 36).clamp(1, 3);
        let col_w = inner.width / cols;
        let cols = cols as usize;

        let widest = self
            .sections
            .iter()
            .flat_map(|(_, rows)| rows.iter().map(|(k, _)| width(&t.glyphs.key_label(k))))
            .max()
            .unwrap_or(0) as u16;
        let key_w = widest.min(col_w * 2 / 5).max(4);
        let action_w = col_w.saturating_sub(key_w + 3).max(8) as usize;

        let mut blocks: Vec<Vec<(String, String, bool)>> = vec![];
        for (title, rows) in &self.sections {
            let mut b = vec![(title.clone(), String::new(), true)];
            for (k, a) in rows {
                for (i, line) in crate::ui::text::wrap(a, action_w).into_iter().enumerate() {
                    let key = if i == 0 { k.clone() } else { String::new() };
                    b.push((key, line, false));
                }
            }
            b.push((String::new(), String::new(), false));
            blocks.push(b);
        }

        let mut columns: Vec<Vec<(String, String, bool)>> = vec![vec![]; cols];
        for (i, b) in blocks.into_iter().enumerate() {
            columns[i % cols].extend(b);
        }
        let total = columns.iter().map(Vec::len).max().unwrap_or(0);
        let body_h = inner.height.saturating_sub(1);
        self.scroll.set_content(total);
        self.scroll.set_viewport(body_h as usize);
        ctx.scrollable(self.id, inner);
        for (ci, col) in columns.iter().enumerate() {
            let x = inner.x + ci as u16 * col_w;
            for (k, i) in self.scroll.visible_range().enumerate() {
                let Some((key, action, heading)) = col.get(i) else {
                    continue;
                };
                let y = inner.y + k as u16;
                if *heading {
                    buf.set_string(x, y, key, t.secondary().bg(bg).add_modifier(Modifier::BOLD));
                } else {
                    buf.set_string(
                        x,
                        y,
                        truncate(&t.glyphs.key_label(key), key_w as usize),
                        t.key_hint_key().bg(bg),
                    );
                    buf.set_string(
                        x + key_w + 1,
                        y,
                        truncate(action, action_w),
                        t.secondary().bg(bg),
                    );
                }
            }
        }
        if self.scroll.overflows() {
            let body = Rect::new(
                area.x,
                inner.y,
                (area.right() - 1).saturating_sub(area.x),
                body_h,
            );
            self.scroll
                .draw_with_fade(body, buf, ctx, self.id, true, &[]);
            let pos = scrollbar::position_label(&self.scroll);
            let pw = width(&pos) as u16;

            let reserve = if self.dismiss.shows_close() {
                crate::ui::modal::CLOSE_WIDTH + 1
            } else {
                0
            };
            buf.set_string(
                area.right().saturating_sub(pw + 4 + reserve),
                area.y,
                format!(" {pos} "),
                t.faint().bg(bg),
            );
        }
        let y = inner.bottom().saturating_sub(1);
        let bw = self.done.width();
        let room = inner.width.saturating_sub(bw + 2);
        buf.set_string(
            inner.x,
            y,
            truncate(&self.hint, room as usize),
            t.faint().bg(bg),
        );
        self.done.render(
            Rect::new(inner.right().saturating_sub(bw), y, bw, 1),
            buf,
            ctx,
            bg,
        );
    }
}

impl Scrollable for HelpOverlay {
    fn pane(&mut self) -> &mut ScrollPane {
        &mut self.scroll
    }
}

impl crate::core::pointer::MouseTarget for HelpOverlay {
    type Action = ();

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn route(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> Option<crate::core::response::Response<()>> {
        let owner = ev.owner();
        let mine = owner == self.id || owner == self.done.id || owner == crate::ui::modal::CLOSE;
        mine.then(|| self.pointer(ev))
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<()> {
        use crate::core::response::Response;
        if ev.wheel().is_some() && !ev.on(self.id) {
            return Response::ignored();
        }
        if let Some(r) = crate::core::pointer::scroll_input(self, ev) {
            return r;
        }
        if ev.is(crate::core::intent::Phase::Click) {
            return self.on_click(ev.id).into();
        }
        Response::ignored()
    }
}

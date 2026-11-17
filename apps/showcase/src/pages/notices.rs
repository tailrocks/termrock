use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use termrock::core::event::Outcome;
use termrock::core::id::WidgetId;
use termrock::core::pointer::MouseTarget;
use termrock::theme::Tone;
use termrock::ui::ctx::RenderCtx;
use termrock::ui::layout::StackChild;
use termrock::ui::specimen::{SpecimenState, StateGrid};
use termrock::widgets::button::Button;
use termrock::widgets::notice::{Notice, NoticeKind};
use termrock::widgets::panel::Panel;

const ID: WidgetId = WidgetId::of("notices");

const OUTCOMES: [(NoticeKind, &str, &str); 3] = [
    (
        NoticeKind::Error,
        "Authentication failed",
        "FATAL: password authentication failed for user \"acme_app\" (SQLSTATE 28P01). Check the password in the keychain or use “Prompt for password”.",
    ),
    (
        NoticeKind::Warning,
        "Could not reach the host",
        "Connection timed out after 10 s (analytics.acme.io:5432). The host may be behind a VPN or the port may be blocked.",
    ),
    (
        NoticeKind::Success,
        "Connected in 182 ms",
        "PostgreSQL 16.4 · 42 schemas · TLS 1.3",
    ),
];

pub fn page() -> Box<dyn Page> {
    Box::new(NoticesPage::new())
}

const CARDS: [(&str, &str); 3] = [
    ("prod-replica", "postgres · connection test"),
    ("Result 1", "error card · 0.4 ms"),
    (
        "New connection",
        "inline · one per kind, the glyph carries it",
    ),
];

const ADDRESS: &str = "acme_app@analytics.acme.io:5432/analytics";

pub struct NoticesPage {
    template: Template,
    opts: PageValues,
    test: Button,

    outcome: usize,
    sections: crate::pages::layout::Sections,
}

impl NoticesPage {
    pub fn new() -> Self {
        Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::Notices)),
            test: Button::secondary(ID.sub("test"), "Test connection"),
            outcome: 0,
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        }
    }

    fn styled(&self, n: Notice) -> Notice {
        let mut n = n
            .max_lines(self.opts.value("notices.max_lines").parse().unwrap_or(2))
            .message_lines(
                self.opts
                    .value("notices.message_lines")
                    .parse()
                    .unwrap_or(2),
            )
            .detail_tone(match self.opts.value("notices.detail_tone") {
                "muted" => Tone::Muted,
                _ => Tone::Secondary,
            })
            .mark_cut(self.opts.on("notices.mark_cut"));
        if !self.opts.on("notices.detail") {
            n.detail = None;
        }
        if !self.opts.on("notices.hint") {
            n.hint = None;
        }
        n
    }

    fn connection(&self) -> Notice {
        let (kind, message, detail) = OUTCOMES[self.outcome];
        let n = Notice::new(kind, message).detail(detail);
        self.styled(match kind {
            NoticeKind::Success => n.hint("Save keeps the connection"),
            _ => n.hint("Test connection retries · e edits the connection"),
        })
    }

    fn query(&self) -> Notice {
        let kind = match self.opts.value("notices.kind") {
            "warning" => NoticeKind::Warning,
            "info" => NoticeKind::Info,
            "success" => NoticeKind::Success,
            _ => NoticeKind::Error,
        };
        self.styled(
            Notice::error("syntax error at or near \"FORM\"")
                .kind(kind)
                .detail("LINE 1: SELECT id, email, created_at FORM users WHERE created_at > now() - interval '7 days' ORDER BY created_at DESC LIMIT 200")
                .hint("Enter on the result tab jumps to the statement"),
        )
    }

    fn inline(&self) -> [Notice; 4] {
        [
            self.styled(Notice::error("Port must be a number between 1 and 65535")),
            self.styled(
                Notice::warning("3 rows changed outside this session")
                    .detail("Reload to see them; saving now overwrites them"),
            ),
            self.styled(Notice::info("Read-only replica: edits are disabled")),
            self.styled(Notice::success("Saved 12 rows in 84 ms")),
        ]
    }

    fn width(&self, w: u16) -> u16 {
        match self.opts.value("notices.width") {
            "narrow" => w.min(44),
            _ => w.min(96),
        }
    }

    fn children(&self, w: u16) -> [StackChild; 3] {
        let inline: u16 = self.inline().iter().map(|n| n.height(w)).sum::<u16>() + 3;
        [
            StackChild::fixed(self.connection().height(w) + 7),
            StackChild::fixed(self.query().height(w) + 3),
            StackChild::fixed(inline + 3),
        ]
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let focus = ctx.interaction.has_focus(self.test.id).then_some(0);
        let w = self.width(area.width.saturating_sub(4));
        let connection = self.connection();
        let query = self.query();
        let inline = self.inline();
        let children = self.children(w);
        let l = self.sections.layout(area, &children, focus, buf, ctx);
        let t = ctx.theme;
        match l.slot(0) {
            Some(r) => {
                let panel = Panel::card(Some(CARDS[0].0)).meta(CARDS[0].1);
                let bg = panel.bg(t);
                let inner = panel.render(r, buf, t);
                buf.set_string(
                    inner.x,
                    inner.y,
                    termrock::ui::text::truncate(ADDRESS, inner.width as usize),
                    t.secondary().bg(bg),
                );
                let body = Rect::new(
                    inner.x,
                    inner.y + 2,
                    w.min(inner.width),
                    inner.height.saturating_sub(4),
                );
                let used = connection.render(body, buf, ctx, bg);
                let y = body.y + used + 1;
                if y < inner.bottom() {
                    let bw = self.test.width();
                    self.test.render(Rect::new(inner.x, y, bw, 1), buf, ctx, bg);
                }
            }
            None => ctx.focus_stop(self.test.id),
        }
        if let Some(r) = l.slot(1) {
            let panel = Panel::card(Some(CARDS[1].0)).meta(CARDS[1].1);
            let bg = panel.bg(t);
            let inner = panel.render(r, buf, t);
            query.render(
                Rect::new(inner.x, inner.y, w.min(inner.width), inner.height),
                buf,
                ctx,
                bg,
            );
        }
        if let Some(r) = l.slot(2) {
            let panel = Panel::card(Some(CARDS[2].0)).meta(CARDS[2].1);
            let bg = panel.bg(t);
            let inner = panel.render(r, buf, t);
            let mut y = inner.y;
            for n in &inline {
                if y >= inner.bottom() {
                    break;
                }
                let h = n.height(w);
                y += n.render(Rect::new(inner.x, y, w.min(inner.width), h), buf, ctx, bg) + 1;
            }
        }
    }

    fn test_connection(&mut self, cx: &mut Cx) {
        self.outcome = (self.outcome + 1) % OUTCOMES.len();
        cx.status(OUTCOMES[self.outcome].1);
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    StateGrid::new(ID.sub("states"))
        .label_width(11)
        .column_width(32)
        .narrow_width(20)
        .rows(&[Rest, Error, Overflow, Narrow])
        .column(
            "With detail",
            Notice::error("Authentication failed").detail("Password rejected for acme_app"),
        )
        .column(
            "With hint",
            Notice::warning("Disk almost full").hint("Free space to keep writing"),
        )
}

impl Page for NoticesPage {
    fn title(&self) -> &'static str {
        "Notices"
    }
    fn blurb(&self) -> &'static str {
        "Notice: inline error, warning, information and success blocks with a wrapped detail"
    }

    fn measure(&self, _avail: Size, _ctx: &RenderCtx) -> Size {
        let w = self.width(u16::MAX);
        let content = w.max(termrock::ui::text::width(ADDRESS) as u16);
        let width = CARDS
            .iter()
            .map(|(title, meta)| {
                Panel::card(Some(title))
                    .meta(meta)
                    .outer(Size::new(content, 0))
                    .width
            })
            .max()
            .unwrap_or(0);

        Size::new(
            width.max(w + 4),
            self.sections.stack.measure(&self.children(w)),
        )
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
        if self.template.section() == Section::Playground
            && let Some(o) = self.sections.handle(ev)
        {
            return o;
        }
        match ev {
            PageEvent::Key(key) if cx.focus.is(self.test.id) => {
                let (o, act) = self.test.on_key(key);
                if act {
                    self.test_connection(cx);
                }
                o
            }
            PageEvent::Pointer(ev) => {
                if let Some(r) = self.test.route(ev)
                    && r.outcome() != Outcome::Ignored
                {
                    if r.has_actions() {
                        self.test_connection(cx);
                    }
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        vec![("Enter", "Activate")]
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        self.opts.set(key, value);
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

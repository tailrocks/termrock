use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::{Actions, KeyBinding, hint_key};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::{SplitDir, SplitKeys, StackChild};
use junie_tui::ui::pane_tree::{Frames, PaneCmd, PaneId, PaneResult, PaneTree};
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::ui::text::truncate;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::panel::Panel;

const ID: WidgetId = WidgetId::of("panetree");
const TREE: WidgetId = WidgetId::of("panetree").sub("tree");

const PANES: &[(&str, &[&str])] = &[
    (
        "shell",
        &[
            "~/src/junie-tui $ cargo nextest run",
            "    Starting 1977 tests across 6 binaries",
            "     Summary 1977 passed",
            "~/src/junie-tui $ ▌",
        ],
    ),
    (
        "build",
        &[
            "   Compiling junie-tui v0.4.0",
            "    Finished `dev` profile in 4.12s",
        ],
    ),
    (
        "logs",
        &[
            "16:42:01 INFO  router: page masterdetail",
            "16:42:03 INFO  pane tree: split right",
            "16:42:07 WARN  seam at minimum",
        ],
    ),
    (
        "editor",
        &[
            "pub fn toggle_zoom(&mut self) -> PaneResult {",
            "    // …",
            "}",
        ],
    ),
    ("htop", &["CPU [|||||     23%]", "MEM [||||||||  61%]"]),
    (
        "notes",
        &["- zoom restores with Alt+Z", "- F6 cycles the panes"],
    ),
];

pub fn page() -> Box<dyn Page> {
    Box::new(PaneTreePage::new())
}

pub struct PaneTreePage {
    template: Template,
    opts: PageValues,
    tree: PaneTree,

    buttons: [Button; 4],

    next: PaneId,
    sections: crate::pages::layout::Sections,
}

fn name(p: PaneId) -> &'static str {
    PANES[(p as usize).saturating_sub(1) % PANES.len()].0
}

impl PaneTreePage {
    pub fn new() -> Self {
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::PaneTree)),
            tree: PaneTree::new(TREE, 1),
            buttons: [
                Button::secondary(ID.sub("split-right"), "Split right"),
                Button::secondary(ID.sub("split-below"), "Split below"),
                Button::secondary(ID.sub("close"), "Close pane"),
                Button::secondary(ID.sub("zoom"), "Zoom"),
            ],
            next: 4,
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        };
        page.rebuild();
        page
    }

    fn rebuild(&mut self) {
        let o = &self.opts;
        let frames = match o.value("panetree.frames") {
            "always" => Frames::Always,
            "never" => Frames::Never,
            _ => Frames::WhenSplit,
        };
        let (cols, rows) = match o.value("panetree.min") {
            "24x6" => (24, 6),
            _ => (12, 3),
        };
        let keys = match o.value("panetree.keys") {
            "custom" => vec![
                (KeyBinding::ctrl('n'), PaneCmd::Next),
                (KeyBinding::ctrl('p'), PaneCmd::Prev),
                (KeyBinding::ctrl('x'), PaneCmd::Zoom),
            ],
            "none" => vec![],
            _ => PaneCmd::default_keymap(),
        };
        let step = o.number("panetree.step").unwrap_or(2);
        let mut tree = PaneTree::new(TREE, 1)
            .frames(frames)
            .min_size(cols, rows)
            .keys(keys)
            .split_keys(if o.value("panetree.keys") == "none" {
                SplitKeys::default().keymap(vec![]).step(step)
            } else {
                SplitKeys::default().step(step)
            })
            .hit_pad(o.number("panetree.hit_pad").unwrap_or(1));
        match o.value("panetree.layout") {
            "single" => {}
            "grid" => {
                tree.split(1, 2, SplitDir::Horizontal, false);
                tree.split(1, 3, SplitDir::Vertical, false);
                tree.split(2, 4, SplitDir::Vertical, false);
            }
            _ => {
                tree.split(1, 2, SplitDir::Horizontal, false);
                tree.split(2, 3, SplitDir::Vertical, false);
            }
        }
        tree.focused = 1;
        self.next = tree.leaves().into_iter().max().unwrap_or(1) + 1;
        self.tree = tree;
    }

    fn legend(&self) -> String {
        let mut parts = Vec::new();
        if let Some(k) = hint_key(&self.tree.keys, PaneCmd::Next) {
            parts.push(format!("{k} next pane"));
        }
        if hint_key(&self.tree.keys, PaneCmd::FocusLeft).is_some() {
            parts.push("Alt+arrows move".to_owned());
        }
        if let Some(h) = self.tree.split_keys.hint(SplitDir::Vertical) {
            parts.push(format!("{} resize", h.key.replace("↑↓", "arrows")));
        }
        if let Some(k) = hint_key(&self.tree.keys, PaneCmd::Zoom) {
            parts.push(format!("{k} zoom"));
        }
        if parts.is_empty() {
            "no pane keys: use the buttons and the pointer".to_owned()
        } else {
            parts.join(" · ")
        }
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let l = self.sections.layout(area, &[self.child()], None, buf, ctx);
        match l.slot(0) {
            Some(r) => self.render_demo(r, buf, ctx),
            None => {
                for b in &self.buttons {
                    ctx.focus_stop(b.id);
                }
                ctx.focus_stop(TREE);
            }
        }
    }

    fn meta(&self) -> String {
        let n = self.tree.leaves().len();
        format!(
            "{} · focus {}{}",
            junie_tui::ui::text::plural(n, "pane", "panes"),
            name(self.tree.focused),
            if self.tree.zoomed().is_some() {
                " · zoomed"
            } else {
                ""
            }
        )
    }

    fn card_size(&self) -> Size {
        let w = |s: &str| junie_tui::ui::text::width(s) as u16;
        let tree = self.tree.natural_size(&|p| {
            let lines = PANES[(p as usize).saturating_sub(1) % PANES.len()].1;
            let widest = lines.iter().map(|l| w(l)).max().unwrap_or(0);
            Size::new(widest + 1, lines.len() as u16)
        });
        let buttons: u16 = self.buttons.iter().map(Button::width).sum::<u16>() + 2 * 3;
        let content = Size::new(
            tree.width.max(buttons).max(w(&self.legend())),
            3 + tree.height,
        );
        let meta = self.meta();
        Panel::card(Some("Workspace panes"))
            .meta(&meta)
            .outer(content)
    }

    fn child(&self) -> StackChild {
        StackChild::flex(12, self.card_size().height.max(26))
    }

    fn render_demo(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let meta = self.meta();
        let panel = Panel::card(Some("Workspace panes")).meta(&meta);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        self.buttons[3].label = if self.tree.zoomed().is_some() {
            "Restore".into()
        } else {
            "Zoom".into()
        };
        let widths: Vec<u16> = self.buttons.iter().map(Button::width).collect();
        let rects = junie_tui::widgets::button::row_layout(
            Rect::new(inner.x, inner.y, inner.width, 1),
            &widths,
            2,
        );
        for (b, r) in self.buttons.iter_mut().zip(rects) {
            b.render(r, buf, ctx, bg);
        }
        let legend = self.legend();
        buf.set_string(
            inner.x,
            inner.y + 1,
            truncate(&legend, inner.width as usize),
            t.faint().bg(bg),
        );
        let body = Rect::new(
            inner.x,
            inner.y + 3,
            inner.width,
            inner.height.saturating_sub(3),
        );
        let title = |p: PaneId| name(p).to_owned();
        let panes = self.tree.render(body, buf, ctx, bg, &title);
        for (p, r) in panes {
            let lines = PANES[(p as usize).saturating_sub(1) % PANES.len()].1;
            for (i, line) in lines.iter().enumerate() {
                let y = r.y + i as u16;
                if y >= r.bottom() || r.width < 2 {
                    break;
                }
                buf.set_string(
                    r.x + 1,
                    y,
                    truncate(line, (r.width - 1) as usize),
                    t.secondary().bg(bg),
                );
            }
        }
    }

    fn report(r: PaneResult, cx: &mut Cx) -> Outcome {
        match r {
            Ok(o) => o,
            Err(why) => {
                cx.status(why.message());
                Outcome::Changed
            }
        }
    }

    fn button(&mut self, i: usize, cx: &mut Cx) -> Outcome {
        let focused = self.tree.focused;
        match i {
            0 | 1 => {
                let dir = if i == 0 {
                    SplitDir::Horizontal
                } else {
                    SplitDir::Vertical
                };
                let new = self.next;
                self.next += 1;
                self.tree.split(focused, new, dir, false);
                cx.status(format!("Opened {}", name(new)));
                Outcome::Changed
            }
            2 => Self::report(self.tree.close(focused).map(|_| Outcome::Changed), cx),
            _ => Self::report(self.tree.toggle_zoom(), cx),
        }
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .label_width(12)
        .column_width(40)
        .rows(&[Rest, Hover, Focused, Selected])
        .column("Pane tree", PaneTree::new(id.child(0), 1))
}

impl Page for PaneTreePage {
    fn title(&self) -> &'static str {
        "Pane tree"
    }
    fn blurb(&self) -> &'static str {
        "PaneTree: nested splits, frames, focus moves, resize chords and zoom"
    }

    fn measure(&self, _avail: Size, _ctx: &RenderCtx) -> Size {
        Size::new(
            self.card_size().width,
            self.sections.stack.measure(&[self.child()]),
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
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        if let Some(o) = self.sections.handle(ev) {
            return o;
        }
        match ev {
            PageEvent::Key(key) => {
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                if f == TREE {
                    return match self.tree.on_key(key) {
                        Some(r) => Self::report(r, cx),
                        None => Outcome::Ignored,
                    };
                }
                let Some(i) = self.buttons.iter().position(|b| b.id == f) else {
                    return Outcome::Ignored;
                };
                let (o, act) = self.buttons[i].on_key(key);
                if act {
                    return self.button(i, cx);
                }
                o
            }
            PageEvent::Pointer(ev) => {
                if let Some(o) = self.tree.on_pointer(ev) {
                    if ev.is(Phase::Click) {
                        cx.focus.focus(TREE);
                    }
                    return o;
                }
                if ev.is(Phase::Click)
                    && let Some(i) = self.buttons.iter().position(|b| b.id == ev.id)
                {
                    cx.focus.focus(ev.id);
                    if self.buttons[i].on_click() {
                        return self.button(i, cx);
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
        if focus == Some(TREE) {
            return self
                .tree
                .hints()
                .into_iter()
                .map(|h| {
                    let key = match h.key {
                        std::borrow::Cow::Borrowed(key) => key,
                        std::borrow::Cow::Owned(_) => "",
                    };
                    let action = match h.action {
                        std::borrow::Cow::Borrowed(s) => s,
                        std::borrow::Cow::Owned(_) => "",
                    };
                    (key, action)
                })
                .collect();
        }
        vec![("Enter", "Activate"), ("Tab", "Next")]
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
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

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::PointerEvent;
use junie_tui::core::scroll::TrackClick;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::{
    Maximized, Overflow, Split, SplitDir, Stack, StackChild, StackLayout, StackScroll,
};
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::button::Button;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::splitter::{SeamDoubleClick, Splitter};

const ID: WidgetId = WidgetId::of("splitsstacks");
const OUTER: WidgetId = WidgetId::of("splitsstacks").sub("outer");
const INNER: WidgetId = WidgetId::of("splitsstacks").sub("inner");
const STACK: WidgetId = WidgetId::of("splitsstacks").sub("stack");

const STACK_ROWS: u16 = 8;

const STACK_W: u16 = 60;

const EXPLORER: &[&str] = &[
    "▾ src",
    "  ▸ core",
    "  ▸ ui",
    "  ▾ widgets",
    "      splitter.rs",
    "      progress.rs",
    "  lib.rs",
    "Cargo.toml",
];
const EDITOR: &[&str] = &[
    "pub fn toggle_max(&mut self, which: Maximized) {",
    "    self.maximized = if self.maximized == which {",
    "        Maximized::None",
    "    } else {",
    "        which",
    "    };",
    "}",
];
const OUTPUT: &[&str] = &[
    "   Compiling junie-tui v0.4.0",
    "    Finished `dev` profile in 4.12s",
    "     Running tests (14 passed)",
];

const CARDS: [(&str, u16); 5] = [
    ("Connection", 3),
    ("Credentials", 3),
    ("Network", 3),
    ("Advanced", 3),
    ("Danger zone", 3),
];

pub fn page() -> Box<dyn Page> {
    Box::new(SplitsStacksPage::new())
}

pub struct SplitsStacksPage {
    template: Template,
    opts: PageValues,
    outer: Split,
    inner: Split,
    outer_seam: Splitter,
    inner_seam: Splitter,

    outer_area: Rect,
    inner_area: Rect,
    /// The seam under the pointer between its press and the end of the gesture.
    held: Option<WidgetId>,
    /// A seam whose grab Esc dropped: the pointer still holds it, so its drag
    /// events are ignored until the button is released.
    dropped: Option<WidgetId>,
    zoom: [Button; 3],
    stack: Stack,
    scroll: StackScroll,
    sections: crate::pages::layout::Sections,
}

impl SplitsStacksPage {
    pub fn new() -> Self {
        let opts = PageValues::new(page_options(PageId::SplitsStacks));
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts,
            outer: Split::new(30, 16, 24),
            inner: Split::new(60, 3, 3),
            outer_seam: Splitter::new(OUTER, SplitDir::Horizontal),
            inner_seam: Splitter::new(INNER, SplitDir::Vertical),
            outer_area: Rect::ZERO,
            inner_area: Rect::ZERO,
            held: None,
            dropped: None,
            zoom: [
                Button::secondary(ID.sub("zoom-editor"), "Zoom editor"),
                Button::secondary(ID.sub("zoom-output"), "Zoom output"),
                Button::secondary(ID.sub("restore"), "Restore"),
            ],
            stack: Stack::default(),
            scroll: StackScroll::default(),
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        };
        page.rebuild();
        page
    }

    fn dir(&self) -> SplitDir {
        match self.opts.value("splitsstacks.dir") {
            "vertical" => SplitDir::Vertical,
            _ => SplitDir::Horizontal,
        }
    }

    fn rebuild(&mut self) {
        let o = &self.opts;
        let percent = o.number("splitsstacks.percent").unwrap_or(30);
        let min = o.number("splitsstacks.min").unwrap_or(16);

        self.outer = match self.dir() {
            SplitDir::Horizontal => Split::new(percent, min, 24),
            SplitDir::Vertical => Split::new(percent, (min / 5).max(3), 6),
        };
        let mut inner = Split::new(60, 3, 3);
        match o.value("splitsstacks.maximized") {
            "first" => inner.toggle_max(Maximized::First),
            "second" => inner.toggle_max(Maximized::Second),
            _ => {}
        }
        self.inner = inner;
        let dir = self.dir();
        let across = match dir {
            SplitDir::Horizontal => SplitDir::Vertical,
            SplitDir::Vertical => SplitDir::Horizontal,
        };
        let double = match o.value("splitsstacks.double_click") {
            "ignore" => SeamDoubleClick::Ignore,
            _ => SeamDoubleClick::Reset,
        };
        let pad = o.number("splitsstacks.hit_pad").unwrap_or(1);
        self.outer_seam = Splitter::new(OUTER, dir).hit_pad(pad).double_click(double);
        self.inner_seam = Splitter::new(INNER, across)
            .hit_pad(pad)
            .double_click(double);
        self.stack = Stack::default()
            .gap(o.number("splitsstacks.gap").unwrap_or(0))
            .overflow(match o.value("splitsstacks.overflow") {
                "clip" => Overflow::Clip,
                _ => Overflow::Scroll,
            })
            .min_child_rows(o.number("splitsstacks.min_child_rows").unwrap_or(1))
            .scrollbar_gap(o.number("splitsstacks.scrollbar_gap").unwrap_or(1))
            .track_click(match o.value("splitsstacks.track_click") {
                "jump" => TrackClick::Jump,
                _ => TrackClick::Page,
            });
        self.scroll.reset();
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let focus = self
            .zoom
            .iter()
            .any(|b| ctx.interaction.has_focus(b.id))
            .then_some(0);
        let l = self
            .sections
            .layout(area, &self.children(), focus, buf, ctx);
        match l.slot(0) {
            Some(r) => self.render_panes(r, buf, ctx),
            None => self.zoom.iter().for_each(|b| ctx.focus_stop(b.id)),
        }
        if let Some(r) = l.slot(1) {
            self.render_stack(r, buf, ctx);
        }
    }

    fn panes_meta(&self) -> String {
        format!(
            "{}% · min {} · {}",
            self.outer.percent,
            self.outer.min_first,
            match self.inner.maximized {
                Maximized::None => "drag a seam, double-click resets",
                Maximized::First => "editor zoomed",
                Maximized::Second => "output zoomed",
            }
        )
    }

    fn panes_size(&self) -> Size {
        let pane = |title: &str, lines: &[&str]| {
            let w = lines
                .iter()
                .map(|l| junie_tui::ui::text::width(l) as u16)
                .max()
                .unwrap_or(0);
            Panel::framed(Some(title)).outer(Size::new(w, lines.len() as u16))
        };
        let across = |split: &Split, dir: SplitDir, a: Size, b: Size| match dir {
            SplitDir::Horizontal => Size::new(
                split.natural_extent(a.width, b.width, 1),
                a.height.max(b.height),
            ),
            SplitDir::Vertical => Size::new(
                a.width.max(b.width),
                split.natural_extent(a.height, b.height, 1),
            ),
        };
        let inner = across(
            &self.inner,
            self.inner_seam.dir,
            pane("Editor", EDITOR),
            pane("Output", OUTPUT),
        );
        let body = across(&self.outer, self.dir(), pane("Explorer", EXPLORER), inner);
        let buttons: u16 = self.zoom.iter().map(Button::width).sum::<u16>()
            + 2 * self.zoom.len().saturating_sub(1) as u16;
        let meta = self.panes_meta();
        Panel::card(Some("Three panes"))
            .meta(&meta)
            .outer(Size::new(body.width.max(buttons), 2 + body.height))
    }

    fn stack_width(&self) -> u16 {
        let meta = self.stack_meta();
        let note = junie_tui::ui::text::width(&format!("{} not in view", CARDS.len())) as u16;
        Panel::card(Some("Stack"))
            .meta(&meta)
            .outer(Size::new(STACK_W + 2 + note, 0))
            .width
    }

    fn stack_meta(&self) -> String {
        let want: u16 = CARDS.iter().map(|c| c.1).sum();
        format!("{want} rows wanted, {STACK_ROWS} given")
    }

    fn children(&self) -> [StackChild; 2] {
        [
            StackChild::flex(12, self.panes_size().height.max(18)),
            StackChild::fixed(3 + 1 + STACK_ROWS),
        ]
    }

    fn render_panes(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let dir = self.dir();
        let meta = self.panes_meta();
        let panel = Panel::card(Some("Three panes")).meta(&meta);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);

        let widths: Vec<u16> = self.zoom.iter().map(Button::width).collect();
        let rects = junie_tui::widgets::button::row_layout(
            Rect::new(inner.x, inner.y, inner.width, 1),
            &widths,
            2,
        );
        for (b, r) in self.zoom.iter_mut().zip(rects) {
            b.render(r, buf, ctx, bg);
        }
        let body = Rect::new(
            inner.x,
            inner.y + 2,
            inner.width,
            inner.height.saturating_sub(2),
        );
        self.outer_area = body;
        let (first, second) = self.outer.layout(dir, body, 1);
        let seam = self.outer.handle(dir, body, 1);
        pane(first, buf, ctx, "Explorer", EXPLORER);
        self.outer_seam.render(seam, buf, ctx, bg);
        let across = self.inner_seam.dir;
        self.inner_area = second;
        let (top, bottom) = self.inner.layout(across, second, 1);
        let seam = self.inner.handle(across, second, 1);
        pane(top, buf, ctx, "Editor", EDITOR);
        self.inner_seam.render(seam, buf, ctx, bg);
        pane(bottom, buf, ctx, "Output", OUTPUT);
    }

    fn render_stack(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let meta = self.stack_meta();
        let panel = Panel::card(Some("Stack")).meta(&meta);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        let region = Rect::new(
            inner.x,
            inner.y,
            inner.width.min(STACK_W),
            inner.height.min(STACK_ROWS),
        );
        let children: Vec<StackChild> = CARDS.iter().map(|c| StackChild::flex(1, c.1)).collect();
        let l: StackLayout = self.stack.layout(region, &children, &mut self.scroll, None);
        Stack::chrome(&l, region, buf, ctx, STACK, &self.scroll);
        for (i, (title, _)) in CARDS.iter().enumerate() {
            let Some(r) = l.slot(i) else { continue };
            let style = t.secondary().bg(t.surface_elevated);
            for y in r.y..r.bottom() {
                buf.set_string(r.x, y, " ".repeat(r.width as usize), style);
            }
            let head = format!(
                "▸ {title} · {}",
                junie_tui::ui::text::plural(r.height as usize, "row", "rows")
            );
            buf.set_string(
                r.x + 1,
                r.y,
                junie_tui::ui::text::truncate(&head, r.width.saturating_sub(2) as usize),
                t.primary().bg(t.surface_elevated),
            );
        }
        let hidden = l.hidden_above + l.hidden_below;
        if hidden > 0 && inner.width > region.width + 4 {
            buf.set_string(
                region.right() + 2,
                region.y,
                format!("{hidden} not in view"),
                t.faint().bg(bg),
            );
        }
    }

    fn zoom_clicked(&mut self, i: usize) -> Outcome {
        match i {
            0 => self.inner.toggle_max(Maximized::First),
            1 => self.inner.toggle_max(Maximized::Second),
            _ => {
                self.inner.reset();
                self.outer.reset();
            }
        }
        Outcome::Changed
    }

    fn pointer(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.scroll.pointer(STACK, ev) {
            return o;
        }
        let (id, pos) = (ev.id, ev.pos());
        if matches!(
            ev.phase(),
            Some(Phase::Release | Phase::DragEnd | Phase::Click)
        ) {
            self.held = None;
            self.dropped = None;
        }
        match ev.phase() {
            Some(Phase::Press) => {
                if let Some((seam, split, area)) = self.seam(id) {
                    seam.on_press(split, area, 1, pos);
                    self.held = Some(id);
                    self.dropped = None;
                    return Outcome::Consumed;
                }
                Outcome::Ignored
            }
            Some(Phase::Drag) => {
                if self.dropped == Some(id) {
                    return Outcome::Consumed;
                }
                if let Some((seam, split, area)) = self.seam(id) {
                    return seam.on_drag(split, area, 1, pos);
                }
                Outcome::Ignored
            }
            Some(Phase::Click) => {
                if let Some((seam, split, _)) = self.seam(id) {
                    return seam.on_click(split, ev.count());
                }
                if let Some(i) = self.zoom.iter().position(|b| b.id == id) {
                    cx.focus.focus(id);
                    if self.zoom[i].on_click() {
                        return self.zoom_clicked(i);
                    }
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    /// Esc drops the grab of the seam being dragged. The panes keep the size
    /// the drag reached: only the rest of the gesture is cancelled.
    fn cancel_drag(&mut self, cx: &mut Cx) -> Outcome {
        let held = self.outer_seam.cancel_drag() | self.inner_seam.cancel_drag();
        self.dropped = self.held.take();
        if held {
            cx.status("Drag cancelled: the seam stays where it is");
        }
        Outcome::Changed
    }

    fn seam(&mut self, id: WidgetId) -> Option<(&mut Splitter, &mut Split, Rect)> {
        if id == OUTER {
            Some((&mut self.outer_seam, &mut self.outer, self.outer_area))
        } else if id == INNER {
            Some((&mut self.inner_seam, &mut self.inner, self.inner_area))
        } else {
            None
        }
    }
}

fn pane(area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, title: &str, lines: &[&str]) {
    if area.width < 4 || area.height < 3 {
        return;
    }
    let t = ctx.theme;
    let panel = Panel::framed(Some(title));
    let bg = panel.bg(t);
    let inner = panel.render(area, buf, t);
    for (i, line) in lines.iter().enumerate() {
        let y = inner.y + i as u16;
        if y >= inner.bottom() {
            break;
        }
        buf.set_string(
            inner.x,
            y,
            junie_tui::ui::text::truncate(line, inner.width as usize),
            t.secondary().bg(bg),
        );
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .label_width(12)
        .column_width(20)
        .rows(&[Rest, Hover, Pressed])
        .column(
            "Vertical seam",
            Splitter::new(id.child(0), SplitDir::Horizontal),
        )
        .column(
            "Horizontal seam",
            Splitter::new(id.child(1), SplitDir::Vertical),
        )
}

impl Page for SplitsStacksPage {
    fn title(&self) -> &'static str {
        "Splits & stacks"
    }
    fn blurb(&self) -> &'static str {
        "Split panes with draggable seams and zoom, stacks that shrink, scroll or clip"
    }

    fn measure(&self, _avail: Size, _ctx: &RenderCtx) -> Size {
        Size::new(
            self.panes_size().width.max(self.stack_width()),
            self.sections.stack.measure(&self.children()),
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
            PageEvent::Pointer(ev) => self.pointer(ev, cx),
            PageEvent::Key(key) => {
                if key.is(KeyCode::Esc) && self.held.is_some() {
                    return self.cancel_drag(cx);
                }
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                let Some(i) = self.zoom.iter().position(|b| b.id == f) else {
                    return Outcome::Ignored;
                };
                let (o, act) = self.zoom[i].on_key(key);
                if act {
                    return self.zoom_clicked(i);
                }
                o
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
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

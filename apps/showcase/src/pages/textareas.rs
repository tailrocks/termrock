use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::inputs::{blur_action, custom_text_keymap, field_keys, pointer_select};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::edit::{EditEntry, FocusSelect};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::NavKeys;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::core::scroll::HScroll;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::measure::{Measure, Tile, Tiled, tiles};
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::input::InputAction;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::textarea::TextArea;

const ID: WidgetId = WidgetId::of("textareas");

pub struct TextAreasPage {
    template: Template,
    opts: PageValues,
    areas: Vec<TextArea>,
}

impl TextAreasPage {
    pub fn new() -> Self {
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::TextAreas)),
            areas: Vec::new(),
        };
        page.rebuild();
        page
    }

    fn rebuild(&mut self) {
        let long = (1..=28)
            .map(|i| match i % 4 {
                0 => format!("{i:>2}. Run the integration suite and attach the report. Review 日本語 · 👩‍💻 · cafe\u{301} through this long Unicode line."),
                1 => format!("{i:>2}. Read src/api/billing.rs before touching invoices."),
                2 => format!("{i:>2}. Keep the public API stable; add, never rename."),
                _ => format!("{i:>2}. Open a PR against main with a clear summary."),
            })
            .collect::<Vec<_>>()
            .join("\n");
        let areas = vec![
            TextArea::new(ID.child(0), "Task description", 8)
                .value(&long)
                .help("Home/End follows long lines · Esc finishes"),
            TextArea::new(ID.child(1), "Notes", 8)
                .placeholder("Anything the agent should know…")
                .help("Optional"),
            TextArea::new(ID.child(2), "Read-only transcript", 4)
                .value("Junie: Reading 14 files…\nJunie: Plan ready. 3 steps.")
                .disabled(true),
            TextArea::new(ID.child(3), "Commit message", 4)
                .value("fix stuff")
                .error(Some("Use the imperative mood and explain why")),
        ];
        let o = &self.opts;
        let keys = field_keys(o, "textareas.field_keys");
        self.areas = areas
            .into_iter()
            .enumerate()
            .map(|(i, a)| {
                let mut a = a
                    .focus_select(match o.value("textareas.focus_select") {
                        "select-all" => FocusSelect::SelectAll,
                        "caret-start" => FocusSelect::CaretStart,
                        _ => FocusSelect::CaretEnd,
                    })
                    .esc_reverts(o.on("textareas.esc_reverts"))
                    .nav(match o.value("textareas.nav") {
                        "vim" => NavKeys::VIM,
                        "no-page" => NavKeys::default().page(false),
                        _ => NavKeys::default(),
                    })
                    .pointer_select(pointer_select(o, "textareas.pointer_select"))
                    .blur_action(blur_action(o, "textareas.blur_action"))
                    .label_hit(o.on("textareas.label_hit"));
                match o.value("textareas.edit_entry") {
                    "immediate" => a = a.edit_entry(EditEntry::Immediate),
                    "type-to-edit" => a = a.edit_entry(EditEntry::TypeToEdit),
                    "explicit" => a = a.edit_entry(EditEntry::Explicit),
                    _ => {}
                }
                if let Some(k) = keys {
                    a = a.field_keys(k);
                }
                if o.value("textareas.text_keymap") == "custom" {
                    a = a.text_keymap(custom_text_keymap(keys.unwrap_or_default()));
                }
                match o.value("textareas.horizontal") {
                    "on" => a = a.horizontal(HScroll::default()),
                    "off" => a = a.horizontal(HScroll::off()),
                    _ => {}
                }

                if let Some(old) = self.areas.get(i) {
                    a = a.value(old.field.buffer.text());
                }
                a
            })
            .collect();
    }

    #[allow(dead_code)]
    pub fn area(&self, i: usize) -> &TextArea {
        &self.areas[i]
    }

    fn index_of(&self, id: WidgetId) -> Option<usize> {
        self.areas
            .iter()
            .position(|a| a.id == id || scrollbar::id_for(a.id) == id)
    }
}

impl Page for TextAreasPage {
    fn title(&self) -> &'static str {
        "Text areas"
    }
    fn blurb(&self) -> &'static str {
        "Multi-line editing, Unicode cursor motion, horizontal and vertical scrolling"
    }

    fn measure(&self, avail: Size, ctx: &RenderCtx) -> Size {
        self.tiled(avail.width, ctx).0.size
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
        self.handle_areas(ev, cx)
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

    fn editing(&self) -> bool {
        self.areas.iter().any(|a| a.editing)
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        let editing = focus
            .and_then(|f| self.index_of(f))
            .map(|i| self.areas[i].editing)
            .unwrap_or(false);
        if editing {
            vec![
                ("Enter", "Newline"),
                ("Esc", "Done"),
                ("Shift+↑↓", "Select"),
                ("Tab", "Next"),
            ]
        } else {
            vec![
                junie_tui::widgets::widget_hints::EDIT_PAIR,
                ("↑↓", "Scroll"),
            ]
        }
    }
}

impl TextAreasPage {
    fn tiled(&self, width: u16, ctx: &RenderCtx) -> (Tiled, [Tiled; 2]) {
        let any = Size::new(0, 0);

        let sizes: Vec<Size> = self
            .areas
            .iter()
            .map(|a| {
                let s = a.measure(any, ctx);
                Size::new(a.min_width(any, ctx), s.height)
            })
            .collect();
        let inner = |k: usize, w: u16| {
            let (a, b) = (sizes[2 * k], sizes[2 * k + 1]);
            let ha = |_: u16| a.height;
            let hb = |_: u16| b.height;
            tiles(
                w,
                &[&[Tile::new(a.width, &ha), Tile::new(b.width, &hb)]],
                (4, 1),
            )
        };
        let panels = [Self::panel(0), Self::panel(1)];
        let card_w = |k: usize| {
            let (a, b) = (sizes[2 * k], sizes[2 * k + 1]);
            panels[k].outer(Size::new(a.width.max(b.width), 0)).width
        };
        let card_h = |k: usize, w: u16| {
            let content = panels[k].inner(Size::new(w, 0)).width;
            panels[k]
                .outer(Size::new(content, inner(k, content).size.height))
                .height
        };
        let h0 = |w: u16| card_h(0, w);
        let h1 = |w: u16| card_h(1, w);
        let cards = tiles(
            width,
            &[&[Tile::new(card_w(0), &h0)], &[Tile::new(card_w(1), &h1)]],
            (0, 1),
        );
        let contents = [0, 1].map(|k| {
            let w = panels[k].inner(Size::new(cards.size.width, 0)).width;
            inner(k, w)
        });
        (cards, contents)
    }

    fn panel(k: usize) -> Panel<'static> {
        match k {
            0 => Panel::card(Some("Playground")).meta("Type to edit · Esc Done · Tab Next"),
            _ => Panel::card(Some("Disabled and error")),
        }
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let (cards, contents) = self.tiled(area.width, ctx);
        for (k, content) in contents.iter().enumerate() {
            let panel = Self::panel(k);
            let bg = panel.bg(t);
            let inner = panel.render(cards.at(k, area), buf, t);
            for i in 0..2 {
                let r = content.at(i, inner);
                self.areas[2 * k + i].render(r, buf, ctx, bg);
            }
        }
    }

    fn handle_areas(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let PageEvent::Blur(ch) = ev {
            let Some(i) = self.index_of(ch.from) else {
                return Outcome::Ignored;
            };
            return match self.areas[i].on_blur() {
                Some(InputAction::Committed) => {
                    cx.status("Saved");
                    Outcome::Changed
                }
                Some(_) => Outcome::Changed,
                None => Outcome::Ignored,
            };
        }
        match ev {
            PageEvent::Key(key) => {
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                let Some(i) = self.index_of(f) else {
                    return Outcome::Ignored;
                };
                let (out, iev) = self.areas[i].on_key(key);
                match iev {
                    Some(InputAction::CommittedTab { backward: false }) => cx.focus_next(),
                    Some(InputAction::CommittedTab { backward: true }) => cx.focus_prev(),
                    Some(InputAction::Committed) => cx.status("Saved"),
                    _ => {}
                }
                out
            }
            PageEvent::Paste(text) => {
                let Some(i) = cx.focus.current().and_then(|f| self.index_of(f)) else {
                    return Outcome::Ignored;
                };
                self.areas[i].on_paste(text)
            }
            PageEvent::Pointer(ev) => {
                for a in &mut self.areas {
                    if let Some(r) = a.route(ev) {
                        if ev.is(Phase::Click) && ev.bar.is_none() {
                            cx.focus.focus(a.id);
                        }
                        return r.outcome();
                    }
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .column_width(44)
        .rows(&[
            Rest, Hover, Focused, Editing, Error, Disabled, Empty, Overflow, Narrow,
        ])
        .column(
            "Text area",
            TextArea::new(id.child(0), "Notes", 2)
                .value("Read src/api/billing.rs first.\nKeep the public API stable."),
        )
}

use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::app::PageId;
use crate::options::page_options;
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use termrock::core::event::Outcome;
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::keymap::{KeyBinding, NavKeys};
use termrock::core::pointer::MouseTarget;
use termrock::core::scroll::TrackClick;
use termrock::core::text::TextBuffer;
use termrock::theme::Tone;
use termrock::ui::ctx::RenderCtx;
use termrock::ui::reveal::{Reveal, RevealMode};
use termrock::ui::specimen::{SpecimenState, StateGrid};
use termrock::ui::text::{self, DurationStyle, WrapOpts};
use termrock::widgets::panel::Panel;
use termrock::widgets::props::{self, Prop, PropsAction, PropsCmd, PropsList, Sheet};

const ID: WidgetId = WidgetId::of("textutilities");
const LIST: WidgetId = WidgetId::of("textutilities.props");

const SAMPLE: &str = "payments-platform/settlement-backoff";

const TRAIL: &str = "Tables › payments › settlements";

const PROSE: &str = "Wrapping keeps words whole and hard-wraps a word longer than the line.\n    An indented block keeps its indent on every continuation line.\nCtrl+F    find in the focused text; Enter steps to the next match, Shift+Enter back";

pub struct TextUtilitiesPage {
    template: Template,
    list: PropsList,

    opts: Vec<usize>,

    last: String,
}

fn list_props() -> Vec<Prop> {
    vec![
        Prop::new("Host", "db-primary.payments.internal").copyable(),
        Prop::new("Port", "5432").copyable(),
        Prop::new("Health", "healthy").tone(Tone::Success),
        Prop::new("Replication lag", "2.4 s").tone(Tone::Warning),
        Prop::new(
            "Connection string",
            "postgres://app@db-primary.payments.internal:5432/settlements?sslmode=verify-full",
        )
        .copyable(),
        Prop::new(
            "Notes",
            "Failover is manual: promote the replica, then repoint the pooler and restart the workers.",
        )
        .wrap(),
        Prop::new("Owner", "payments-platform"),
        Prop::new("Region", "eu-west-1"),
    ]
}

impl TextUtilitiesPage {
    pub fn new() -> Self {
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            list: PropsList::new(LIST, list_props()),
            opts: page_options(PageId::TextUtilities)
                .iter()
                .map(|o| o.default)
                .collect(),
            last: "—".into(),
        };
        page.list = page.build_list();
        page
    }

    #[allow(dead_code)]
    pub fn list(&self) -> &PropsList {
        &self.list
    }

    fn opt(&self, key: &str) -> &'static str {
        let table = page_options(PageId::TextUtilities);
        let i = table
            .iter()
            .position(|o| o.key == key)
            .unwrap_or_else(|| panic!("no option {key}"));
        table[i].values[self.opts[i]]
    }

    fn sheet(&self) -> Sheet {
        match self.opt("textutilities.sheet") {
            "narrow-labels" => Sheet {
                label_max_share: 25,
                value_min: 20,
            },
            _ => Sheet::default(),
        }
    }

    fn build_list(&self) -> PropsList {
        let reveal = match self.opt("textutilities.reveal") {
            "inline" => Reveal::default().mode(RevealMode::InlineWrap),
            "off" => Reveal::default().hover(false).keyboard(None),
            _ => Reveal::default(),
        };
        let mut list = PropsList::new(LIST, list_props())
            .nav(match self.opt("textutilities.nav") {
                "vim" => NavKeys::VIM,
                _ => NavKeys::default(),
            })
            .track_click(match self.opt("textutilities.track_click") {
                "jump" => TrackClick::Jump,
                _ => TrackClick::Page,
            })
            .scrollbar_gap(self.opt("textutilities.scrollbar_gap").parse().unwrap_or(1))
            .sheet(self.sheet())
            .reveal(reveal);
        if self.opt("textutilities.copy_key") == "c" {
            list = list.keymap(vec![(KeyBinding::char('c'), PropsCmd::Copy)]);
        }
        list.cursor = self.list.cursor.min(list.props.len().saturating_sub(1));
        list
    }

    fn list_readout(&self) -> String {
        let l = &self.list;
        let key = termrock::core::keymap::label_for(&l.keymap, PropsCmd::Copy)
            .unwrap_or_else(|| "none".into());
        let reveal = match l.reveal.config {
            Some(r) if r.keyboard.is_none() && !r.hover => "off".to_owned(),
            Some(r) => format!("{:?}", r.mode),
            None => "app default".into(),
        };
        format!(
            "PropsList · nav {} · copy {key} · track_click {:?} · scrollbar_gap {} · label share {}% · reveal {reveal}",
            if l.nav == NavKeys::VIM {
                "vim"
            } else {
                "default"
            },
            l.scroll.track_click.unwrap_or_default(),
            l.scrollbar_gap,
            l.sheet.label_max_share,
        )
    }

    fn helpers(&self, w: usize) -> Vec<Prop> {
        let style = match self.opt("textutilities.duration") {
            "compact" => DurationStyle::Compact,
            _ => DurationStyle::Spoken,
        };
        let case = self.opt("textutilities.case") == "on";
        let hay = "SELECT id FROM users; select name from users";
        let ranges: Vec<String> = text::find_ranges(hay, "select", case)
            .iter()
            .map(|r| format!("{}..{}", r.start, r.end))
            .collect();
        let durations = [
            Duration::from_millis(0),
            Duration::from_millis(840),
            Duration::from_millis(1_250),
            Duration::from_secs(182),
            Duration::from_secs(8_040),
        ]
        .map(|d| text::duration(d, style))
        .join(" · ");
        vec![
            Prop::new("truncate", text::truncate(SAMPLE, w)),
            Prop::new("truncate_middle", text::truncate_middle(SAMPLE, w)),
            Prop::new("truncate_sep", text::truncate_sep(TRAIL, w)),
            Prop::new("crumb_fit", text::crumb_fit(TRAIL, w)),
            Prop::new("slice_cells", text::slice_cells(SAMPLE, 9, w)),
            Prop::new("clip", text::clip(SAMPLE, w)),
            Prop::new("fit", format!("[{}]", text::fit("ok", w))),
            Prop::new("fit_right", format!("[{}]", text::fit_right("42", w))),
            Prop::new(
                "wider_than",
                format!(
                    "{} (width {})",
                    text::wider_than(SAMPLE, w),
                    text::width(SAMPLE)
                ),
            ),
            Prop::new("width_capped", text::width_capped(SAMPLE, w).to_string()),
            Prop::new(
                "width",
                format!("{} cells for 日本語 abc", text::width("日本語 abc")),
            ),
            Prop::new(
                "center_x",
                format!(
                    "\"ok\" starts at {} of {w}",
                    text::center_x(Rect::new(0, 0, w as u16, 1), "ok")
                ),
            ),
            Prop::new("thousands", text::thousands(1_234_567)),
            Prop::new(
                "plural",
                format!(
                    "{} · {}",
                    text::plural(1, "row", "rows"),
                    text::plural(3, "row", "rows")
                ),
            ),
            Prop::new(
                "bytes",
                [512, 1_536, 3_435_973_837].map(text::bytes).join(" · "),
            ),
            Prop::new(&format!("duration {style:?}"), durations),
            Prop::new(
                "find_ranges",
                format!(
                    "\"select\" {} → {}",
                    if case { "case-sensitive" } else { "any case" },
                    ranges.join(", ")
                ),
            ),
            Prop::new(
                "Prop::wrap",
                "a value marked wrap continues on the rows below instead of being cut at the edge",
            )
            .wrap(),
        ]
    }

    fn wrap_opts(&self) -> WrapOpts {
        match self.opt("textutilities.wrap") {
            "plain" => WrapOpts::PLAIN,
            "no-hang" => WrapOpts {
                hang_at_gap: None,
                ..WrapOpts::default()
            },
            "no-indent" => WrapOpts {
                keep_indent: false,
                ..WrapOpts::default()
            },
            _ => WrapOpts::default(),
        }
    }

    fn width(&self) -> usize {
        self.opt("textutilities.width").parse().unwrap_or(20)
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let (left, right) = crate::pages::layout::columns(area, 64.min(area.width * 11 / 20), 2);
        let w = self.width();

        let helpers = self.helpers(w);
        let title = format!("ui::text at width {w}");
        let panel = Panel::card(Some(&title)).meta("props::render_with");
        let bg = panel.bg(t);
        let sheet = self.sheet();
        let inner_w = left.width.saturating_sub(4);
        let h = props::measure_with(&helpers, inner_w, sheet) + 3;
        let card = Rect::new(left.x, left.y, left.width, h.min(left.height));
        let inner = panel.render(card, buf, t);
        props::render_with(inner, buf, t, &helpers, bg, sheet);

        let below = card.bottom() + 1;
        if below + 4 < left.bottom() {
            let ww = (w + 16).min(inner_w as usize);
            let opts = self.wrap_opts();
            let lines = text::wrap_with(PROSE, ww, opts);
            let title = format!(
                "wrap_with at {ww} · keep_indent {} · hang_at_gap {:?}",
                opts.keep_indent, opts.hang_at_gap
            );
            let panel = Panel::card(Some(&title));
            let bg = panel.bg(t);
            let h = (lines.len() as u16 + 3).min(left.bottom() - below);
            let inner = panel.render(Rect::new(left.x, below, left.width, h), buf, t);
            for (i, l) in lines.iter().enumerate() {
                let y = inner.y + i as u16;
                if y >= inner.bottom() {
                    break;
                }
                buf.set_string(inner.x, y, l, t.secondary().bg(bg));

                let edge = inner.x + ww as u16;
                if edge < inner.right() {
                    buf.set_string(edge, y, "┊", t.faint().bg(bg));
                }
            }
        }

        let mut y = right.y;
        let panel = Panel::card(Some("PropsList"))
            .meta("Enter activates · y copies")
            .focused(ctx.interaction.focused(LIST));
        let bg = panel.bg(t);
        let h = 13.min(right.height);
        let inner = panel.render(Rect::new(right.x, y, right.width, h), buf, t);
        self.list.render(inner, buf, ctx, bg);
        y += h;
        let last = format!(
            "last PropsAction: {}   (Activate(row) on Enter or a click · Copy(row) on the copy key)",
            self.last
        );
        for line in [self.list_readout(), last] {
            if y < right.bottom() {
                buf.set_string(
                    right.x,
                    y,
                    text::truncate(&line, right.width as usize),
                    t.faint().bg(t.canvas),
                );
            }
            y += 1;
        }
        y += 1;

        if y + 5 < right.bottom() {
            let mut b = TextBuffer::multi("SELECT id, name\nFROM users\nWHERE active");
            b.select_word_at(b.text().find("users").unwrap_or(0));
            let pos = b.cursor_pos();
            let panel = Panel::card(Some("TextBuffer · the model under every text control"));
            let bg = panel.bg(t);
            let inner = panel.render(Rect::new(right.x, y, right.width, 8), buf, t);
            let list = if self.opt("textutilities.buffer") == "geometry" {
                Self::geometry_facts(&mut b)
            } else {
                vec![
                    Prop::new("text", b.text().replace('\n', " ⏎ ")),
                    Prop::new(
                        "selection",
                        format!(
                            "{:?} \"{}\"",
                            b.selection(),
                            b.selected_text().unwrap_or("")
                        ),
                    ),
                    Prop::new("cursor_pos", format!("{pos:?}")),
                    Prop::new("selection_lines", format!("{:?}", b.selection_lines())),
                ]
            };
            props::render(inner, buf, t, &list, bg);
        }
    }
}

impl TextUtilitiesPage {
    /// What the buffer knows about its lines: where each one sits in the text,
    /// how wide the widest is, where an offset falls and whether an edit moved
    /// the revision a view caches against.
    fn geometry_facts(b: &mut TextBuffer) -> Vec<Prop> {
        let offset = b.text().find("users").unwrap_or(0);
        let range = b.line_range(b.position_at(offset).line);
        let shown = range.as_ref().map_or(String::new(), |r| {
            format!("{}..{} {}", r.start, r.end, &b.text()[r.clone()])
        });
        let facts = vec![
            Prop::new("position_at", {
                let at = b.position_at(offset);
                format!("{offset} → {}:{}", at.line, at.col)
            }),
            Prop::new("line_range", shown),
            Prop::new("max_line_width", format!("{} cells", b.max_line_width())),
        ];
        let before = b.revision();
        b.insert_at(0, "-- ");
        let moved = b.revision() != before;
        let mut facts = facts;
        facts.push(Prop::new("revision", format!("edit moves it: {moved}")));
        facts
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let rows = |n: usize| list_props().into_iter().take(n).collect::<Vec<_>>();
    StateGrid::new(id)
        .rows(&[Rest, Focused, Loading, Error, Overflow, Narrow])
        .column_width(48)
        .column("Facts", PropsList::new(id.child(0), rows(3)))
        .column(
            "Narrow labels",
            PropsList::new(id.child(1), rows(3)).sheet(Sheet {
                label_max_share: 25,
                value_min: 20,
            }),
        )
}

impl Page for TextUtilitiesPage {
    fn title(&self) -> &'static str {
        "Text utilities"
    }

    fn blurb(&self) -> &'static str {
        "Text helpers (truncation, wrapping, widths), property sheets and PropsList"
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
        let act = |page: &mut Self, a: Option<PropsAction>, cx: &mut Cx| {
            let Some(a) = a else { return };
            let p = &page.list.props[match a {
                PropsAction::Copy(i) | PropsAction::Activate(i) => i,
            }];
            match a {
                PropsAction::Copy(_) => {
                    cx.status(format!("Copied {}", p.label));
                    page.last = format!("Copy({})", p.label);
                }
                PropsAction::Activate(_) => {
                    cx.status(format!("{}: {}", p.label, p.value));
                    page.last = format!("Activate({})", p.label);
                }
            }
        };
        match ev {
            PageEvent::Key(key) if cx.focus.is(LIST) => {
                let (o, a) = self.list.on_key(key);
                act(self, a, cx);
                o
            }
            PageEvent::Pointer(ev) => {
                let Some(r) = self.list.route(ev) else {
                    return Outcome::Ignored;
                };
                if ev.is(Phase::Click) {
                    cx.focus.focus(LIST);
                }
                let (o, actions) = r.into_parts();
                for a in actions {
                    act(self, Some(a), cx);
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
        if focus == Some(LIST) {
            let copy = if self.opt("textutilities.copy_key") == "c" {
                "c"
            } else {
                "y"
            };
            return vec![("↑↓", "Move"), ("Enter", "Open"), (copy, "Copy")];
        }
        Vec::new()
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        let i = page_options(PageId::TextUtilities)
            .iter()
            .position(|o| o.key == key)?;
        Some(self.opts[i])
    }

    fn set_option(&mut self, key: &str, value: usize) {
        let table = page_options(PageId::TextUtilities);
        if let Some(i) = table.iter().position(|o| o.key == key) {
            self.opts[i] = value.min(table[i].values.len() - 1);
            self.list = self.build_list();
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

pub fn page() -> Box<dyn Page> {
    Box::new(TextUtilitiesPage::new())
}

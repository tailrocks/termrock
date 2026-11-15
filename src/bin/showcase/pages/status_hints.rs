use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::page_options;
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::cli::CommonArgs;
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::shell::StatusLine;
use junie_tui::theme::{BadgeKind, HintAlign, HintBarStyle, StatusKind, Theme, Tone};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::StackChild;
use junie_tui::ui::reveal::Reveal;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::ui::text::truncate;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::hintbar::{HintBar, HintLayer};
use junie_tui::widgets::keyhint::{self, Hint as KeyHint, HintPriority, hint, toggle_hint};
use junie_tui::widgets::progress::{ProgressBar, render_spinner};
use junie_tui::widgets::too_small::TooSmall;

const ID: WidgetId = WidgetId::of("statushints");

const NARROW: u16 = 44;

const LABEL_W: u16 = 22;

const BAR_W: u16 = 64;

const TOO_SMALL_W: u16 = 44;

const CAPTIONS: [&str; 9] = [
    "Layers · HintBar::resolve: the topmost layer that exists wins",
    "Alignment · HintAlign",
    "Priorities · HintPriority: normal hints drop from the right, pinned ones last",
    "Statuses · the right edge",
    "Toggle hints and the reveal key",
    "Status line · StatusLine: the shell's footer message",
    "Motion · --motion full | reduced | paused (Inspector: shell.motion)",
    "Terminal too small · TooSmall: what an app draws below its minimum size",
    "Command line · CommonArgs: every app flattens it into its own clap parser",
];

const AUTO_NOTE: &str = "Auto: pinned when the key is in theme.hintbar.pinned_keys (Esc) · Hint::pinned keeps F5 · Normal drops first";

const MOTION_NOTE: &str = "full: the spinner turns and the bar fills · reduced: the bar fills, the spinner keeps its first frame · paused: nothing moves; --frame N picks the tick";

fn ttl_note() -> String {
    format!(
        "information and warnings go after {} s (StatusLine::DEFAULT_TTL_MS) · an error stays until the next key, paste or click · --motion paused freezes the footer",
        StatusLine::<()>::DEFAULT_TTL_MS / 1000
    )
}

fn note_rows(text: &str, w: u16) -> u16 {
    junie_tui::ui::text::wrap(text, w.saturating_sub(2).max(1) as usize).len() as u16
}

fn note(area: Rect, buf: &mut Buffer, t: &Theme, y: u16, text: &str) -> u16 {
    let lines = junie_tui::ui::text::wrap(text, area.width.saturating_sub(2).max(1) as usize);
    for (i, l) in lines.iter().enumerate() {
        let y = y + i as u16;
        if y < area.bottom() {
            buf.set_string(area.x + 2, y, l, t.faint().bg(t.canvas));
        }
    }
    lines.len() as u16
}

fn hanging(line: &str, w: u16) -> Vec<String> {
    let w = w as usize;
    if junie_tui::ui::text::width(line) <= w {
        return vec![line.to_owned()];
    }
    let (flag, help) = line.split_at(ARG_COL.min(line.len()));
    let room = w.saturating_sub(ARG_COL).max(10);
    junie_tui::ui::text::wrap(help, room)
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                format!("{flag}{l}")
            } else {
                format!("{:ARG_COL$}{l}", "")
            }
        })
        .collect()
}

const ARG_COL: usize = 24;

fn shell_height(w: u16) -> u16 {
    let args: u16 = common_args()
        .iter()
        .map(|l| hanging(l, w.saturating_sub(2)).len() as u16)
        .sum();

    16 + note_rows(&ttl_note(), w) + note_rows(MOTION_NOTE, w) + args
}

const POSTS: [(&str, &str, Tone); 3] = [
    ("Post info", "Saved 3 rows", Tone::Secondary),
    ("Post warning", "2 rows skipped", Tone::Warning),
    ("Post error", "Connection lost", Tone::Error),
];

const JOB_STEP: f64 = 0.02;

pub struct StatusHintsPage {
    template: Template,

    opts: Vec<usize>,

    posts: Vec<Button>,

    run: Button,

    job: Option<f64>,
    sections: crate::pages::layout::Sections,
}

impl StatusHintsPage {
    pub fn new() -> Self {
        Self {
            template: Template::new(ID, states_grid()),
            opts: page_options(PageId::StatusHints)
                .iter()
                .map(|o| o.default)
                .collect(),
            posts: POSTS
                .iter()
                .enumerate()
                .map(|(i, (label, _, _))| Button::secondary(ID.sub("post").child(i), label))
                .collect(),
            run: Button::secondary(ID.sub("run"), "Run job"),
            job: None,
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        }
    }

    #[allow(dead_code)]
    pub fn job(&self) -> Option<f64> {
        self.job
    }

    fn too_small(&self, t: &Theme) -> TooSmall<'static> {
        let (w, h) = match self.opt("statushints.too_small_min") {
            "100x30" => (100, 30),
            _ => (TooSmall::MIN_WIDTH, TooSmall::MIN_HEIGHT),
        };
        let mut screen = TooSmall::new("junie showcase").min(w, h);
        if self.opt("statushints.quit_hint") == "custom" {
            screen = screen.quit_hint("q Quit · resize to continue");
        }
        if self.opt("statushints.brand") == "accent" {
            screen = screen.brand_style(t.accent_fg().bg(t.canvas));
        }
        screen
    }

    fn opt(&self, key: &str) -> &'static str {
        let table = page_options(PageId::StatusHints);
        let i = table
            .iter()
            .position(|o| o.key == key)
            .unwrap_or_else(|| panic!("no option {key}"));
        table[i].values[self.opts[i]]
    }

    fn screen_hints(&self) -> Vec<KeyHint> {
        let priority = match self.opt("statushints.priority") {
            "normal" => HintPriority::Normal,
            "pinned" => HintPriority::Pinned,
            _ => HintPriority::Auto,
        };
        vec![
            hint("Enter", "Open"),
            hint("Tab", "Next pane"),
            hint("/", "Filter"),
            toggle_hint(
                "Ctrl+H",
                self.opt("statushints.hidden") == "on",
                "Hide hidden",
                "Show hidden",
            ),
            hint("?", "Help"),
            KeyHint::new("F5", "Refresh").priority(priority),
        ]
    }

    fn layers(&self) -> [Option<HintLayer>; 3] {
        let top = self.opt("statushints.layer");
        let modal = (top == "modal")
            .then(|| HintLayer::new(vec![hint("Enter", "Delete"), hint("Esc", "Cancel")]));
        let menu = (top != "screen").then(|| {
            HintLayer::new(vec![
                hint("↑↓", "Move"),
                hint("Enter", "Choose"),
                hint("Esc", "Close"),
            ])
        });
        let mut screen = HintLayer::new(self.screen_hints());
        match self.opt("statushints.status") {
            "plain" => screen = screen.status("3 rows selected", Tone::Secondary),
            "warning" => screen = screen.status("2 unsaved changes", Tone::Warning),
            "error" => screen = screen.status("Connection lost", Tone::Error),
            "parts" => {
                screen = screen.status_parts(
                    vec![
                        ("saved ".to_owned(), Tone::Muted),
                        ("payments.settlements".to_owned(), Tone::Secondary),
                    ],
                    Tone::Secondary,
                )
            }
            _ => {}
        }
        if self.opt("statushints.badge") == "on" {
            screen = screen.badge("EDIT", BadgeKind::Edit);
        }
        if self.opt("statushints.align") == "center" {
            screen = screen.align(HintAlign::Center);
        }
        [modal, menu, Some(screen)]
    }

    fn style(&self, t: &Theme) -> HintBarStyle {
        HintBarStyle {
            compact: self.opt("statushints.compact") == "on",
            status_min_share: self.opt("statushints.share").parse().unwrap_or(50),
            ..t.hintbar
        }
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let focus = std::iter::once(self.run.id)
            .chain(self.posts.iter().map(|b| b.id))
            .any(|id| ctx.interaction.has_focus(id))
            .then_some(1);
        let t = ctx.theme;
        let (hints_h, shell_h) = (self.hints_height(area.width, t), shell_height(area.width));
        let l = self.sections.layout(
            area,
            &[StackChild::fixed(hints_h), StackChild::fixed(shell_h)],
            focus,
            buf,
            ctx,
        );
        if let Some(r) = l.slot(0) {
            self.render_hints(r, buf, ctx);
        }
        match l.slot(1) {
            Some(r) => self.render_shell(r, buf, ctx),
            None => {
                for b in &self.posts {
                    ctx.focus_stop(b.id);
                }
                ctx.focus_stop(self.run.id);
            }
        }
    }

    fn render_hints(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let style = self.style(t);
        let mut y = area.y;
        let caption = |buf: &mut Buffer, y: u16, text: &str| {
            if y < area.bottom() {
                let text = junie_tui::ui::text::truncate(text, area.width as usize);
                buf.set_string(area.x, y, text, t.title().bg(t.canvas));
            }
        };
        let label = |buf: &mut Buffer, y: u16, text: &str| {
            if y < area.bottom() {
                buf.set_string(area.x + 2, y, text, t.muted().bg(t.canvas));
            }
        };
        let row = |y: u16, w: u16| {
            let x = area.x + LABEL_W;
            Rect::new(x, y, w.min(area.right().saturating_sub(x)), 1)
        };
        let full = area.width.saturating_sub(LABEL_W);

        caption(
            buf,
            y,
            "Layers · HintBar::resolve: the topmost layer that exists wins",
        );
        y += 1;
        let layers = self.layers();
        for (name, layer) in ["modal", "menu", "screen"].iter().zip(&layers) {
            if y >= area.bottom() {
                break;
            }
            match layer {
                Some(l) => {
                    label(buf, y, &format!("{name} layer"));
                    let mut plain = l.clone();
                    plain.status = None;
                    plain.status_parts.clear();
                    keyhint::render_parts(
                        row(y, full),
                        buf,
                        t,
                        &plain.hints,
                        None,
                        None,
                        &[],
                        &HintBarStyle {
                            align: HintAlign::Start,
                            ..style
                        },
                    );
                }
                None => label(buf, y, &format!("{name} layer  (none)")),
            }
            y += 1;
        }
        let resolved = HintBar::resolve(&layers);
        let w = if self.opt("statushints.width") == "narrow" {
            NARROW
        } else {
            full
        };
        if y < area.bottom() {
            label(buf, y, "resolved →");
            let r = row(y, w);
            junie_tui::ui::ctx::fill(buf, r, ratatui::style::Style::new().bg(t.surface));
            let s = HintBarStyle {
                align: resolved.align.unwrap_or(style.align),
                ..style
            };
            keyhint::render_parts(
                r,
                buf,
                t,
                &resolved.hints,
                resolved.badge,
                resolved
                    .status
                    .as_ref()
                    .map(|(s, tone)| (s.as_str(), *tone)),
                &resolved.status_parts,
                &s,
            );
        }
        y += 1;
        y += note(area, buf, t, y, &self.style_text(t)) + 1;

        caption(buf, y, "Alignment · HintAlign");
        y += 1;
        let hints = [
            hint("↑↓", "Move"),
            hint("Enter", "Open"),
            hint("Esc", "Back"),
        ];
        for (name, align) in [
            ("HintAlign::Start", HintAlign::Start),
            ("HintAlign::Center", HintAlign::Center),
        ] {
            if y >= area.bottom() {
                break;
            }
            label(buf, y, name);
            keyhint::render_aligned(row(y, full.min(BAR_W)), buf, t, &hints, None, None, align);
            y += 1;
        }

        if y < area.bottom() {
            label(buf, y, "Theme::centred_hints");
            let centred = t.centred_hints();
            keyhint::render_parts(
                row(y, full.min(BAR_W)),
                buf,
                t,
                &hints,
                None,
                None,
                &[],
                &centred.hintbar,
            );
        }
        y += 2;

        caption(
            buf,
            y,
            "Priorities · HintPriority: normal hints drop from the right, pinned ones last",
        );
        y += 1;
        let ranked = [
            hint("Enter", "Open").priority(HintPriority::Normal),
            hint("Tab", "Next pane"),
            hint("Space", "Select").priority(HintPriority::Normal),
            hint("Esc", "Back"),
            hint("F5", "Refresh").pinned(),
        ];
        let all = keyhint::width(&ranked, None, None);
        for (name, w) in [("all fit", all), ("36 columns", 36), ("24 columns", 24)] {
            if y >= area.bottom() {
                break;
            }
            label(buf, y, name);
            keyhint::render(row(y, w), buf, t, &ranked, None, None);
            y += 1;
        }
        y += note(area, buf, t, y, AUTO_NOTE) + 1;

        caption(buf, y, "Statuses · the right edge");
        y += 1;
        let short = [hint("Ctrl+S", "Save"), hint("Esc", "Back")];
        if y < area.bottom() {
            label(buf, y, "render_status");
            keyhint::render_status(
                row(y, full.min(BAR_W)),
                buf,
                t,
                &short,
                None,
                Some(("Saved", Some(StatusKind::Ok))),
            );
        }
        y += 1;
        if y < area.bottom() {
            label(buf, y, "render_toned");
            keyhint::render_toned(
                row(y, full.min(BAR_W)),
                buf,
                t,
                &short,
                None,
                Some(("Connection lost", Tone::Error)),
            );
        }
        y += 1;
        if y < area.bottom() {
            label(buf, y, "EDIT badge");
            keyhint::render(
                row(y, full.min(BAR_W)),
                buf,
                t,
                &keyhint::EDIT_FIELD,
                Some(("EDIT", BadgeKind::Edit)),
                None,
            );
        }
        y += 2;

        caption(buf, y, "Toggle hints and the reveal key");
        y += 1;
        if y < area.bottom() {
            label(buf, y, "toggle_hint");
            let on = self.opt("statushints.hidden") == "on";
            keyhint::render(
                row(y, full.min(BAR_W)),
                buf,
                t,
                &[toggle_hint("Ctrl+H", on, "Hide hidden", "Show hidden")],
                None,
                Some(if on {
                    "hidden files shown"
                } else {
                    "hidden files hidden"
                }),
            );
        }
        y += 1;
        if y < area.bottom()
            && let Some(key) = Reveal::default().key_label()
        {
            label(buf, y, "reveal key");
            keyhint::render(
                row(y, full.min(BAR_W)),
                buf,
                t,
                &[hint("↑↓", "Move"), KeyHint::new(key, t.strings.reveal_open)],
                None,
                Some("shown while a row is cut"),
            );
        }
    }

    fn render_shell(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let bg = t.canvas;
        let mut y = area.y;
        let caption = |buf: &mut Buffer, y: u16, text: &str| {
            if y < area.bottom() {
                let text = truncate(text, area.width as usize);
                buf.set_string(area.x, y, text, t.title().bg(bg));
            }
        };

        caption(
            buf,
            y,
            "Status line · StatusLine: the shell's footer message",
        );
        y += 1;
        if y < area.bottom() {
            let mut x = area.x + 2;
            for b in &mut self.posts {
                let w = b.width();
                b.render(Rect::new(x, y, w, 1), buf, ctx, bg);
                x += w + 2;
            }
        }
        y += 1;
        y += note(area, buf, t, y, &ttl_note()) + 1;

        caption(
            buf,
            y,
            "Motion · --motion full | reduced | paused (Inspector: shell.motion)",
        );
        y += 1;
        if y < area.bottom() {
            let w = self.run.width();
            self.run
                .render(Rect::new(area.x + 2, y, w, 1), buf, ctx, bg);
            let x = area.x + 4 + w;
            let bar_w = area.right().saturating_sub(x).min(48);
            let label = match self.job {
                Some(p) => format!("Indexing {:>3} %", (p * 100.0).round() as u32),
                None => "Indexing idle".to_owned(),
            };
            render_spinner(Rect::new(x, y, 18, 1), buf, ctx, &label, bg);
            ProgressBar::new("", self.job.unwrap_or(0.0)).render(
                Rect::new(x + 18, y, bar_w.saturating_sub(18), 1),
                buf,
                ctx,
                bg,
            );
        }
        y += 1;
        y += note(area, buf, t, y, MOTION_NOTE) + 1;

        caption(
            buf,
            y,
            "Terminal too small · TooSmall: what an app draws below its minimum size",
        );
        y += 1;
        let screen = self.too_small(t);
        let boxed = Rect::new(
            area.x + 2,
            y,
            TOO_SMALL_W.min(area.width.saturating_sub(2)),
            7,
        )
        .intersection(area);
        if !boxed.is_empty() {
            let block = ratatui::widgets::Block::bordered()
                .border_style(t.faint().bg(bg))
                .style(ratatui::style::Style::new().bg(t.canvas));
            let inner = block.inner(boxed);
            ratatui::widgets::Widget::render(block, boxed, buf);
            screen.render(inner, buf, t);
        }
        y += 8;

        caption(
            buf,
            y,
            "Command line · CommonArgs: every app flattens it into its own clap parser",
        );
        y += 1;
        for line in common_args() {
            for l in hanging(&line, area.width.saturating_sub(2)) {
                if y < area.bottom() {
                    buf.set_string(area.x + 2, y, l, t.secondary().bg(bg));
                }
                y += 1;
            }
        }
    }

    fn style_text(&self, t: &Theme) -> String {
        let s = self.style(t);
        let resolved = HintBar::resolve(&self.layers());
        format!(
            "HintBarStyle · align {:?} · status_min_share {}% · compact {} · pinned_keys {}",
            resolved.align.unwrap_or(s.align),
            s.status_min_share,
            if s.compact { "on" } else { "off" },
            s.pinned_keys.join(" "),
        )
    }

    fn hints_height(&self, w: u16, t: &Theme) -> u16 {
        24 + note_rows(&self.style_text(t), w) + note_rows(AUTO_NOTE, w)
    }

    fn natural_width(&self) -> u16 {
        let bar = |l: &HintLayer| {
            let status = l
                .status
                .as_ref()
                .map(|(s, _)| format!("! {s}"))
                .or_else(|| {
                    (!l.status_parts.is_empty())
                        .then(|| l.status_parts.iter().map(|(p, _)| p.as_str()).collect())
                });
            keyhint::width(&l.hints, l.badge.map(|(b, _)| b), status.as_deref())
        };
        let layers = self.layers();
        let bars = layers
            .iter()
            .flatten()
            .map(bar)
            .chain(std::iter::once(bar(&HintBar::resolve(&layers))))
            .chain([BAR_W, NARROW])
            .max()
            .unwrap_or(BAR_W);
        let captions = CAPTIONS
            .iter()
            .map(|c| junie_tui::ui::text::width(c) as u16)
            .max()
            .unwrap_or(0);
        (LABEL_W + bars).max(captions).max(2 + TOO_SMALL_W)
    }

    fn post(i: usize, cx: &mut Cx) {
        let (_, text, tone) = POSTS[i];
        cx.status_in(text, tone);
    }
}

fn common_args() -> Vec<String> {
    use clap::{Args, Command};
    let cmd = CommonArgs::augment_args(Command::new("app"));
    cmd.get_arguments()
        .map(|a| {
            let mut flag = String::new();
            if let Some(s) = a.get_short() {
                flag.push_str(&format!("-{s}, "));
            }
            if let Some(l) = a.get_long() {
                flag.push_str(&format!("--{l}"));
            }
            if let Some(v) = a.get_value_names().and_then(|v| v.first()) {
                flag.push_str(&format!(" <{v}>"));
            }
            let help = a.get_help().map(|h| h.to_string()).unwrap_or_default();
            let values: Vec<String> = a
                .get_possible_values()
                .iter()
                .filter(|v| !v.is_hide_set())
                .map(|v| v.get_name().to_owned())
                .collect();
            let values = if values.is_empty() {
                String::new()
            } else {
                format!("  [{}]", values.join(" | "))
            };
            format!("{flag:<24}{help}{values}")
        })
        .collect()
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let hints = vec![
        hint("↑↓", "Move"),
        hint("Enter", "Open"),
        hint("/", "Filter"),
        hint("Esc", "Back"),
    ];
    StateGrid::new(ID.sub("states"))
        .rows(&[Rest, Overflow, Error, Editing, Narrow])
        .column_width(40)
        .column("Start", HintLayer::new(hints.clone()))
        .column(
            "Center",
            HintLayer::new(hints.clone()).align(HintAlign::Center),
        )
        .column(
            "Pinned F5",
            HintLayer::new(
                hints
                    .iter()
                    .cloned()
                    .chain([hint("F5", "Refresh").pinned()])
                    .collect(),
            ),
        )
}

impl Page for StatusHintsPage {
    fn title(&self) -> &'static str {
        "Status & hints"
    }

    fn blurb(&self) -> &'static str {
        "The footer hint bar: layers, alignment, priorities, statuses, badges"
    }

    fn measure(&self, avail: Size, ctx: &RenderCtx) -> Size {
        let w = avail.width.max(self.natural_width());
        let h = self.hints_height(w, ctx.theme) + 1 + shell_height(w);
        Size::new(w, h)
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
        if let PageEvent::Tick = ev {
            return match self.job {
                Some(p) if p < 1.0 => {
                    self.job = Some((p + JOB_STEP).min(1.0));
                    if self.job == Some(1.0) {
                        cx.status_as(StatusKind::Ok, "Indexing finished");
                    }
                    Outcome::Changed
                }
                _ => Outcome::Ignored,
            };
        }
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
                if let Some(i) = self.posts.iter().position(|b| cx.focus.is(b.id)) {
                    let (o, act) = self.posts[i].on_key(key);
                    if act {
                        Self::post(i, cx);
                    }
                    return o;
                }
                if cx.focus.is(self.run.id) {
                    let (o, act) = self.run.on_key(key);
                    if act {
                        self.job = Some(0.0);
                    }
                    return o;
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => {
                for i in 0..self.posts.len() {
                    if let Some(r) = self.posts[i].route(ev)
                        && r.outcome() != Outcome::Ignored
                    {
                        if r.has_actions() {
                            Self::post(i, cx);
                        }
                        return Outcome::Changed;
                    }
                }
                if let Some(r) = self.run.route(ev)
                    && r.outcome() != Outcome::Ignored
                {
                    if r.has_actions() {
                        self.job = Some(0.0);
                    }
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self) -> bool {
        self.job.is_some_and(|p| p < 1.0)
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if focus.is_some_and(|f| f == self.run.id || self.posts.iter().any(|b| b.id == f)) {
            return vec![("Enter", "Activate")];
        }
        Vec::new()
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        let i = page_options(PageId::StatusHints)
            .iter()
            .position(|o| o.key == key)?;
        Some(self.opts[i])
    }

    fn set_option(&mut self, key: &str, value: usize) {
        let table = page_options(PageId::StatusHints);
        if let Some(i) = table.iter().position(|o| o.key == key) {
            self.opts[i] = value.min(table[i].values.len() - 1);
        }
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }
}

pub fn page() -> Box<dyn Page> {
    Box::new(StatusHintsPage::new())
}

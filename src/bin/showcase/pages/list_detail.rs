mod hosted;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

use self::hosted::{JobsHost, paint_row};
use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::Focus;
use junie_tui::core::id::{ItemKey, Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::list::{KeyedRowSource, RowSource, SourceStamp};
use junie_tui::core::pointer::PointerEvent;
use junie_tui::theme::{ButtonKind, StatusKind, Tone};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::Split;
use junie_tui::ui::specimen::StateGrid;
use junie_tui::ui::text::fit;
use junie_tui::widgets::empty::{self, EmptyState};
use junie_tui::widgets::list_detail::{
    Action, Event, Line, ListDetail, ListDetailSource, ListRowRef, SourceFrame,
};
use junie_tui::widgets::notice::Notice;

const ID: WidgetId = WidgetId::of("listdetail");
const LIST: WidgetId = ID.sub("list");
const DETAIL: WidgetId = ID.sub("detail");

#[derive(Clone, Copy)]
struct Job {
    name: &'static str,
    state: StatusKind,
    word: &'static str,
    tone: Tone,
    summary: &'static str,
    faint: bool,
    bindings: &'static [(&'static str, &'static str, &'static str)],
    problem: Option<&'static str>,
}

const JOBS: &[Job] = &[
    Job {
        name: "nightly-backup",
        state: StatusKind::Ok,
        word: "ok",
        tone: Tone::Success,
        summary: "Copies the project volumes to the archive store every night at 02:00, keeps 14 daily and 8 weekly copies and verifies each copy by reading it back before it prunes the oldest one.",
        faint: false,
        bindings: &[
            ("volume", "projects", "read-only"),
            ("volume", "uploads", "read-only"),
            ("secret", "ARCHIVE_TOKEN", "from the keychain"),
            ("network", "archive.internal", "port 443 only"),
        ],
        problem: None,
    },
    Job {
        name: "image-resize",
        state: StatusKind::Fail,
        word: "failing",
        tone: Tone::Error,
        summary: "Resizes uploaded images.",
        faint: false,
        bindings: &[
            ("volume", "uploads", "read-write"),
            ("secret", "CDN_KEY", "missing"),
        ],
        problem: Some("The secret CDN_KEY is not set, so uploads cannot be published."),
    },
    Job {
        name: "report-mailer",
        state: StatusKind::Warn,
        word: "slow",
        tone: Tone::Warning,
        summary: "Mails the weekly report to the team lists.",
        faint: false,
        bindings: &[("secret", "SMTP_PASSWORD", "from the keychain")],
        problem: Some("The last run took 9 minutes, three times its usual time."),
    },
    Job {
        name: "cache-warmer",
        state: StatusKind::Skipped,
        word: "paused",
        tone: Tone::Muted,
        summary: "Requests the hot pages after every deploy.",
        faint: true,
        bindings: &[],
        problem: None,
    },
    Job {
        name: "search-index",
        state: StatusKind::Ok,
        word: "ok",
        tone: Tone::Success,
        summary: "Rebuilds the search index when the catalog changes.",
        faint: false,
        bindings: &[("volume", "catalog", "read-only")],
        problem: None,
    },
];

const EXTRA: [&str; 19] = [
    "log-rotate",
    "db-vacuum",
    "cert-renew",
    "queue-drain",
    "thumb-gc",
    "invoice-run",
    "audit-export",
    "dns-sync",
    "backup-verify",
    "metrics-roll",
    "temp-sweep",
    "mail-digest",
    "feed-import",
    "session-prune",
    "sitemap-build",
    "geo-refresh",
    "link-check",
    "font-subset",
    "tag-recount",
];

const fn routine(name: &'static str) -> Job {
    Job {
        name,
        state: StatusKind::Ok,
        word: "ok",
        tone: Tone::Success,
        summary: "A routine housekeeping job.",
        faint: false,
        bindings: &[],
        problem: None,
    }
}

/// The five jobs and nineteen routine ones: more rows than a short terminal
/// shows, so the master scrolls.
static MANY: [Job; 24] = {
    let mut jobs = [routine("log-rotate"); 24];
    let mut i = 0;
    while i < JOBS.len() {
        jobs[i] = JOBS[i];
        i += 1;
    }
    let mut j = 0;
    while j < EXTRA.len() {
        jobs[JOBS.len() + j] = routine(EXTRA[j]);
        j += 1;
    }
    jobs
};

/// The jobs as a keyed, borrowed provider: rows read the static records in
/// place and are identified by job name, never by position.
struct JobRows {
    jobs: &'static [Job],
    reversed: bool,
    data: u64,
}

impl JobRows {
    /// The job shown at visible row `visible`: the order option reverses the
    /// view, never the keys.
    fn job(&self, visible: usize) -> &'static Job {
        let at = if self.reversed {
            self.jobs.len() - 1 - visible
        } else {
            visible
        };
        &self.jobs[at]
    }

    fn position(&self, key: ItemKey) -> Option<usize> {
        (0..self.jobs.len()).find(|&i| self.key(i) == key)
    }
}

impl RowSource for JobRows {
    fn len(&self) -> usize {
        self.jobs.len()
    }

    fn label(&self, i: usize) -> &str {
        self.job(i).name
    }

    fn key(&self, i: usize) -> ItemKey {
        ItemKey::text(self.job(i).name)
    }
}

impl KeyedRowSource for JobRows {
    fn stamp(&self) -> SourceStamp {
        SourceStamp {
            instance: ItemKey::text("showcase.list_detail.jobs"),
            data: self.data,
            view: 0,
        }
    }

    fn source_len(&self) -> usize {
        self.jobs.len()
    }

    fn source_key(&self, source_index: usize) -> ItemKey {
        self.key(source_index)
    }
}

impl ListDetailSource for JobRows {
    fn row(&self, visible: usize) -> ListRowRef<'_> {
        let job = self.job(visible);
        ListRowRef {
            mark: Some(job.state),
            label: job.name,
            detail: job.summary,
            meta: job.word,
            meta_tone: job.tone,
            faint: job.faint,
        }
    }
}

pub fn page() -> Box<dyn Page> {
    Box::new(ListDetailPage::new())
}

pub struct ListDetailPage {
    template: Template,
    opts: PageValues,
    ld: ListDetail,
    log: Option<String>,
    /// The selected job for the painters that own the selection.
    chosen: ItemKey,
}

/// Where the selection lives: in the list, or in the page by job key.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Painter {
    Standard,
    Rows,
    Hosted,
}

impl ListDetailPage {
    fn new() -> Self {
        Self {
            template: Template::new(ID, StateGrid::new(ID.sub("states"))),
            opts: PageValues::new(page_options(PageId::ListDetail)),
            ld: ListDetail::new(LIST, DETAIL, Split::new(50, 36, 40)),
            log: None,
            chosen: ItemKey::text(JOBS[0].name),
        }
    }

    fn painter(&self) -> Painter {
        match self.opts.value("listdetail.painter") {
            "rows" => Painter::Rows,
            "hosted" => Painter::Hosted,
            _ => Painter::Standard,
        }
    }

    fn jobs(&self) -> &'static [Job] {
        match self.opts.value("listdetail.data") {
            "none" => &[],
            "many" => &MANY,
            _ => JOBS,
        }
    }

    fn rows(&self) -> JobRows {
        let data = self.opts.get("listdetail.data").unwrap_or(0) as u64;
        let reversed = self.opts.value("listdetail.order") == "reversed";
        JobRows {
            jobs: self.jobs(),
            reversed,
            data: data | u64::from(reversed) << 2,
        }
    }

    fn lines(job: &Job) -> Vec<Line> {
        let mut lines = vec![
            Line::Prop("State".into(), job.word.into(), job.tone),
            Line::Prop("Summary".into(), job.summary.into(), Tone::Normal),
            Line::Blank,
        ];
        if let Some(problem) = job.problem {
            lines.push(Line::Notice(if job.state == StatusKind::Fail {
                Notice::error(problem)
            } else {
                Notice::warning(problem)
            }));
            lines.push(Line::Blank);
        }
        if job.bindings.is_empty() {
            lines.push(Line::Text("No bindings".into(), Tone::Muted));
        } else {
            lines.push(Line::Heading(
                "Bindings".into(),
                format!("{} listed", job.bindings.len()),
            ));
            lines.extend(job.bindings.iter().map(|(kind, name, how)| Line::Item {
                mark: None,
                cols: [(*kind).into(), (*name).into(), (*how).into()],
                tone: Tone::Normal,
            }));
        }
        lines
    }

    fn actions(&self, job: &Job) -> Vec<Action> {
        if self.opts.value("listdetail.actions") == "hidden" {
            return vec![];
        }
        vec![
            Action::new("run", "Run now", ButtonKind::Secondary),
            Action::new("pause", "Pause", ButtonKind::Secondary)
                .disabled(job.name == "cache-warmer"),
            Action::new("remove", "Remove", ButtonKind::Danger),
        ]
    }

    fn empty() -> EmptyState {
        EmptyState::new("No jobs yet").hint("Jobs you add are listed here with their bindings.")
    }

    fn fire(&mut self, event: Option<Event>) {
        self.log = match event {
            Some(Event::Action(name)) => Some(format!("action {name}")),
            Some(Event::Item(i)) => Some(format!("binding {} chosen", i + 1)),
            Some(Event::Row(i)) => Some(format!("{} selected", self.rows().job(i).name)),
            None => return,
        };
    }

    /// The selected visible row of a painter that owns the selection by job
    /// key; a key that is gone falls back to the first row.
    fn chosen_row(&mut self, rows: &JobRows) -> Option<usize> {
        if rows.len() == 0 {
            return None;
        }
        Some(rows.position(self.chosen).unwrap_or_else(|| {
            self.chosen = rows.key(0);
            0
        }))
    }

    /// Master keys for a painter that owns the selection: the list does not
    /// move a cursor it does not hold, so the page moves the chosen key by the
    /// page size the list reports.
    fn move_chosen(&mut self, key: &Key, focus: &mut Focus, rows: &JobRows) -> Option<Outcome> {
        let len = rows.len();
        if !key.plain() || len == 0 {
            return None;
        }
        let at = self.chosen_row(rows).unwrap_or(0);
        let page = self.ld.master_page();
        let to = match key.code {
            KeyCode::Up | KeyCode::Char('k') => at.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => (at + 1).min(len - 1),
            KeyCode::PageUp => at.saturating_sub(page),
            KeyCode::PageDown => (at + page).min(len - 1),
            KeyCode::Home | KeyCode::Char('g') => 0,
            KeyCode::End | KeyCode::Char('G') => len - 1,
            KeyCode::Enter | KeyCode::Tab => {
                self.ld.open_detail(focus);
                return Some(Outcome::Changed);
            }
            _ => return None,
        };
        if to != at {
            self.chosen = rows.key(to);
            self.ld.reset_detail();
        }
        Some(Outcome::Changed)
    }

    fn key(&mut self, key: &Key, focus: &mut Focus) -> Outcome {
        let rows = self.rows();
        if self.painter() == Painter::Standard {
            let (o, event) = self.ld.on_key(key, focus, rows.len());
            self.fire(event);
            return o;
        }
        let current = focus.current();
        let in_detail = self.ld.on_detail(current) || self.ld.actions_focused(current);
        if !in_detail && let Some(o) = self.move_chosen(key, focus, &rows) {
            return o;
        }
        let (o, events) = self.ld.on_detail_key(key, focus).into_parts();
        for event in events {
            self.fire(Some(event));
        }
        o
    }

    fn click(&mut self, ev: &PointerEvent, focus: &mut Focus) -> Outcome {
        let (o, event) = if self.painter() == Painter::Standard {
            self.ld.click_source(ev.id, focus)
        } else if let Some(key) = ev.key_of(Part::ROW) {
            self.chosen = key;
            self.ld.reset_detail();
            self.ld.close_detail(focus);
            let at = self.rows().position(key);
            (Outcome::Changed, at.map(Event::Row))
        } else {
            self.ld.click(ev.id, focus, 0)
        };
        self.fire(event);
        o
    }

    fn render_playground(&mut self, body: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let rows = self.rows();
        let painter = self.painter();
        let selected = if painter == Painter::Standard {
            let bound = self.ld.bind_source(&rows);
            bound
                .ok()
                .map(|()| self.ld.cursor)
                .filter(|&i| i < rows.len())
        } else {
            self.chosen_row(&rows)
        };
        let job = selected.map(|i| rows.job(i));
        let readout = self.opts.on("listdetail.readout") && body.height > 1;
        let list = Rect {
            height: body.height - u16::from(readout),
            ..body
        };
        let width = self
            .opts
            .number("listdetail.width")
            .map_or(list.width, |w| w.min(list.width));
        let owner = Rect { width, ..list };
        if painter == Painter::Hosted {
            let mut host = JobsHost::new(&rows, job);
            self.ld.render_hosted(
                owner,
                rows.len(),
                |i| rows.key(i),
                selected,
                buf,
                ctx,
                &mut host,
            );
        } else if let (Some(job), Some(selected)) = (job, selected) {
            self.render_framed(owner, &rows, (job, selected), buf, ctx);
        } else {
            ctx.control(LIST, Rect::new(list.x, list.y, 1, 1), false);
            empty::render_ctx(list, buf, ctx, &Self::empty(), ctx.theme.canvas);
        }
        if readout {
            let line = Rect::new(body.x, list.bottom(), body.width, 1);
            self.paint_readout(line, selected.zip(job), rows.len(), buf, ctx);
        }
    }

    /// One line under the list with what `ListDetail` reports after the frame:
    /// the keyed selection, the rows a page key moves by, the first visible
    /// row and the drawer.
    fn paint_readout(
        &self,
        line: Rect,
        selected: Option<(usize, &Job)>,
        len: usize,
        buf: &mut Buffer,
        ctx: &RenderCtx,
    ) {
        let held = match self.painter() {
            Painter::Standard => self.ld.selected_key(),
            _ => Some(self.chosen),
        };
        let row = match selected {
            Some((at, job)) if held == Some(ItemKey::text(job.name)) => {
                format!("row {} of {len} · keyed to this job", at + 1)
            }
            Some((at, _)) => format!("row {} of {len} · not keyed", at + 1),
            None => "no row".to_owned(),
        };
        let text = format!(
            "{row} · page {} · first {} · drawer {}",
            self.ld.master_page(),
            self.ld.master_offset() + 1,
            if self.ld.drawer_open() {
                "open"
            } else {
                "closed"
            }
        );
        buf.set_string(
            line.x + 1,
            line.y,
            fit(&text, usize::from(line.width.saturating_sub(2))),
            ctx.theme.faint().bg(ctx.theme.canvas),
        );
    }

    fn render_framed(
        &mut self,
        owner: Rect,
        rows: &JobRows,
        (job, selected): (&Job, usize),
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
    ) {
        let meta = format!("{} listed", rows.len());
        let lines = Self::lines(job);
        let actions = self.actions(job);
        let frame = SourceFrame {
            title: "Jobs",
            meta: &meta,
            head: &[],
            under: self.log.as_deref(),
            detail_title: job.name,
            detail_meta: job.word,
            lines: &lines,
            actions: &actions,
        };
        if self.painter() == Painter::Standard {
            if self
                .ld
                .render_source(owner, rows, &frame, buf, ctx)
                .is_err()
            {
                ctx.control(LIST, Rect::new(owner.x, owner.y, 1, 1), false);
            }
        } else {
            self.ld.render_source_selected_with(
                owner,
                rows,
                &frame,
                Some(selected),
                buf,
                ctx,
                paint_row,
            );
        }
    }
}

impl Page for ListDetailPage {
    fn title(&self) -> &'static str {
        "List & detail"
    }

    fn blurb(&self) -> &'static str {
        "A list beside its detail · the detail scrolls, lists items and ends in actions · below 100 columns the detail is a drawer"
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
        match ev {
            PageEvent::Key(key) => self.key(key, cx.focus),
            PageEvent::Pointer(ev) => {
                if let Some(o) = self.ld.on_pointer(ev) {
                    return o;
                }
                if let Some((axis, delta)) = ev.wheel() {
                    return match axis {
                        junie_tui::core::hit::Axis::Vertical => self.ld.wheel_event(ev, delta),
                        junie_tui::core::hit::Axis::Horizontal => Outcome::Ignored,
                    };
                }
                match ev.phase() {
                    Some(Phase::Click) => self.click(ev, cx.focus),
                    _ => Outcome::Ignored,
                }
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if self.ld.actions_focused(focus) {
            vec![("←→", "Choose"), ("Enter", "Run"), ("Esc", "Back")]
        } else if self.ld.on_detail(focus) {
            vec![
                ("↑↓", "Move"),
                ("Enter", "Choose"),
                ("Tab", "Actions"),
                ("Esc", "Back"),
            ]
        } else {
            vec![("↑↓", "Move"), ("Enter", "Details")]
        }
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
            self.ld.clamp(self.jobs().len());
        }
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }
}

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::list::{KeyedRowSource, RowSource, SourceStamp};
use junie_tui::theme::{ButtonKind, StatusKind, Tone};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::Split;
use junie_tui::ui::specimen::StateGrid;
use junie_tui::widgets::empty::{self, EmptyState};
use junie_tui::widgets::list_detail::{
    Action, Event, Line, ListDetail, ListDetailSource, ListRowRef, SourceFrame,
};
use junie_tui::widgets::notice::Notice;

const ID: WidgetId = WidgetId::of("listdetail");
const LIST: WidgetId = ID.sub("list");
const DETAIL: WidgetId = ID.sub("detail");

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

/// The jobs as a keyed, borrowed provider: rows read the static records in
/// place and are identified by job name, never by position.
struct JobRows {
    jobs: &'static [Job],
    data: u64,
}

impl RowSource for JobRows {
    fn len(&self) -> usize {
        self.jobs.len()
    }

    fn label(&self, i: usize) -> &str {
        self.jobs[i].name
    }

    fn key(&self, i: usize) -> ItemKey {
        ItemKey::text(self.jobs[i].name)
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
        let job = &self.jobs[visible];
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
}

impl ListDetailPage {
    fn new() -> Self {
        Self {
            template: Template::new(ID, StateGrid::new(ID.sub("states"))),
            opts: PageValues::new(page_options(PageId::ListDetail)),
            ld: ListDetail::new(LIST, DETAIL, Split::new(50, 36, 40)),
            log: None,
        }
    }

    fn jobs(&self) -> &'static [Job] {
        if self.opts.value("listdetail.data") == "none" {
            &[]
        } else {
            JOBS
        }
    }

    fn rows(&self) -> JobRows {
        JobRows {
            jobs: self.jobs(),
            data: u64::from(self.opts.value("listdetail.data") == "none"),
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

    fn actions(&self) -> Vec<Action> {
        if self.opts.value("listdetail.actions") == "hidden" {
            return vec![];
        }
        vec![
            Action::new("run", "Run now", ButtonKind::Secondary),
            Action::new("pause", "Pause", ButtonKind::Secondary).disabled(self.ld.cursor == 3),
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
            Some(Event::Row(i)) => Some(format!("{} selected", JOBS[i].name)),
            None => return,
        };
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
            Section::Playground => {
                let rows = self.rows();
                let bound = self.ld.bind_source(&rows);
                let Some(job) = bound.ok().and_then(|()| rows.jobs.get(self.ld.cursor)) else {
                    ctx.control(LIST, Rect::new(body.x, body.y, 1, 1), false);
                    empty::render_ctx(body, buf, ctx, &Self::empty(), ctx.theme.canvas);
                    return;
                };
                let meta = format!("{} listed", rows.len());
                let lines = Self::lines(job);
                let actions = self.actions();
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
                if self
                    .ld
                    .render_source(body, &rows, &frame, buf, ctx)
                    .is_err()
                {
                    ctx.control(LIST, Rect::new(body.x, body.y, 1, 1), false);
                }
            }
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
        let len = self.jobs().len();
        match ev {
            PageEvent::Key(key) => {
                let (o, event) = self.ld.on_key(key, cx.focus, len);
                self.fire(event);
                o
            }
            PageEvent::Pointer(ev) => {
                if let Some(o) = self.ld.on_pointer(ev) {
                    return o;
                }
                if let Some((axis, delta)) = ev.wheel() {
                    return match axis {
                        junie_tui::core::hit::Axis::Vertical => self.ld.wheel(ev.id, delta),
                        junie_tui::core::hit::Axis::Horizontal => Outcome::Ignored,
                    };
                }
                match ev.phase() {
                    Some(Phase::Click) => {
                        let (o, event) = self.ld.click_source(ev.id, cx.focus);
                        self.fire(event);
                        o
                    }
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

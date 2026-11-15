use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::{Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::button::{Button, row_layout};
use junie_tui::widgets::diff::{DiffFile, DiffHunk, DiffLine, DiffMode, DiffStatus, DiffView};
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::viewport::ViewportAction;

const ID: WidgetId = WidgetId::of("diff");

const LIST_W: u16 = 38;

pub struct DiffPage {
    template: Template,
    opts: PageValues,
    view: DiffView,
    files: ListBox,
    review: Button,
    empty: Button,
    copied: Option<String>,
}

fn sample() -> DiffFile {
    DiffFile {
        path: "config/service.toml".into(),
        status: DiffStatus::Modified,
        hunks: (0..5)
            .map(|i| DiffHunk {
                old_start: i * 8 + 1,
                new_start: i * 9 + 1,
                lines: vec![
                    DiffLine::context(format!("[service.worker_{i}]")),
                    DiffLine::remove("attempts = 3"),
                    DiffLine::add("attempts = 5"),
                    DiffLine::add("backoff = \"exponential\""),
                    DiffLine::context("region = \"東京\""),
                    DiffLine::context("label = \"cafe\u{301} ☕\""),
                    DiffLine::context("endpoint = \"https://api.example.test/workers/health\""),
                    DiffLine::context("enabled = true"),
                ],
            })
            .collect(),
    }
}

fn changes() -> Vec<DiffFile> {
    vec![
        sample(),
        DiffFile {
            path: "docs/retries.md".into(),
            status: DiffStatus::Added,
            hunks: vec![DiffHunk {
                old_start: 0,
                new_start: 1,
                lines: vec![
                    DiffLine::add("# Retries"),
                    DiffLine::add(""),
                    DiffLine::add("Workers retry five times with exponential backoff."),
                ],
            }],
        },
        DiffFile {
            path: "src/legacy_backoff.rs".into(),
            status: DiffStatus::Deleted,
            hunks: vec![DiffHunk {
                old_start: 1,
                new_start: 0,
                lines: vec![
                    DiffLine::remove("pub fn linear(attempt: u32) -> u64 {"),
                    DiffLine::remove("    200 * attempt as u64"),
                    DiffLine::remove("}"),
                ],
            }],
        },
        DiffFile {
            path: "src/backoff/retry.rs".into(),
            status: DiffStatus::Renamed {
                from: "src/retry.rs".into(),
            },
            hunks: vec![DiffHunk {
                old_start: 1,
                new_start: 1,
                lines: vec![
                    DiffLine::remove("use crate::legacy_backoff::linear;"),
                    DiffLine::add("use super::exponential;"),
                    DiffLine::context("pub async fn fetch(url: &str) -> Result<Body, Error> {"),
                ],
            }],
        },
    ]
}

impl DiffPage {
    pub fn new() -> Self {
        let items = changes()
            .iter()
            .map(|f| {
                ListItem::new(&f.header()).meta(&format!("+{} −{}", f.additions(), f.deletions()))
            })
            .collect();
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::Diff)),
            view: DiffView::new(ID.sub("view")),
            files: ListBox::new(ID.sub("files"), items, SelectMode::Single),
            review: Button::toggle(ID.sub("review"), "Review", false),
            empty: Button::toggle(ID.sub("empty"), "Empty", false),
            copied: None,
        };
        page.update_controls();
        page
    }

    fn update_controls(&mut self) {
        let review = self.review.on == Some(true);
        if review != (self.view.mode == DiffMode::Review) {
            self.view.set_mode(self.view.mode.toggled());
        }
        let none = self.empty.on == Some(true) || self.opts.value("diff.file") == "none";
        self.view.set_file(if none {
            None
        } else {
            changes().into_iter().nth(self.files.cursor)
        });
        self.view.term.clear_selection();
        self.copied = None;
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let controls = Rect::new(area.x, area.y, area.width, area.height.min(1));
        let rects = row_layout(controls, &[self.review.width(), self.empty.width()], 2);
        self.review.render(rects[0], buf, ctx, ctx.theme.canvas);
        self.empty.render(rects[1], buf, ctx, ctx.theme.canvas);
        let body = Rect::new(
            area.x,
            area.y.saturating_add(2).min(area.bottom()),
            area.width,
            area.height.saturating_sub(2),
        );

        let body = match self.opts.value("diff.width") {
            "narrow" => Rect::new(body.x, body.y, body.width.min(48), body.height),
            _ => body,
        };
        let (list, diff) = crate::pages::layout::columns(body, LIST_W, 2);
        let t = ctx.theme;
        let panel = Panel::framed(Some("Changes"))
            .meta("4 files")
            .focused(ctx.interaction.focused(self.files.id));
        let bg = panel.bg(t);
        let inner = panel.render(list, buf, t);
        self.files.render(inner, buf, ctx, bg);
        let panel = Panel::framed(Some("Diff")).focused(ctx.interaction.focused(self.view.id()));
        let inner = panel.render(diff, buf, ctx.theme);
        self.view.render(inner, buf, ctx, ctx.theme.canvas);
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let file = DiffFile {
        path: "retry.toml".into(),
        status: DiffStatus::Modified,
        hunks: vec![DiffHunk {
            old_start: 1,
            new_start: 1,
            lines: vec![
                DiffLine::remove("attempts = 3"),
                DiffLine::add("attempts = 5"),
                DiffLine::context("enabled = true"),
            ],
        }],
    };
    let mut unified = DiffView::new(id.child(0));
    unified.set_file(Some(file.clone()));
    let mut review = DiffView::new(id.child(1));
    review.set_file(Some(file));
    review.set_mode(DiffMode::Review);
    StateGrid::new(id)
        .label_width(10)
        .column_width(42)
        .rows(&[Rest, Focused, Empty, Narrow])
        .column("Unified", unified)
        .column("Review", review)
}

impl Page for DiffPage {
    fn title(&self) -> &'static str {
        "Diff viewer"
    }

    fn blurb(&self) -> &'static str {
        "Unified or old / new review · narrow panes use unified · select and copy text"
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
            PageEvent::Key(key) => {
                if cx.focus.is(self.review.id) || cx.focus.is(self.empty.id) {
                    let button = if cx.focus.is(self.review.id) {
                        &mut self.review
                    } else {
                        &mut self.empty
                    };
                    let (outcome, activated) = button.on_key(key);
                    if activated {
                        self.update_controls();
                    }
                    return outcome;
                }
                if cx.focus.is(self.files.id) {
                    let before = self.files.cursor;
                    let o = self.files.on_key(key);
                    if self.files.cursor != before {
                        self.update_controls();
                    }
                    return o;
                }
                if cx.focus.is(self.view.id()) {
                    let (outcome, event) = self.view.on_key(key);
                    if let Some(ViewportAction::Copy(text)) = event {
                        self.copied = Some(text);
                        cx.status("Selection copied");
                    }
                    return outcome;
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => {
                if ev.is(Phase::Click) && (ev.on(self.review.id) || ev.on(self.empty.id)) {
                    let button = if ev.on(self.review.id) {
                        &mut self.review
                    } else {
                        &mut self.empty
                    };
                    cx.focus.focus(button.id);
                    if button.route(ev).is_some_and(|r| r.has_actions()) {
                        self.update_controls();
                        return Outcome::Changed;
                    }
                    return Outcome::Ignored;
                }
                let before = self.files.cursor;
                if let Some(r) = self.files.route(ev) {
                    if ev.is(Phase::Click) && ev.index_of(Part::ROW).is_some() {
                        cx.focus.focus(self.files.id);
                    }
                    let o = r.outcome();
                    if self.files.cursor != before {
                        self.update_controls();
                    }
                    return o;
                }
                if ev.on(self.view.id())
                    && ev.bar.is_none()
                    && matches!(ev.phase(), Some(Phase::Press | Phase::Click | Phase::Drag))
                {
                    cx.focus.focus(self.view.id());
                }
                self.view
                    .route(ev)
                    .map_or(Outcome::Ignored, |r| r.outcome())
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if focus == Some(self.view.id()) {
            vec![
                ("↑↓", "Scroll"),
                ("←→", "Pan"),
                ("drag", "Select"),
                ("y", "Copy"),
                ("Esc", "Clear"),
            ]
        } else if focus == Some(self.files.id) {
            vec![("↑↓", "File"), ("Tab", "Next control")]
        } else {
            vec![("Enter", "Toggle"), ("Tab", "Next control")]
        }
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        if key == "diff.mode" {
            return Some(usize::from(self.review.on == Some(true)));
        }
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
            if key == "diff.mode" {
                self.review.on = Some(self.opts.value("diff.mode") == "review");
            }
            self.update_controls();
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

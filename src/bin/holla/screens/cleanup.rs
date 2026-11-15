use crate::screens::Holla;
use junie_tui::shell::Page as ShellPage;
use junie_tui::widgets::page_header::{Heading, PageHeader};
use std::collections::BTreeSet;

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::hit::Axis;
use junie_tui::core::id::{ItemKey, Part, PartRef, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::PointerEvent;
use junie_tui::core::scroll::ScrollState;
use junie_tui::theme::Tone;
use junie_tui::ui::context::ContextAnchor;
use junie_tui::ui::ctx::{RenderCtx, fill, legible_on_fill};
use junie_tui::ui::master_detail::MasterDetail;
use junie_tui::ui::text::{fit, width, wrap};
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::menu::MenuItem;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::props::Prop;
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::statusbar::StatusItem;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::domain::catalog::{InsightCategory, insight_candidates, observe_process};
use crate::domain::cleanup::{self, DeleteItem, DeletePlan, Eligibility, Mode, ProcessObservation};
use crate::domain::projection::{CleanupStamp, Deadline, Seen};
use crate::screens::{
    Cx, Go, MenuTake, ModalResult, Page, PendingMenu, Screen, StatusBits, open_context, row_anchor,
    scroll_drag, scroll_press,
};
use crate::sim::world::World;
use junie_tui::ui::layer::LayerTag;
use junie_tui::ui::text::plural;

pub const LIST: WidgetId = WidgetId::of("cleanup.list");
pub const DETAIL: WidgetId = WidgetId::of("cleanup.detail");

pub const ROW_MENU: WidgetId = WidgetId::of("cleanup.row.menu");

#[derive(Debug, Clone, PartialEq, Eq)]
enum Row {
    Heading(String),
    Blank,
    Category(usize),
    Candidate(usize, usize),
    Note(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Special {
    id: String,
    label: String,
    note: String,
    prerequisite: Option<String>,
}

const CLEANUP_ROW: &str = "cleanup-row";
const NO_LONGER_AVAILABLE: &str = "This item is no longer available";

/// What a row menu item targets, captured when the menu opens. Categories are keyed
/// by category id and candidates by category id plus path, never by row position.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CleanupTarget {
    Category(String),
    Candidate { category: String, path: String },
}

/// One explicit action per menu item. Toggles capture the desired state when the menu
/// opens, so a stale menu can neither invert a selection nor replay a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CleanupAction {
    SetOpen(bool),
    SetSelected(bool),
    CopyPath,
    SetAllSelected(bool),
    SetDetails(bool),
    SetPermanent(bool),
    SetDryRun(bool),
    Review,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CleanupContext {
    target: CleanupTarget,
    actions: Vec<CleanupAction>,
}

pub struct CleanupPage {
    pub filter: Option<String>,
    cats: Vec<InsightCategory>,
    special: Option<Special>,
    rows: Vec<Row>,
    pub cursor: usize,
    scroll: ScrollState,
    detail_scroll: ScrollState,
    list_area: Rect,
    detail_area: Rect,

    pub checked: BTreeSet<String>,

    /// Expanded categories by id, so a fold survives a reorder or a category
    /// that leaves and returns.
    open: BTreeSet<String>,
    menu: PendingMenu<CleanupContext>,
    pub mode: Mode,
    pub dry_run: bool,

    md: MasterDetail,
    seeded: bool,
    last_tick: u64,
    seen: Seen<CleanupStamp>,
    age_deadline: Deadline,

    pub reviewed: Option<u64>,
}

fn row_part(i: usize) -> PartRef {
    PartRef::item(Part::ROW, ItemKey::index(i))
}

impl CleanupPage {
    pub fn new(filter: Option<&str>) -> Self {
        Self {
            filter: filter.map(str::to_owned),
            cats: vec![],
            special: None,
            rows: vec![],
            cursor: 0,
            scroll: ScrollState::default(),
            detail_scroll: ScrollState::default(),
            list_area: Rect::ZERO,
            detail_area: Rect::ZERO,
            checked: BTreeSet::new(),
            open: BTreeSet::new(),
            menu: PendingMenu::default(),
            mode: Mode::Trash,
            dry_run: false,
            md: MasterDetail::new(LIST, DETAIL).detail_share(38, 34, 50),
            seeded: false,
            last_tick: u64::MAX,
            seen: Seen::default(),
            age_deadline: Deadline::default(),
            reviewed: None,
        }
    }

    fn special_category(&self, w: &World) -> Option<(Special, InsightCategory)> {
        let id = self.filter.as_deref()?;
        let cwd = w.location.cwd.clone();
        let (special, (paths, unreadable)): (Special, (Vec<String>, Vec<String>)) = match id {
            "gradle.clean-all" => (
                Special {
                    id: id.into(),
                    label: "Gradle outputs".into(),
                    note: ".gradle and build directories below this folder · depth 5 · no symlinks · no node_modules".into(),
                    prerequisite: Some("gradle --stop".into()),
                },
                cleanup::walk_candidates_report(&w.fs, &cwd, &|p, is_dir| is_dir && (p.ends_with("/.gradle") || p.ends_with("/build"))),
            ),
            "idea.clean" => (
                Special {
                    id: id.into(),
                    label: "IntelliJ metadata".into(),
                    note: ".idea directories and lowercase .iml files below this folder · depth 5 · no symlinks · no node_modules".into(),
                    prerequisite: None,
                },
                cleanup::walk_candidates_report(&w.fs, &cwd, &|p, is_dir| (is_dir && p.ends_with("/.idea")) || (!is_dir && p.ends_with(".iml"))),
            ),
            _ => return None,
        };
        let cat: &'static cleanup::Category = cleanup::category("project.artifacts").unwrap();
        let mut candidates = vec![];
        for p in paths {
            let scan = w.fs.scan(
                &p,
                &crate::sim::fs::ScanOptions {
                    include_hidden: true,
                    ..Default::default()
                },
            );
            let age =
                w.fs.get(&p)
                    .and_then(|n| cleanup::age_days(n.mtime, w.now_secs()));
            candidates.push(crate::domain::catalog::InsightCandidate {
                path: p,
                bytes: scan.allocated,
                partial: scan.errors() > 0,
                age_days: age,
                eligibility: Eligibility::Preselected,
            });
        }
        candidates.sort_by(|a, b| a.path.cmp(&b.path));
        Some((
            special,
            InsightCategory {
                category: cat,
                candidates,
                process: ProcessObservation::NotRunning,
                unreadable,
            },
        ))
    }

    fn is_open(&self, ci: usize) -> bool {
        self.cats
            .get(ci)
            .is_some_and(|c| self.open.contains(c.category.id))
    }

    fn any_open(&self) -> bool {
        (0..self.cats.len()).any(|ci| self.is_open(ci))
    }

    fn set_open(&mut self, ci: usize, open: bool) {
        let Some(id) = self.cats.get(ci).map(|c| c.category.id) else {
            return;
        };
        if open {
            self.open.insert(id.to_owned());
        } else {
            self.open.remove(id);
        }
    }

    fn selected_path(&self) -> Option<String> {
        match self.rows.get(self.cursor) {
            Some(Row::Candidate(c, i)) => self
                .cats
                .get(*c)
                .and_then(|k| k.candidates.get(*i))
                .map(|k| k.path.clone()),
            _ => None,
        }
    }

    /// Brings the candidates up to date with the world and returns whether it
    /// did any work. A cleanup stamp rescans the filesystem once. A day
    /// boundary refreshes only the cached ages and eligibility. Nothing else
    /// touches the candidates: folds, checks, mode and dry run rebuild rows.
    fn ensure_projection(&mut self, w: &World) -> bool {
        let stamp = w.cleanup_stamp();
        if self.seen.needs_rebuild(stamp) {
            self.reload_source(w);
            self.seen.published(stamp);
            return true;
        }
        if self.age_deadline.due(w.now_secs()) {
            self.refresh_ages(w);
            self.rebuild_rows();
            return true;
        }
        #[cfg(test)]
        self.assert_matches_reference(w);
        false
    }

    fn reload_source(&mut self, w: &World) {
        let keep = self.selected_path();
        let (cats, special) = if let Some((sp, cat)) = self.special_category(w) {
            (vec![cat], Some(sp))
        } else {
            let mut cats = insight_candidates(w);
            if let Some(f) = &self.filter {
                cats.retain(|c| c.category.id == f);
            }
            (cats, None)
        };
        self.cats = cats;
        self.special = special;
        self.reconcile_checked();
        if !self.seeded {
            for c in &self.cats {
                for k in &c.candidates {
                    if k.eligibility == Eligibility::Preselected {
                        self.checked.insert(k.path.clone());
                    }
                }
            }
            if self.filter.is_some() {
                for c in &self.cats {
                    self.open.insert(c.category.id.to_owned());
                }
            }
            self.seeded = true;
        }
        self.age_deadline = self.next_age_deadline(w);
        self.install_rows(keep);
    }

    /// Re-derives every cached candidate's age and eligibility from its file's
    /// modification time at the current clock. No scan, no size, no process
    /// observation: those are source facts the stamp owns.
    fn refresh_ages(&mut self, w: &World) {
        let now = w.now_secs();
        let special = self.special.is_some();
        for c in &mut self.cats {
            for k in &mut c.candidates {
                let age =
                    w.fs.get(&k.path)
                        .and_then(|n| cleanup::age_days(n.mtime, now));
                k.age_days = age;
                k.eligibility = if special {
                    Eligibility::Preselected
                } else {
                    cleanup::eligibility(c.category, age, &c.process)
                };
            }
        }
        self.reconcile_checked();
        self.age_deadline = self.next_age_deadline(w);
    }

    fn next_age_deadline(&self, w: &World) -> Deadline {
        let now = w.now_secs();
        let mtimes = self
            .cats
            .iter()
            .flat_map(|c| c.candidates.iter())
            .filter_map(|k| w.fs.get(&k.path).map(|n| n.mtime));
        Deadline::new(now, crate::domain::projection::age_deadline(mtimes, now))
    }

    /// Keeps only checked paths that are still eligible candidates. A path that
    /// becomes eligible later is not selected for the user.
    fn reconcile_checked(&mut self) {
        let live: BTreeSet<&str> = self
            .cats
            .iter()
            .flat_map(|c| c.candidates.iter())
            .filter(|k| k.eligibility.selectable())
            .map(|k| k.path.as_str())
            .collect();
        self.checked.retain(|p| live.contains(p.as_str()));
    }

    fn rebuild_rows(&mut self) {
        let keep = self.selected_path();
        self.install_rows(keep);
    }

    fn install_rows(&mut self, keep: Option<String>) {
        crate::domain::projection::record(|x| x.cleanup_row_builds += 1);
        let mut rows = vec![];
        if self.cats.is_empty() {
            rows.push(Row::Note(match &self.filter {
                Some(f) => format!("{f} is not detected on this host"),
                None => "no cleanup categories are detected on this host".into(),
            }));
        }
        for (ci, c) in self.cats.iter().enumerate() {
            if ci > 0 {
                rows.push(Row::Blank);
            }
            rows.push(Row::Category(ci));
            if self.is_open(ci) {
                if c.candidates.is_empty() {
                    rows.push(Row::Note("  no candidates · nothing to remove".into()));
                }
                for (ki, _) in c.candidates.iter().enumerate() {
                    rows.push(Row::Candidate(ci, ki));
                }
            }
        }
        rows.push(Row::Blank);
        rows.push(Row::Heading(format!(
            "mode {} · {} · m toggles the mode · n toggles dry run",
            match self.mode {
                Mode::Trash => "Trash (recoverable until emptied)",
                Mode::Permanent => "PERMANENT (cannot be undone)",
            },
            if self.dry_run {
                "dry run: nothing is removed"
            } else {
                "live"
            }
        )));
        self.rows = rows;
        let target = keep.and_then(|p| {
            self.rows.iter().position(
                |r| matches!(r, Row::Candidate(c, i) if self.cats[*c].candidates[*i].path == p),
            )
        });
        self.cursor = target
            .unwrap_or(self.cursor)
            .min(self.rows.len().saturating_sub(1));
        if !matches!(
            self.rows.get(self.cursor),
            Some(Row::Category(_) | Row::Candidate(..))
        ) {
            self.cursor = self
                .rows
                .iter()
                .position(|r| matches!(r, Row::Category(_) | Row::Candidate(..)))
                .unwrap_or(0);
        }
        self.scroll.set_content(self.rows.len());
    }

    /// The differential check of the cache: a settled page must read exactly
    /// like a page built from nothing at this instant.
    #[cfg(test)]
    pub(crate) fn candidates_signature(&self) -> Vec<String> {
        let mut v: Vec<String> = self.cats.iter().map(|c| format!("{c:?}")).collect();
        v.push(format!("{:?}", self.special));
        v
    }

    #[cfg(test)]
    fn assert_matches_reference(&self, w: &World) {
        let mut reference = CleanupPage::new(self.filter.as_deref());
        crate::domain::projection::uncounted(|| reference.ensure_projection(w));
        crate::domain::projection::assert_same_signature(
            &self.candidates_signature(),
            &reference.candidates_signature(),
            "cleanup",
        );
    }

    /// The item the detail panel describes, so its scroll rewinds only when this changes.
    fn detail_target(&self) -> Option<(&'static str, Option<String>)> {
        match self.rows.get(self.cursor)? {
            Row::Category(ci) => Some((self.cats.get(*ci)?.category.id, None)),
            Row::Candidate(ci, ki) => {
                let cat = self.cats.get(*ci)?;
                Some((cat.category.id, Some(cat.candidates.get(*ki)?.path.clone())))
            }
            _ => None,
        }
    }

    fn rewind_detail_if_moved(&mut self, before: Option<(&'static str, Option<String>)>) {
        if self.detail_target() != before {
            self.detail_scroll.jump_start();
        }
    }

    /// Moves the cursor and rewinds the detail only when the described item
    /// changes, so re-selecting the current item keeps the reader's place.
    fn select_row(&mut self, row: usize) {
        let before = self.detail_target();
        self.cursor = row;
        self.rewind_detail_if_moved(before);
    }

    fn step(&mut self, delta: isize) {
        let n = self.rows.len() as isize;
        let mut c = self.cursor as isize;
        loop {
            c += delta;
            if c < 0 || c >= n {
                return;
            }
            if matches!(self.rows[c as usize], Row::Category(_) | Row::Candidate(..)) {
                self.select_row(c as usize);
                self.scroll.ensure_visible(self.cursor);
                return;
            }
        }
    }

    pub fn selected_items(&self) -> Vec<DeleteItem> {
        let mut v = vec![];
        for c in &self.cats {
            for k in &c.candidates {
                if self.checked.contains(&k.path) && k.eligibility.selectable() {
                    v.push(DeleteItem {
                        path: k.path.clone(),
                        category: self
                            .special
                            .as_ref()
                            .map(|s| s.id.clone())
                            .unwrap_or(c.category.id.into()),
                        estimate: k.bytes,
                        guard: c.category.guard_process,
                    });
                }
            }
        }
        v
    }

    pub fn selected_bytes(&self) -> u64 {
        self.selected_items().iter().map(|i| i.estimate).sum()
    }

    fn build_plan(&self, w: &World) -> DeletePlan {
        let root = match &self.special {
            Some(_) => w.location.cwd.clone(),
            None => w.location.home.clone(),
        };
        DeletePlan::new(
            &w.host.name,
            &root,
            self.selected_items(),
            self.mode,
            self.dry_run,
        )
    }

    fn category_all_selected(&self, category: &str) -> bool {
        let eligible: Vec<&String> = self
            .cats
            .iter()
            .filter(|c| c.category.id == category)
            .flat_map(|c| c.candidates.iter())
            .filter(|k| k.eligibility.selectable())
            .map(|k| &k.path)
            .collect();
        !eligible.is_empty() && eligible.iter().all(|p| self.checked.contains(*p))
    }

    fn all_eligible_selected(&self) -> bool {
        let eligible: Vec<&String> = self
            .cats
            .iter()
            .flat_map(|c| c.candidates.iter())
            .filter(|k| k.eligibility.selectable())
            .map(|k| &k.path)
            .collect();
        !eligible.is_empty() && eligible.iter().all(|p| self.checked.contains(*p))
    }

    fn set_candidate_selected(&mut self, category: &str, path: &str, selected: bool, cx: &mut Cx) {
        let Some(k) = self
            .cats
            .iter()
            .find(|c| c.category.id == category)
            .and_then(|c| c.candidates.iter().find(|k| k.path == path))
        else {
            cx.status(NO_LONGER_AVAILABLE);
            return;
        };
        if let Eligibility::Ineligible(why) = &k.eligibility
            && selected
        {
            cx.status(format!(
                "Cannot select {} · {why}",
                path.rsplit('/').next().unwrap_or("")
            ));
            return;
        }
        if selected {
            self.checked.insert(path.to_owned());
        } else {
            self.checked.remove(path);
        }
        cx.status(format!(
            "{} selected · {}",
            plural(self.checked.len(), "item", "items"),
            junie_tui::ui::text::bytes(self.selected_bytes())
        ));
    }

    fn set_category_selected(&mut self, category: &str, selected: bool, cx: &mut Cx) {
        let Some(c) = self.cats.iter().find(|c| c.category.id == category) else {
            cx.status(NO_LONGER_AVAILABLE);
            return;
        };
        let label = c.category.label;
        let eligible: Vec<String> = c
            .candidates
            .iter()
            .filter(|k| k.eligibility.selectable())
            .map(|k| k.path.clone())
            .collect();
        let skipped = c.candidates.len() - eligible.len();
        if eligible.is_empty() {
            cx.status("No eligible candidate in this category");
            return;
        }
        for p in eligible {
            if selected {
                self.checked.insert(p);
            } else {
                self.checked.remove(&p);
            }
        }
        cx.status(format!(
            "{label} · {} selected · {}{}",
            plural(self.checked.len(), "item", "items"),
            junie_tui::ui::text::bytes(self.selected_bytes()),
            if skipped > 0 {
                format!(" · {skipped} ineligible stay unselected")
            } else {
                String::new()
            }
        ));
    }

    fn set_all_selected(&mut self, selected: bool, cx: &mut Cx) {
        let eligible: Vec<String> = self
            .cats
            .iter()
            .flat_map(|c| c.candidates.iter())
            .filter(|k| k.eligibility.selectable())
            .map(|k| k.path.clone())
            .collect();
        for p in eligible {
            if selected {
                self.checked.insert(p);
            } else {
                self.checked.remove(&p);
            }
        }
        cx.status(if selected {
            format!(
                "Every eligible candidate selected · {}",
                junie_tui::ui::text::bytes(self.selected_bytes())
            )
        } else {
            "Everything unselected".to_owned()
        });
    }

    fn set_mode(&mut self, mode: Mode, cx: &mut Cx) {
        self.mode = mode;
        self.reviewed = None;
        cx.status(match self.mode {
            Mode::Trash => "Mode: Trash · recoverable until the Trash is emptied",
            Mode::Permanent => "Mode: PERMANENT · cannot be undone · any earlier review is void",
        });
    }

    fn set_dry_run(&mut self, dry_run: bool, cx: &mut Cx) {
        self.dry_run = dry_run;
        self.reviewed = None;
        cx.status(if self.dry_run {
            "Dry run: paths are validated and sized, the log records would-remove, nothing is deleted and no daemon is stopped"
        } else {
            "Live run"
        });
    }

    fn review(&mut self, w: &World, cx: &mut Cx) -> Outcome {
        let plan = self.build_plan(w);
        if plan.items.is_empty() {
            cx.status("Select at least one eligible candidate first · Space selects");
            return Outcome::Changed;
        }
        self.reviewed = Some(plan.revision);
        cx.go(Go::Push(Page::CleanupGate { plan }));
        Outcome::Changed
    }

    fn toggle_candidate(&mut self, ci: usize, ki: usize, cx: &mut Cx) {
        let category = self.cats[ci].category.id;
        let path = self.cats[ci].candidates[ki].path.clone();
        let selected = !self.checked.contains(&path);
        self.set_candidate_selected(category, &path, selected, cx);
    }

    fn toggle_category(&mut self, ci: usize, cx: &mut Cx) {
        let category = self.cats[ci].category.id;
        let selected = !self.category_all_selected(category);
        self.set_category_selected(category, selected, cx);
    }

    fn detail(&self, w: &World) -> (String, String, Vec<Prop>) {
        match self.rows.get(self.cursor) {
            Some(Row::Category(ci)) => {
                let c = &self.cats[*ci];
                let cat = c.category;
                let (label, note) = match &self.special {
                    Some(s) => (s.label.clone(), s.note.clone()),
                    None => (cat.label.to_owned(), cat.note.to_owned()),
                };
                let mut v = vec![
                    Prop::new("Safety", cat.safety.label()).tone(match cat.safety {
                        cleanup::Safety::ReviewFirst => Tone::Warning,
                        _ => Tone::Normal,
                    }),
                    Prop::new(
                        "Age rule",
                        if cat.min_age_days > 0 {
                            format!(
                                "older than {} days · unknown or future age counts as recent",
                                cat.min_age_days
                            )
                        } else {
                            "any age".into()
                        },
                    )
                    .wrap(),
                    Prop::new(
                        "Platform",
                        if cat.macos_only {
                            "macOS only"
                        } else {
                            "macOS and Linux"
                        },
                    ),
                    Prop::new(
                        "Roots",
                        cat.roots
                            .iter()
                            .map(|r| format!("~/{r}"))
                            .collect::<Vec<_>>()
                            .join(" · "),
                    )
                    .wrap(),
                    Prop::new("Why", note).wrap(),
                    Prop::new(
                        "Size",
                        format!(
                            "{}{}",
                            junie_tui::ui::text::bytes(c.bytes()),
                            if c.candidates.iter().any(|k| k.partial) {
                                " · partial (unreadable entries)"
                            } else {
                                ""
                            }
                        ),
                    ),
                ];
                if !c.unreadable.is_empty() {
                    v.push(
                        Prop::new(
                            "Unreadable",
                            format!(
                                "{} skipped · the list is a lower bound: {}",
                                plural(c.unreadable.len(), "folder", "folders"),
                                c.unreadable
                                    .iter()
                                    .map(|p| w.location.short(p))
                                    .collect::<Vec<_>>()
                                    .join(" · ")
                            ),
                        )
                        .tone(Tone::Warning)
                        .wrap(),
                    );
                }
                if let Some(g) = cat.guard_process {
                    v.push(
                        Prop::new(
                            "Guard",
                            match &c.process {
                                ProcessObservation::Running => {
                                    format!("{g} is running · candidates are skipped")
                                }
                                ProcessObservation::NotRunning => format!("{g} is not running"),
                                ProcessObservation::Unknown(why) => {
                                    format!("{g} state unknown · {why} · treated as blocked")
                                }
                            },
                        )
                        .tone(match c.process {
                            ProcessObservation::NotRunning => Tone::Normal,
                            _ => Tone::Warning,
                        })
                        .wrap(),
                    );
                }
                if let Some(s) = &self.special
                    && let Some(p) = &s.prerequisite
                {
                    v.push(Prop::new("Prerequisite", format!("{p} runs first · a failed or unknown stop prevents the cleanup · dry run never stops the daemon")).tone(Tone::Warning).wrap());
                }
                v.push(Prop::new(
                    "Selection",
                    format!(
                        "{} of {} candidates eligible · {} checked",
                        c.candidates
                            .iter()
                            .filter(|k| k.eligibility.selectable())
                            .count(),
                        c.candidates.len(),
                        c.candidates
                            .iter()
                            .filter(|k| self.checked.contains(&k.path))
                            .count()
                    ),
                ));
                (label, "category".into(), v)
            }
            Some(Row::Candidate(ci, ki)) => {
                let c = &self.cats[*ci];
                let k = &c.candidates[*ki];
                let node = w.fs.get(&k.path);
                let mut v = vec![
                    Prop::new("Path", w.location.short(&k.path)).wrap(),
                    Prop::new("Kind", node.map(|n| n.kind_word()).unwrap_or("missing")),
                    Prop::new(
                        "Size",
                        format!(
                            "{}{}",
                            junie_tui::ui::text::bytes(k.bytes),
                            if k.partial { " · lower bound" } else { "" }
                        ),
                    ),
                    Prop::new(
                        "Age",
                        match k.age_days {
                            Some(0) => "modified today".into(),
                            Some(d) => format!("{d} days"),
                            None => "unknown · treated as recent".into(),
                        },
                    )
                    .tone(if k.age_days.is_none() {
                        Tone::Warning
                    } else {
                        Tone::Normal
                    }),
                    Prop::new(
                        "Category",
                        format!("{} · {}", c.category.label, c.category.safety.label()),
                    ),
                    Prop::new(
                        "Eligibility",
                        match &k.eligibility {
                            Eligibility::Preselected => "eligible · selected by default".into(),
                            Eligibility::Selectable => {
                                "eligible · review first, never preselected".into()
                            }
                            Eligibility::Ineligible(why) => format!("ineligible · {why}"),
                        },
                    )
                    .tone(match k.eligibility {
                        Eligibility::Ineligible(_) => Tone::Warning,
                        _ => Tone::Normal,
                    })
                    .wrap(),
                ];
                match cleanup::validate(&k.path, &w.location.home, w.host.os, &w.fs) {
                    Ok(canon) if canon != k.path => v.push(
                        Prop::new("Canonical", w.location.short(&canon))
                            .tone(Tone::Muted)
                            .wrap(),
                    ),
                    Ok(_) => {}
                    Err(cleanup::Deny(why)) => {
                        v.push(Prop::new("Denied", why).tone(Tone::Error).wrap())
                    }
                }
                v.push(
                    Prop::new(
                        "Method",
                        match (self.mode, self.dry_run) {
                            (_, true) => "dry run · would be removed · nothing changes",
                            (Mode::Trash, false) => {
                                "move to Trash · recoverable until the Trash is emptied"
                            }
                            (Mode::Permanent, false) => "permanent · cannot be undone",
                        },
                    )
                    .tone(if self.mode == Mode::Permanent && !self.dry_run {
                        Tone::Error
                    } else {
                        Tone::Normal
                    })
                    .wrap(),
                );
                (
                    k.path.rsplit('/').next().unwrap_or("").to_owned(),
                    "candidate".into(),
                    v,
                )
            }
            _ => ("Cleanup".into(), String::new(), vec![]),
        }
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let bg = t.canvas;
        let focused = ctx.interaction.focused(LIST);
        self.list_area = area;
        self.scroll.set_content(self.rows.len());
        self.scroll.set_viewport(area.height as usize);
        ctx.control(LIST, area, false);
        ctx.scrollable(LIST, area);
        let has_sb = self.scroll.overflows();
        let row_w = area.width.saturating_sub(u16::from(has_sb));
        for (k, i) in self.scroll.visible_range().enumerate() {
            let y = area.y + k as u16;
            let row = Rect::new(area.x, y, row_w, 1);
            match &self.rows[i] {
                Row::Blank => {}
                Row::Heading(h) => Heading::new(h).render(
                    Rect::new(area.x + 3, y, row_w.saturating_sub(3), 1),
                    buf,
                    t,
                    bg,
                ),
                Row::Note(n) => buf.set_string(
                    area.x + 5,
                    y,
                    junie_tui::ui::reveal::cut(
                        Rect::new(area.x + 5, y, row_w.saturating_sub(5), 1),
                        n,
                    ),
                    t.muted(),
                ),
                Row::Category(ci) => {
                    let c = &self.cats[*ci];
                    let rid = LIST.part(row_part(i));
                    let mut s = ctx.state(rid);
                    s.focused = focused && i == self.cursor;
                    s.selected = i == self.cursor;
                    let st = t.row(s, bg);
                    fill(buf, row, st);
                    buf.set_string(
                        row.x,
                        y,
                        t.gutter_symbol(s),
                        t.gutter(s, st.bg.unwrap_or(bg), false),
                    );
                    let plain = st.remove_modifier(Modifier::BOLD);
                    let eligible = c
                        .candidates
                        .iter()
                        .filter(|k| k.eligibility.selectable())
                        .count();
                    let checked = c
                        .candidates
                        .iter()
                        .filter(|k| self.checked.contains(&k.path))
                        .count();
                    let mark = if eligible == 0 {
                        "[–]"
                    } else if checked == eligible {
                        "[✓]"
                    } else if checked > 0 {
                        "[~]"
                    } else {
                        "[ ]"
                    };
                    buf.set_string(
                        row.x + 1,
                        y,
                        mark,
                        plain.fg(if checked > 0 {
                            ctx.chosen(focused)
                        } else {
                            t.text_muted
                        }),
                    );
                    let fold = if self.is_open(*ci) { "▾" } else { "▸" };
                    buf.set_string(row.x + 5, y, fold, plain.fg(t.text_secondary));
                    let label = match &self.special {
                        Some(sp) => sp.label.clone(),
                        None => c.category.label.to_owned(),
                    };
                    let label_w = (row_w * 34 / 100).clamp(16, 40);
                    buf.set_string(row.x + 7, y, fit(&label, label_w as usize), st);
                    let mut x = row.x + 7 + label_w + 2;
                    let size = format!("{:>9}", junie_tui::ui::text::bytes(c.bytes()));
                    if x + 10 <= row.right() {
                        buf.set_string(x, y, &size, plain.fg(t.text_secondary));
                    }
                    x += 11;
                    let meta = format!(
                        "{} · {} · {}",
                        c.category.safety.label(),
                        plural(c.candidates.len(), "candidate", "candidates"),
                        match &c.process {
                            ProcessObservation::Running =>
                                format!("{} running", c.category.guard_process.unwrap_or("app")),
                            ProcessObservation::Unknown(_) => "process state unknown".into(),
                            ProcessObservation::NotRunning =>
                                if c.category.min_age_days > 0 {
                                    format!("≥ {} d", c.category.min_age_days)
                                } else {
                                    "any age".into()
                                },
                        }
                    );
                    let avail = row.right().saturating_sub(x + 1) as usize;
                    if avail >= 8 {
                        buf.set_string(
                            x,
                            y,
                            junie_tui::ui::text::truncate_sep(&meta, avail),
                            plain.fg(match c.process {
                                ProcessObservation::NotRunning => t.text_muted,
                                _ => t.warning,
                            }),
                        );
                    }
                    legible_on_fill(buf, row, t, bg);
                    ctx.register_part(LIST, row_part(i), row);
                }
                Row::Candidate(ci, ki) => {
                    let c = &self.cats[*ci];
                    let k = &c.candidates[*ki];
                    let rid = LIST.part(row_part(i));
                    let mut s = ctx.state(rid);
                    s.focused = focused && i == self.cursor;
                    s.selected = i == self.cursor;
                    let st = t.row(s, bg);
                    fill(buf, row, st);
                    buf.set_string(
                        row.x,
                        y,
                        t.gutter_symbol(s),
                        t.gutter(s, st.bg.unwrap_or(bg), false),
                    );
                    let plain = st.remove_modifier(Modifier::BOLD);
                    let ineligible = !k.eligibility.selectable();
                    let quiet = if s.selected {
                        t.text_muted
                    } else {
                        t.text_faint
                    };
                    let checked = self.checked.contains(&k.path);
                    let (mark, ms) = if ineligible {
                        ("[ ]", plain.fg(quiet))
                    } else if checked {
                        ("[✓]", plain.fg(ctx.chosen(focused)))
                    } else {
                        ("[ ]", plain.fg(t.text_muted))
                    };
                    buf.set_string(row.x + 3, y, mark, ms);
                    let label = w.location.short(&k.path);
                    let label_w = (row_w * 40 / 100).clamp(16, 48);
                    buf.set_string(
                        row.x + 7,
                        y,
                        fit(&label, label_w as usize),
                        if ineligible { st.fg(quiet) } else { st },
                    );
                    let mut x = row.x + 7 + label_w + 2;
                    if x + 10 <= row.right() {
                        buf.set_string(
                            x,
                            y,
                            format!("{:>9}", junie_tui::ui::text::bytes(k.bytes)),
                            plain.fg(t.text_secondary),
                        );
                    }
                    x += 11;
                    let age = match k.age_days {
                        Some(0) => "today".to_owned(),
                        Some(d) => format!("{d} d"),
                        None => "age unknown".into(),
                    };
                    if x + 12 <= row.right() {
                        buf.set_string(
                            x,
                            y,
                            fit(&age, 12),
                            plain.fg(if k.age_days.is_none() {
                                t.warning
                            } else {
                                t.text_muted
                            }),
                        );
                    }
                    x += 13;
                    let tail = match &k.eligibility {
                        Eligibility::Ineligible(why) => why.clone(),
                        Eligibility::Selectable => "review first".into(),
                        Eligibility::Preselected => String::new(),
                    };
                    let avail = row.right().saturating_sub(x + 1) as usize;
                    if avail >= 6 && !tail.is_empty() {
                        buf.set_string(
                            x,
                            y,
                            junie_tui::ui::reveal::cut(Rect::new(x, y, (avail) as u16, 1), &tail),
                            plain.fg(if ineligible { quiet } else { t.text_faint }),
                        );
                    }
                    legible_on_fill(buf, row, t, bg);
                    ctx.register_part(LIST, row_part(i), row);
                }
            }
        }
        if has_sb {
            scrollbar::render_with_fade(
                Rect::new(area.x, area.y, area.width, area.height),
                buf,
                ctx,
                LIST,
                &self.scroll,
                focused,
            );
        }
    }

    fn render_detail(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let focused = ctx.interaction.focused(DETAIL);
        let (title, meta, props) = self.detail(w);
        let panel = Panel::card(Some(&title)).focused(focused).meta(&meta);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        self.detail_area = inner;
        ctx.control(DETAIL, area, false);
        ctx.scrollable(DETAIL, inner);
        let label_w = props.iter().map(|p| width(&p.label)).max().unwrap_or(4) as u16 + 2;
        let vw = inner.width.saturating_sub(label_w) as usize;
        let mut flat: Vec<(String, String, Tone)> = vec![];
        for p in props {
            for (i, part) in wrap(&p.value, vw.max(8)).into_iter().enumerate() {
                flat.push((
                    if i == 0 {
                        p.label.clone()
                    } else {
                        String::new()
                    },
                    part,
                    p.tone,
                ));
            }
        }
        self.detail_scroll.set_content(flat.len());
        self.detail_scroll.set_viewport(inner.height as usize);
        for (k, i) in self.detail_scroll.visible_range().enumerate() {
            let y = inner.y + k as u16;
            let (l, v, tone) = &flat[i];
            buf.set_string(inner.x, y, l, t.muted().bg(bg));
            buf.set_string(
                inner.x + label_w,
                y,
                junie_tui::ui::reveal::cut(Rect::new(inner.x + label_w, y, (vw) as u16, 1), v),
                Style::new().fg(t.tone(*tone)).bg(bg),
            );
        }
        if self.detail_scroll.overflows() {
            scrollbar::render_with_fade(
                Rect::new(inner.x, inner.y, inner.width, inner.height),
                buf,
                ctx,
                DETAIL,
                &self.detail_scroll,
                focused,
            );
        }
    }

    fn resolve_target(&self, target: &CleanupTarget) -> Option<(usize, Option<usize>)> {
        match target {
            CleanupTarget::Category(id) => {
                let ci = self
                    .cats
                    .iter()
                    .position(|c| c.category.id == id.as_str())?;
                Some((ci, None))
            }
            CleanupTarget::Candidate { category, path } => {
                let ci = self
                    .cats
                    .iter()
                    .position(|c| c.category.id == category.as_str())?;
                let ki = self.cats[ci]
                    .candidates
                    .iter()
                    .position(|k| &k.path == path)?;
                Some((ci, Some(ki)))
            }
        }
    }

    fn run_action(
        &mut self,
        target: &CleanupTarget,
        action: CleanupAction,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        let global = matches!(
            action,
            CleanupAction::SetAllSelected(_)
                | CleanupAction::SetDetails(_)
                | CleanupAction::SetPermanent(_)
                | CleanupAction::SetDryRun(_)
                | CleanupAction::Review
        );
        if !global && self.resolve_target(target).is_none() {
            cx.status(NO_LONGER_AVAILABLE);
            return Outcome::Changed;
        }
        match action {
            CleanupAction::SetOpen(open) => {
                if let CleanupTarget::Category(id) = target {
                    if open {
                        self.open.insert(id.clone());
                    } else {
                        self.open.remove(id);
                    }
                }
            }
            CleanupAction::SetSelected(selected) => match target {
                CleanupTarget::Category(id) => self.set_category_selected(id, selected, cx),
                CleanupTarget::Candidate { category, path } => {
                    self.set_candidate_selected(category, path, selected, cx);
                }
            },
            CleanupAction::CopyPath => {
                if let CleanupTarget::Candidate { path, .. } = target {
                    cx.copy(path.clone());
                }
            }
            CleanupAction::SetAllSelected(selected) => self.set_all_selected(selected, cx),
            CleanupAction::SetDetails(open) => {
                if open {
                    self.md.open_drawer(cx.focus);
                } else {
                    self.md.close_drawer(cx.focus);
                }
            }
            CleanupAction::SetPermanent(permanent) => self.set_mode(
                if permanent {
                    Mode::Permanent
                } else {
                    Mode::Trash
                },
                cx,
            ),
            CleanupAction::SetDryRun(dry_run) => self.set_dry_run(dry_run, cx),
            CleanupAction::Review => return self.review(w, cx),
        }
        self.rebuild_rows();
        Outcome::Changed
    }

    fn row_menu(&mut self, row: usize, anchor: ContextAnchor, cx: &mut Cx) -> Outcome {
        let mut items: Vec<(MenuItem, CleanupAction)> = vec![];
        let (target, title) = match self.rows.get(row) {
            Some(Row::Category(ci)) => {
                let c = &self.cats[*ci];
                let open = self.open.contains(c.category.id);
                items.push((
                    MenuItem::new(if open { "Collapse" } else { "Expand" }).shortcut("Enter"),
                    CleanupAction::SetOpen(!open),
                ));
                let all = self.category_all_selected(c.category.id);
                let mut select = MenuItem::new(if all {
                    "Unselect category"
                } else {
                    "Select category"
                })
                .shortcut("Space")
                .separator();
                if !c.candidates.iter().any(|k| k.eligibility.selectable()) {
                    select = select.reason("No eligible candidate in this category");
                }
                items.push((select, CleanupAction::SetSelected(!all)));
                (
                    CleanupTarget::Category(c.category.id.to_owned()),
                    c.category.label.to_string(),
                )
            }
            Some(Row::Candidate(ci, ki)) => {
                let c = &self.cats[*ci];
                let k = &c.candidates[*ki];
                let checked = self.checked.contains(&k.path);
                let mut select =
                    MenuItem::new(if checked { "Unselect" } else { "Select" }).shortcut("Space");
                if let Eligibility::Ineligible(why) = &k.eligibility {
                    select = select.reason(why.clone());
                }
                items.push((select, CleanupAction::SetSelected(!checked)));
                items.push((
                    MenuItem::new("Copy path").shortcut("y").separator(),
                    CleanupAction::CopyPath,
                ));
                (
                    CleanupTarget::Candidate {
                        category: c.category.id.to_owned(),
                        path: k.path.clone(),
                    },
                    k.path.clone(),
                )
            }
            _ => return Outcome::Ignored,
        };
        let all = self.all_eligible_selected();
        let nothing_eligible = !self
            .cats
            .iter()
            .flat_map(|c| c.candidates.iter())
            .any(|k| k.eligibility.selectable());
        let mut every = MenuItem::new(if all {
            "Unselect everything"
        } else {
            "Select every eligible"
        })
        .shortcut("a");
        if nothing_eligible {
            every = every.reason("Nothing here is eligible");
        }
        items.push((every, CleanupAction::SetAllSelected(!all)));
        items.push((
            self.md.menu_item().shortcut("p").separator(),
            CleanupAction::SetDetails(!self.md.drawer_open()),
        ));
        items.push((
            MenuItem::new(if self.mode == Mode::Permanent {
                "Use Trash"
            } else {
                "Use permanent delete"
            })
            .shortcut("m"),
            CleanupAction::SetPermanent(self.mode != Mode::Permanent),
        ));
        items.push((
            MenuItem::new(if self.dry_run {
                "Switch to live run"
            } else {
                "Switch to dry run"
            })
            .shortcut("n"),
            CleanupAction::SetDryRun(!self.dry_run),
        ));
        let mut review = MenuItem::new("Review deletion…").shortcut("d").danger();
        if self.selected_items().is_empty() {
            review = review.reason("Select at least one eligible candidate first");
        }
        items.push((review, CleanupAction::Review));

        let (menu_items, actions): (Vec<MenuItem>, Vec<CleanupAction>) = items.into_iter().unzip();
        let token = self.menu.capture(CleanupContext { target, actions });
        open_context(
            cx,
            ROW_MENU,
            anchor,
            title,
            menu_items,
            LayerTag::new(CLEANUP_ROW).key(token),
        )
    }
}

impl ShellPage<Holla> for CleanupPage {
    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        self.ensure_projection(w);
        if let Some(o) = self.md.on_key(key, cx.focus) {
            return o;
        }
        if cx.focus.is(DETAIL) {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') if key.plain() => {
                    self.detail_scroll.scroll_by(-1);
                    return Outcome::Changed;
                }
                KeyCode::Down | KeyCode::Char('j') if key.plain() => {
                    self.detail_scroll.scroll_by(1);
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        if matches!(key.code, KeyCode::Char(_)) && key.chord() {
            return Outcome::Ignored;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.step(-1);
                Outcome::Changed
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.step(1);
                Outcome::Changed
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.select_row(
                    self.rows
                        .iter()
                        .position(|r| matches!(r, Row::Category(_) | Row::Candidate(..)))
                        .unwrap_or(0),
                );
                self.scroll.jump_start();
                Outcome::Changed
            }
            KeyCode::End | KeyCode::Char('G') => {
                if let Some(last) = self
                    .rows
                    .iter()
                    .rposition(|r| matches!(r, Row::Category(_) | Row::Candidate(..)))
                {
                    self.select_row(last);
                    self.scroll.ensure_visible(self.cursor);
                }
                Outcome::Changed
            }
            KeyCode::Char(' ') => {
                match self.rows.get(self.cursor).cloned() {
                    Some(Row::Category(ci)) => self.toggle_category(ci, cx),
                    Some(Row::Candidate(ci, ki)) => self.toggle_candidate(ci, ki, cx),
                    _ => cx.status("Move onto a category or candidate first"),
                }
                self.rebuild_rows();
                Outcome::Changed
            }
            KeyCode::Enter | KeyCode::Right | KeyCode::Left => {
                match self.rows.get(self.cursor).cloned() {
                    Some(Row::Category(ci)) => {
                        if key.code == KeyCode::Left
                            || (key.code == KeyCode::Enter && self.is_open(ci))
                        {
                            self.set_open(ci, false);
                        } else {
                            self.set_open(ci, true);
                        }
                        self.rebuild_rows();
                    }
                    Some(Row::Candidate(ci, ki)) => {
                        if key.code == KeyCode::Enter {
                            self.toggle_candidate(ci, ki, cx);
                            self.rebuild_rows();
                        } else if key.code == KeyCode::Left {
                            let before = self.detail_target();
                            self.set_open(ci, false);
                            self.rebuild_rows();
                            self.cursor = self
                                .rows
                                .iter()
                                .position(|r| matches!(r, Row::Category(c) if *c == ci))
                                .unwrap_or(0);
                            self.rewind_detail_if_moved(before);
                        }
                    }
                    _ => {}
                }
                Outcome::Changed
            }
            KeyCode::Char('a') => {
                let selected = !self.all_eligible_selected();
                self.set_all_selected(selected, cx);
                self.rebuild_rows();
                Outcome::Changed
            }
            KeyCode::Char('m') => {
                let next = if self.mode == Mode::Permanent {
                    Mode::Trash
                } else {
                    Mode::Permanent
                };
                self.set_mode(next, cx);
                self.rebuild_rows();
                Outcome::Changed
            }
            KeyCode::Char('n') => {
                let dry_run = !self.dry_run;
                self.set_dry_run(dry_run, cx);
                self.rebuild_rows();
                Outcome::Changed
            }
            KeyCode::Char('d') | KeyCode::Backspace => self.review(w, cx),
            KeyCode::Char('p') => {
                self.md.toggle_drawer(cx.focus);
                Outcome::Changed
            }
            KeyCode::Char('y') => {
                if let Some(Row::Candidate(ci, ki)) = self.rows.get(self.cursor) {
                    cx.copy(self.cats[*ci].candidates[*ki].path.clone());
                }
                Outcome::Changed
            }
            KeyCode::Esc => {
                if self.filter.is_none() && self.any_open() {
                    self.open.clear();
                    self.rebuild_rows();
                    return Outcome::Changed;
                }
                cx.go(Go::Pop);
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        self.ensure_projection(w);
        let (id, pos) = (ev.id, ev.pos());

        let row = ev.on(LIST).then(|| ev.index_of(Part::ROW)).flatten();
        if let Some((axis, delta)) = ev.wheel() {
            if axis != Axis::Vertical {
                return Outcome::Ignored;
            }
            if id == LIST {
                self.scroll.scroll_by(delta as isize);
                return Outcome::Changed;
            }
            if id == DETAIL {
                self.detail_scroll.scroll_by(delta as isize);
                return Outcome::Changed;
            }
            return Outcome::Ignored;
        }
        match ev.phase() {
            Some(Phase::Press | Phase::Drag) => {
                let (area, scroll) = if id == scrollbar::id_for(LIST) {
                    (self.list_area, &mut self.scroll)
                } else if id == scrollbar::id_for(DETAIL) {
                    (self.detail_area, &mut self.detail_scroll)
                } else {
                    return Outcome::Ignored;
                };
                if ev.is(Phase::Press) {
                    scroll_press(area, pos, scroll)
                } else {
                    scroll_drag(area, pos, scroll)
                }
            }
            Some(Phase::Click) => {
                if id == scrollbar::id_for(LIST) {
                    cx.focus.focus(LIST);
                    return scroll_press(self.list_area, pos, &mut self.scroll);
                }
                if id == scrollbar::id_for(DETAIL) {
                    cx.focus.focus(DETAIL);
                    return scroll_press(self.detail_area, pos, &mut self.detail_scroll);
                }
                if id == DETAIL {
                    cx.focus.focus(DETAIL);
                    return Outcome::Changed;
                }
                if let Some(i) = row {
                    self.select_row(i);
                    cx.focus.focus(LIST);
                    match self.rows.get(i).cloned() {
                        Some(Row::Category(ci)) => {
                            self.set_open(ci, !self.is_open(ci));
                            self.rebuild_rows();
                        }
                        Some(Row::Candidate(ci, ki)) => {
                            self.toggle_candidate(ci, ki, cx);
                            self.rebuild_rows();
                        }
                        _ => {}
                    }
                    return Outcome::Changed;
                }
                if id == LIST {
                    cx.focus.focus(LIST);
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            Some(Phase::Secondary) => {
                let Some(i) = row else {
                    return Outcome::Ignored;
                };
                if !matches!(
                    self.rows.get(i),
                    Some(Row::Category(_) | Row::Candidate(..))
                ) {
                    return Outcome::Ignored;
                }
                if cx.context.moves_cursor {
                    self.select_row(i);
                }
                cx.focus.focus(LIST);
                self.row_menu(i, ContextAnchor::Pointer(pos), cx)
            }
            _ => Outcome::Ignored,
        }
    }

    fn on_tick(&mut self, w: &mut World, _cx: &mut Cx) -> Outcome {
        if w.tick == self.last_tick {
            return Outcome::Ignored;
        }
        self.last_tick = w.tick;

        if self.ensure_projection(w) {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn enter(&mut self, w: &mut World, cx: &mut Cx) {
        self.ensure_projection(w);
        cx.focus.focus(LIST);
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        self.ensure_projection(w);
        let t = ctx.theme;
        let title = match &self.special {
            Some(s) => format!("Cleanup · {}", s.label),
            None => match &self.filter {
                Some(f) => format!(
                    "Cleanup · {}",
                    cleanup::category(f).map(|c| c.label).unwrap_or(f)
                ),
                None => "Cleanup review".into(),
            },
        };
        let total: u64 = self.cats.iter().map(InsightCategory::bytes).sum();
        let line = format!(
            "{} · {} · {} selected ({}) · {}{}",
            plural(self.cats.len(), "category", "categories"),
            junie_tui::ui::text::bytes(total),
            self.selected_items().len(),
            junie_tui::ui::text::bytes(self.selected_bytes()),
            match self.mode {
                Mode::Trash => "Trash",
                Mode::Permanent => "PERMANENT",
            },
            if self.dry_run { " · dry run" } else { "" }
        );
        let header = PageHeader::new(&title).subtitle(
            &line,
            if self.mode == Mode::Permanent && !self.dry_run {
                Tone::Error
            } else {
                Tone::Muted
            },
        );
        header.render(area, buf, t);
        let body = header.body(area);
        let l = self.md.layout(body, buf, ctx, ctx.theme.canvas);
        if !l.master.is_empty() {
            self.render_list(l.master, buf, ctx, w);
        }
        if !l.detail.is_empty() {
            self.render_detail(l.detail, buf, ctx, w);
        }
        self.md.finish(ctx);
    }

    fn on_layer_result(
        &mut self,
        tag: &LayerTag,
        result: ModalResult,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        match self.menu.take(tag, CLEANUP_ROW, &result) {
            MenuTake::Stale => Outcome::Ignored,
            MenuTake::Dismissed => Outcome::Changed,
            MenuTake::Chosen(context, index) => match context.actions.get(index).copied() {
                Some(action) => self.run_action(&context.target, action, w, cx),
                None => Outcome::Ignored,
            },
        }
    }

    fn hints(&self, focus: Option<WidgetId>, _w: &World) -> Vec<Hint> {
        if focus == Some(DETAIL) {
            return vec![hint("↑↓", "Scroll"), hint("Esc", "Back to list")];
        }
        let mut v = vec![
            hint("↑↓", "Move"),
            hint("Space", "Select"),
            hint("Enter", "Open"),
        ];
        v.push(hint("a", "All"));
        v.push(junie_tui::widgets::keyhint::toggle_hint(
            "m",
            self.mode == Mode::Permanent,
            "Use Trash",
            "Use permanent delete",
        ));
        v.push(junie_tui::widgets::keyhint::toggle_hint(
            "n",
            self.dry_run,
            "Switch to live run",
            "Switch to dry run",
        ));
        v.push(hint("d", "Review deletion"));
        v.push(hint("p", "Details"));
        v.push(hint("Esc", "Back"));
        v.push(junie_tui::widgets::keyhint::CONTEXT_MENU);
        v
    }

    fn crumb(&self, _w: &World) -> String {
        match &self.filter {
            Some(f) => format!(
                "Cleanup › {}",
                cleanup::category(f).map(|c| c.label).unwrap_or(f)
            ),
            None => "Cleanup".into(),
        }
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(LIST)
    }
}

impl Screen for CleanupPage {
    fn background_tick(&self) -> bool {
        false
    }

    fn on_context_key(&mut self, _w: &mut World, cx: &mut Cx) -> Outcome {
        if !cx.focus.is(LIST) {
            return Outcome::Ignored;
        }
        let anchor = row_anchor(LIST, self.cursor, self.list_area, self.scroll.offset);
        self.row_menu(self.cursor, anchor, cx)
    }

    fn status(&self, _w: &World) -> StatusBits {
        StatusBits {
            center: Some(
                StatusItem::new(
                    format!(
                        "{} selected · {}{}",
                        self.selected_items().len(),
                        junie_tui::ui::text::bytes(self.selected_bytes()),
                        if self.dry_run { " · dry run" } else { "" }
                    ),
                    Tone::Secondary,
                )
                .priority(6),
            ),
            right: vec![],
        }
    }
}

pub fn plan_facts(plan: &DeletePlan, w: &World) -> Vec<Prop> {
    let home = w.location.home.clone();
    let denied = plan
        .items
        .iter()
        .filter(|i| {
            cleanup::validate(&i.path, &home, w.host.os, &w.fs).is_err()
                || cleanup::protected_descendant(&i.path, &home, &w.fs).is_some()
        })
        .count();
    let guarded: Vec<&str> = plan
        .items
        .iter()
        .filter_map(|i| i.guard)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut v = vec![
        Prop::new("Action", format!(
            "{} {} under {}",
            if plan.dry_run { "dry run over" } else if plan.mode == Mode::Permanent { "permanently delete" } else { "move to Trash" },
            plural(plan.items.len(), "item", "items"),
            w.location.short(&plan.root)
        )).wrap(),
        Prop::new("Host", format!("{} · {}", w.host.name, w.host.role.label())),
        Prop::new("Mode", match (plan.mode, plan.dry_run) {
            (_, true) => "dry run · validate and size · log would-remove · touch nothing".to_owned(),
            (Mode::Trash, false) => format!("Trash · {} · space returns after the Trash is emptied", match &w.platform.trash {
                crate::domain::stack::TrashBackend::MacNative => "native macOS Trash",
                crate::domain::stack::TrashBackend::FreeDesktop => "FreeDesktop trash",
                crate::domain::stack::TrashBackend::Unavailable(_) => "no Trash backend · every item will fail, never fall back",
            }),
            (Mode::Permanent, false) => "PERMANENT · unlink and remove_dir_all · cannot be undone".into(),
        }).tone(if plan.mode == Mode::Permanent && !plan.dry_run { Tone::Error } else { Tone::Normal }).wrap(),
        Prop::new("Estimate", format!("{} allocated · APFS clones may overcount · purgeable space excluded · not a free-space guarantee", junie_tui::ui::text::bytes(plan.estimate()))).wrap(),
        Prop::new("Denied", if denied == 0 { "none · every path passed the lexical, protected-root, user-data and symlink rules".to_owned() } else { format!("{denied} would be skipped at commit by the safety rules") }).tone(if denied > 0 { Tone::Warning } else { Tone::Muted }).wrap(),
    ];
    if !guarded.is_empty() {
        v.push(
            Prop::new(
                "Guard",
                format!(
                    "{} · observed once at commit · a running or unknown process skips its items",
                    guarded.join(", ")
                ),
            )
            .tone(Tone::Warning)
            .wrap(),
        );
    }
    if plan.items.iter().any(|i| i.category == "gradle.clean-all") {
        v.push(
            Prop::new(
                "Prerequisite",
                if plan.dry_run {
                    "gradle --stop is not run on a dry run"
                } else {
                    "gradle --stop first · a failed or unknown stop cancels the cleanup"
                },
            )
            .tone(Tone::Warning)
            .wrap(),
        );
    }
    v.push(
        Prop::new(
            "Revision",
            format!(
                "{:016x} · any change to paths, mode or dry run voids this review",
                plan.revision
            ),
        )
        .tone(Tone::Muted)
        .wrap(),
    );
    v.push(Prop::new("Threat model", "an unprivileged user · ancestors are re-resolved at commit · a concurrent rename of the final path cannot be excluded").tone(Tone::Muted).wrap());
    let shown: Vec<String> = plan
        .items
        .iter()
        .take(10)
        .map(|i| w.location.short(&i.path))
        .collect();
    v.push(
        Prop::new(
            "Paths",
            format!(
                "{}{}",
                shown.join(" · "),
                if plan.items.len() > 10 {
                    format!(" · … {} more", plan.items.len() - 10)
                } else {
                    String::new()
                }
            ),
        )
        .wrap(),
    );
    v
}

pub fn commit(plan: &DeletePlan, w: &mut World) -> Result<usize, String> {
    if plan.fingerprint() != plan.revision {
        return Err("the reviewed plan changed · confirm again".into());
    }
    if w.cleanup_job.is_some() {
        return Err("a cleanup is still running · wait for its report".into());
    }
    if !plan.dry_run && plan.items.iter().any(|i| i.category == "gradle.clean-all") {
        let stop = crate::domain::exec::Command::argv(
            "gradle",
            &["--stop"],
            &w.location.cwd,
            &w.host.name,
        );
        let script = crate::domain::outcomes::for_command(w, &stop)
            .unwrap_or_else(|| crate::domain::activity::Script::unmodeled("gradle --stop"));
        if script.exit != 0 || script.spawn_failure.is_some() {
            let reason = script
                .lines
                .iter()
                .rev()
                .find(|l| l.tone == crate::domain::activity::LineTone::Error)
                .map(|l| l.text.clone())
                .or(script.spawn_failure.clone())
                .unwrap_or("unknown".into());
            return Err(format!(
                "gradle --stop failed ({reason}) · daemon state unknown · nothing was removed"
            ));
        }
        w.apply_effect(&crate::domain::effect::Effect::GradleStop);
    }
    let procs: Vec<(String, ProcessObservation)> = plan
        .items
        .iter()
        .filter_map(|i| i.guard)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|g| (g.to_owned(), observe_process(w, g)))
        .collect();
    let exec = cleanup::Execution::start(plan, plan.revision, &w.ops_log.path)?;
    Ok(w.edit_cleanup(|w| {
        w.reports.push(exec.report().clone());
        let report_index = w.reports.len() - 1;
        w.cleanup_job = Some(crate::sim::world::CleanupJob {
            exec,
            report_index,
            processes: procs,
        });
        report_index
    }))
}

/// One tick of a running cleanup job. A running job is real work every tick
/// (log lines, removals, report progress), so each step is one cleanup edit.
pub fn step(w: &mut World) -> Option<usize> {
    let job = w.cleanup_job.take()?;
    w.edit_cleanup(|w| step_job(w, job))
}

fn step_job(w: &mut World, mut job: crate::sim::world::CleanupJob) -> Option<usize> {
    let home = w.location.home.clone();
    let os = w.host.os;
    let now = w.now_secs();
    let procs = job.processes.clone();
    let observe = |name: &str| -> ProcessObservation {
        procs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, o)| o.clone())
            .unwrap_or(ProcessObservation::NotRunning)
    };
    let mut fs = std::mem::take(&mut w.fs);
    let mut log = std::mem::take(&mut w.ops_log);
    job.exec.step(
        &mut fs,
        &mut log,
        &cleanup::ExecContext {
            home: &home,
            os,
            now_secs: now,
            processes: &observe,
        },
    );
    w.fs = fs;
    let idx = job.report_index;
    if job.exec.finished() {
        let report = job.exec.finish(&mut log);
        w.ops_log = log;
        settle(w, idx, report);
        Some(idx)
    } else {
        w.ops_log = log;
        w.reports[idx] = job.exec.report().clone();
        w.cleanup_job = Some(job);
        None
    }
}

fn settle(w: &mut World, idx: usize, report: cleanup::Report) {
    w.persisted.ops_log = w.ops_log.lines.clone();

    let removed: Vec<String> = report
        .items
        .iter()
        .filter(|i| {
            matches!(
                i.outcome,
                cleanup::Outcome::Removed | cleanup::Outcome::Trashed
            )
        })
        .map(|i| i.path.clone())
        .collect();

    for p in &removed {
        w.disk
            .candidates
            .retain(|c| !(c.path == *p || c.path.starts_with(&format!("{p}/"))));
    }
    if report.freed_now > 0
        && let Some(fs) = w.disk.filesystems.first_mut()
    {
        fs.used_gb = fs
            .used_gb
            .saturating_sub((report.freed_now / (1024 * 1024 * 1024)) as u32);
    }
    w.reports[idx] = report;
    if let Some(root) = w.scan.as_ref().map(|s| s.root.clone()) {
        w.start_scan(&root);
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod context_tests {
    use super::*;
    use crate::domain::fixtures::world_for;
    use crate::scenario::{Motion, Scenario};
    use crate::screens::pointer_tests::{cell_of, render_hits, secondary_open};
    use junie_tui::core::focus::{Focus, FocusRing};
    use junie_tui::shell::Request;

    fn world() -> World {
        world_for(Scenario::RustDirty, Motion::Full)
    }

    /// Opens the page's menu for `row` through the real context-key path and returns its layer tag.
    fn open_menu(page: &mut CleanupPage, w: &mut World, row: usize) -> LayerTag {
        page.cursor = row;
        let mut focus = Focus::default();
        focus.focus(LIST);
        let ring = FocusRing::default();
        let mut cx = Cx::new(&mut focus, &ring);
        page.on_context_key(w, &mut cx);
        cx.requests
            .iter()
            .find_map(|r| match r {
                Request::Open(_, tag) => Some(tag.clone()),
                _ => None,
            })
            .expect("the row menu opened")
    }

    /// Delivers the menu choice `index` for `tag`, returning the page's outcome and the requests it made.
    fn deliver(
        page: &mut CleanupPage,
        w: &mut World,
        tag: &LayerTag,
        index: usize,
    ) -> (Outcome, Vec<Request<ModalResult, (), Go>>) {
        let mut focus = Focus::default();
        let ring = FocusRing::default();
        let mut cx = Cx::new(&mut focus, &ring);
        let outcome = page.on_layer_result(tag, ModalResult::MenuChosen(index), w, &mut cx);
        (outcome, std::mem::take(&mut cx.requests))
    }

    /// Delivers the menu choice `index` for `tag` and returns what the page asked the shell to do.
    fn choose(
        page: &mut CleanupPage,
        w: &mut World,
        tag: &LayerTag,
        index: usize,
    ) -> Vec<Request<ModalResult, (), Go>> {
        deliver(page, w, tag, index).1
    }

    fn status_of(requests: &[Request<ModalResult, (), Go>]) -> Option<String> {
        requests.iter().find_map(|r| match r {
            Request::Status(s, _) => Some(s.clone()),
            _ => None,
        })
    }

    fn row_of_candidate(page: &CleanupPage, ci: usize, ki: usize) -> usize {
        page.rows
            .iter()
            .position(|r| matches!(r, Row::Candidate(c, k) if *c == ci && *k == ki))
            .expect("candidate row")
    }

    fn row_of_category(page: &CleanupPage, ci: usize) -> usize {
        page.rows
            .iter()
            .position(|r| matches!(r, Row::Category(c) if *c == ci))
            .expect("category row")
    }

    /// A page with every category expanded, so each candidate has a row.
    fn expanded_page(w: &World) -> CleanupPage {
        let mut page = CleanupPage::new(None);
        page.ensure_projection(w);
        for c in &page.cats {
            page.open.insert(c.category.id.to_owned());
        }
        page.rebuild_rows();
        page
    }

    fn open_candidate(page: &mut CleanupPage, w: &mut World, ci: usize, ki: usize) -> LayerTag {
        let row = row_of_candidate(page, ci, ki);
        open_menu(page, w, row)
    }

    #[test]
    fn cleanup_context_keeps_candidate_path_after_rebuild() {
        let mut w = world();
        let mut page = expanded_page(&w);
        let last = page.cats.len() - 1;
        assert!(last >= 1, "fixture needs two categories");
        let path = page.cats[last].candidates[0].path.clone();
        page.open.clear();
        page.open.insert(page.cats[last].category.id.to_owned());
        page.rebuild_rows();
        let row = row_of_candidate(&page, last, 0);

        let tag = open_menu(&mut page, &mut w, row);
        // Expanding an earlier category inserts rows above the captured candidate.
        page.open.insert(page.cats[0].category.id.to_owned());
        page.rebuild_rows();
        assert_ne!(row_of_candidate(&page, last, 0), row, "the row moved");

        let requests = choose(&mut page, &mut w, &tag, 1);
        assert!(
            requests
                .iter()
                .any(|r| matches!(r, Request::Copy(s, _) if *s == path)),
            "copied the captured path {path}, got {:?}",
            status_of(&requests)
        );
    }

    #[test]
    fn cleanup_context_pointer_menu_keeps_candidate_after_rebuild() {
        let mut w = world();
        let mut page = expanded_page(&w);
        let last = page.cats.len() - 1;
        assert!(last >= 1, "fixture needs two categories");
        let path = page.cats[last].candidates[0].path.clone();
        page.open.clear();
        page.open.insert(page.cats[last].category.id.to_owned());
        page.rebuild_rows();
        let row = row_of_candidate(&page, last, 0);

        let hits = render_hits(&mut page, &w);
        let tag = secondary_open(
            &mut page,
            &mut w,
            &hits,
            cell_of(&hits, LIST.part(row_part(row))),
        );

        page.open.insert(page.cats[0].category.id.to_owned());
        page.rebuild_rows();
        assert_ne!(row_of_candidate(&page, last, 0), row, "the row moved");

        let requests = choose(&mut page, &mut w, &tag, 1);
        assert!(
            requests
                .iter()
                .any(|r| matches!(r, Request::Copy(s, _) if *s == path)),
            "the pointer menu copied the captured path {path}, got {:?}",
            status_of(&requests)
        );
    }

    #[test]
    fn cleanup_context_category_uses_stable_id() {
        let mut w = world();
        let mut page = expanded_page(&w);
        let with_selectable: Vec<usize> = (0..page.cats.len())
            .filter(|&ci| {
                page.cats[ci]
                    .candidates
                    .iter()
                    .any(|k| k.eligibility.selectable())
            })
            .collect();
        assert!(
            with_selectable.len() >= 2,
            "fixture needs two selectable categories"
        );
        let (earlier_ci, target_ci) = (
            with_selectable[0],
            with_selectable[with_selectable.len() - 1],
        );
        let captured_id = page.cats[target_ci].category.id;
        let selectable_paths = |page: &CleanupPage, ci: usize| -> Vec<String> {
            page.cats[ci]
                .candidates
                .iter()
                .filter(|k| k.eligibility.selectable())
                .map(|k| k.path.clone())
                .collect()
        };
        let earlier = selectable_paths(&page, earlier_ci);
        let target_paths = selectable_paths(&page, target_ci);

        // The earlier category starts closed, so opening it later inserts rows above the captured one.
        page.open.remove(page.cats[earlier_ci].category.id);
        page.rebuild_rows();
        let row = row_of_category(&page, target_ci);

        let tag = open_menu(&mut page, &mut w, row);
        page.open
            .insert(page.cats[earlier_ci].category.id.to_owned());
        page.rebuild_rows();
        assert_ne!(row_of_category(&page, target_ci), row, "the row moved");
        assert_eq!(page.cats[target_ci].category.id, captured_id);

        let before: BTreeSet<String> = page.checked.clone();
        let all_before = target_paths.iter().all(|p| before.contains(p));
        choose(&mut page, &mut w, &tag, 1);

        for p in &target_paths {
            assert_eq!(
                page.checked.contains(p),
                !all_before,
                "captured category {p}"
            );
        }
        for p in &earlier {
            assert_eq!(
                page.checked.contains(p),
                before.contains(p),
                "the earlier category keeps its state: {p}"
            );
        }
    }

    #[test]
    fn context_select_and_unselect_are_idempotent() {
        let mut w = world();
        let mut page = expanded_page(&w);
        let (ci, ki, path) = page
            .cats
            .iter()
            .enumerate()
            .find_map(|(ci, c)| {
                c.candidates
                    .iter()
                    .enumerate()
                    .find(|(_, k)| k.eligibility.selectable())
                    .map(|(ki, k)| (ci, ki, k.path.clone()))
            })
            .expect("a selectable candidate");
        page.checked.remove(&path);
        let row = row_of_candidate(&page, ci, ki);

        let tag = open_menu(&mut page, &mut w, row);
        // The selection changes while the menu is open, so the advertised "Select" must stay a select.
        page.checked.insert(path.clone());
        choose(&mut page, &mut w, &tag, 0);
        assert!(page.checked.contains(&path), "Select stays selected");
    }

    #[test]
    fn context_old_token_cannot_consume_new_menu() {
        let mut w = world();
        let mut page = expanded_page(&w);
        let (ci, ki, first) = page
            .cats
            .iter()
            .enumerate()
            .find_map(|(ci, c)| {
                c.candidates
                    .iter()
                    .enumerate()
                    .find(|(_, k)| k.eligibility.selectable())
                    .map(|(ki, k)| (ci, ki, k.path.clone()))
            })
            .expect("a selectable candidate");
        page.checked.remove(&first);
        let first_row = row_of_candidate(&page, ci, ki);
        let second = page
            .cats
            .iter()
            .enumerate()
            .flat_map(|(ci, c)| {
                c.candidates
                    .iter()
                    .enumerate()
                    .map(move |(ki, k)| (ci, ki, k))
            })
            .find(|(_, _, k)| k.path != first && k.eligibility.selectable())
            .map(|(ci, ki, k)| (ci, ki, k.path.clone()))
            .expect("a second selectable candidate");
        page.checked.remove(&second.2);

        let old_tag = open_menu(&mut page, &mut w, first_row);
        let second_row = row_of_candidate(&page, second.0, second.1);
        let new_tag = open_menu(&mut page, &mut w, second_row);

        let (outcome, _) = deliver(&mut page, &mut w, &old_tag, 0);
        assert_eq!(outcome, Outcome::Ignored, "a stale result is ignored");
        assert!(!page.checked.contains(&first), "the stale menu did nothing");
        assert!(
            !page.checked.contains(&second.2),
            "the newer menu stays pending"
        );

        choose(&mut page, &mut w, &new_tag, 0);
        assert!(
            page.checked.contains(&second.2),
            "the newer menu acts on its own target"
        );
        assert!(!page.checked.contains(&first));
    }

    #[test]
    fn context_missing_target_and_newly_ineligible_are_safe() {
        let mut w = world();
        let mut page = expanded_page(&w);
        let (ci, ki, path) = page
            .cats
            .iter()
            .enumerate()
            .find_map(|(ci, c)| {
                c.candidates
                    .iter()
                    .enumerate()
                    .find(|(_, k)| k.eligibility.selectable())
                    .map(|(ki, k)| (ci, ki, k.path.clone()))
            })
            .expect("a selectable candidate");
        page.checked.remove(&path);
        let row = row_of_candidate(&page, ci, ki);
        let tag = open_menu(&mut page, &mut w, row);

        // A background rebuild removes the captured candidate before the choice arrives.
        let checked_before = page.checked.clone();
        page.cats[ci].candidates.remove(ki);
        let requests = choose(&mut page, &mut w, &tag, 0);

        assert_eq!(
            status_of(&requests).as_deref(),
            Some(NO_LONGER_AVAILABLE),
            "missing target is reported"
        );
        assert_eq!(page.checked, checked_before, "no other row changed");
    }

    fn dismiss(page: &mut CleanupPage, w: &mut World, tag: &LayerTag) {
        let mut focus = Focus::default();
        let ring = FocusRing::default();
        let mut cx = Cx::new(&mut focus, &ring);
        page.on_layer_result(tag, ModalResult::Cancelled, w, &mut cx);
    }

    fn first_selectable(page: &CleanupPage, skip: Option<&str>) -> (usize, usize, String) {
        page.cats
            .iter()
            .enumerate()
            .find_map(|(ci, c)| {
                c.candidates
                    .iter()
                    .enumerate()
                    .find(|(_, k)| k.eligibility.selectable() && Some(k.path.as_str()) != skip)
                    .map(|(ki, k)| (ci, ki, k.path.clone()))
            })
            .expect("a selectable candidate")
    }

    #[test]
    fn context_cancel_retires_only_matching_request() {
        let mut w = world();
        let mut page = expanded_page(&w);
        let (ci, ki, first) = first_selectable(&page, None);
        page.checked.remove(&first);
        let old_tag = open_candidate(&mut page, &mut w, ci, ki);

        let (ci2, ki2, second) = first_selectable(&page, Some(&first));
        page.checked.remove(&second);
        let new_tag = open_candidate(&mut page, &mut w, ci2, ki2);

        // A dismissal of the superseded menu must not retire the newer one.
        dismiss(&mut page, &mut w, &old_tag);
        choose(&mut page, &mut w, &new_tag, 0);
        assert!(page.checked.contains(&second), "the newer menu survived");

        // A dismissal retires its own request, so a late choice does nothing.
        let (ci3, ki3, third) = first_selectable(&page, Some(&second));
        page.checked.remove(&third);
        let dismissed = open_candidate(&mut page, &mut w, ci3, ki3);
        dismiss(&mut page, &mut w, &dismissed);
        choose(&mut page, &mut w, &dismissed, 0);
        assert!(
            !page.checked.contains(&third),
            "the dismissed menu did nothing"
        );
    }

    #[test]
    fn context_global_action_survives_target_removal() {
        let mut w = world();
        let mut page = expanded_page(&w);
        let (ci, ki, _) = first_selectable(&page, None);
        let tag = open_candidate(&mut page, &mut w, ci, ki);

        page.cats[ci].candidates.remove(ki);
        // Item 2 is "Select every eligible", a page-wide action.
        let requests = choose(&mut page, &mut w, &tag, 2);
        let status = status_of(&requests).expect("the global action reports");
        assert_ne!(
            status, NO_LONGER_AVAILABLE,
            "a global action is not blocked by the target"
        );
    }

    #[test]
    fn context_newly_ineligible_select_is_refused() {
        let mut w = world();
        let mut page = expanded_page(&w);
        let (ci, ki, path) = first_selectable(&page, None);
        page.checked.remove(&path);
        let tag = open_candidate(&mut page, &mut w, ci, ki);

        // A guard appears between opening the menu and choosing Select.
        page.cats[ci].candidates[ki].eligibility = Eligibility::Ineligible("running".into());
        let requests = choose(&mut page, &mut w, &tag, 0);

        assert!(
            !page.checked.contains(&path),
            "an ineligible candidate is not selected"
        );
        assert!(
            status_of(&requests).is_some_and(|s| s.starts_with("Cannot select ")),
            "the refusal is reported"
        );
    }
}

#[cfg(test)]
mod detail_scroll_tests {
    use super::*;
    use crate::domain::fixtures::world_for;
    use crate::scenario::{Motion, Scenario};
    use crate::screens::pointer_tests::{cell_of, render_hits};
    use junie_tui::core::focus::{Focus, FocusRing};
    use junie_tui::core::hit::HitRegistry;
    use ratatui::crossterm::event::KeyModifiers;
    use ratatui::layout::Position;

    fn selectable_rows(page: &CleanupPage) -> Vec<usize> {
        (0..page.rows.len())
            .filter(|&i| matches!(page.rows[i], Row::Category(_) | Row::Candidate(..)))
            .collect()
    }

    /// A page with every category expanded and its cursor on the first selectable row.
    fn page_on_first_row(w: &World) -> CleanupPage {
        let mut page = CleanupPage::new(None);
        page.ensure_projection(w);
        for c in &page.cats {
            page.open.insert(c.category.id.to_owned());
        }
        page.rebuild_rows();
        let rows = selectable_rows(&page);
        assert!(rows.len() >= 3, "fixture needs three selectable rows");
        page.cursor = rows[0];
        page
    }

    fn scroll_detail_to_three(page: &mut CleanupPage) {
        page.detail_scroll.set_content(100);
        page.detail_scroll.set_viewport(5);
        page.detail_scroll.jump_start();
        page.detail_scroll.scroll_by(3);
        assert_eq!(page.detail_scroll.offset, 3);
    }

    fn press(page: &mut CleanupPage, w: &mut World, code: KeyCode) {
        let mut focus = Focus::default();
        focus.focus(LIST);
        let ring = FocusRing::default();
        let mut cx = Cx::new(&mut focus, &ring);
        let key = Key {
            code,
            mods: KeyModifiers::NONE,
        };
        page.on_key(&key, w, &mut cx);
    }

    /// Renders the page, scrolls its detail to 3 (a render clamps the scroll to the
    /// detail height), then routes a `phase` pointer event on `row` through `pointer`.
    fn deliver(page: &mut CleanupPage, w: &mut World, phase: Phase, row: usize) {
        let hits: HitRegistry = render_hits(page, w);
        scroll_detail_to_three(page);
        let pos: Position = cell_of(&hits, LIST.part(row_part(row)));
        let id = hits.hit(pos).expect("a rendered row is under the pointer");
        let event = PointerEvent::new(&hits, phase, id, pos, 1, KeyModifiers::NONE);
        let mut focus = Focus::default();
        let ring = FocusRing::default();
        let mut cx = Cx::new(&mut focus, &ring);
        page.pointer(&event, w, &mut cx);
    }

    #[test]
    fn up_and_down_at_the_ends_keep_the_detail_scroll() {
        let mut w = world_for(Scenario::RustDirty, Motion::Full);
        let mut page = page_on_first_row(&w);
        let rows = selectable_rows(&page);
        let (first, second, last) = (rows[0], rows[1], *rows.last().expect("rows"));

        scroll_detail_to_three(&mut page);
        press(&mut page, &mut w, KeyCode::Up);
        assert_eq!(page.cursor, first);
        assert_eq!(page.detail_scroll.offset, 3, "Up on the first row");

        press(&mut page, &mut w, KeyCode::Down);
        assert_eq!(page.cursor, second);
        assert_eq!(page.detail_scroll.offset, 0, "Down onto a new row");

        page.cursor = last;
        scroll_detail_to_three(&mut page);
        press(&mut page, &mut w, KeyCode::Down);
        assert_eq!(page.cursor, last);
        assert_eq!(page.detail_scroll.offset, 3, "Down on the last row");
    }

    #[test]
    fn home_and_end_reset_the_detail_scroll_only_when_the_row_changes() {
        let mut w = world_for(Scenario::RustDirty, Motion::Full);
        let mut page = page_on_first_row(&w);
        let rows = selectable_rows(&page);
        let (first, last) = (rows[0], *rows.last().expect("rows"));

        scroll_detail_to_three(&mut page);
        press(&mut page, &mut w, KeyCode::Home);
        assert_eq!(page.cursor, first);
        assert_eq!(page.detail_scroll.offset, 3, "Home on the first row");

        press(&mut page, &mut w, KeyCode::End);
        assert_eq!(page.cursor, last);
        assert_eq!(page.detail_scroll.offset, 0, "End onto a new row");

        scroll_detail_to_three(&mut page);
        press(&mut page, &mut w, KeyCode::End);
        assert_eq!(page.cursor, last);
        assert_eq!(page.detail_scroll.offset, 3, "End on the last row");

        press(&mut page, &mut w, KeyCode::Home);
        assert_eq!(page.cursor, first);
        assert_eq!(page.detail_scroll.offset, 0, "Home onto a new row");
    }

    #[test]
    fn row_clicks_reset_the_detail_scroll_only_when_the_row_changes() {
        let mut w = world_for(Scenario::RustDirty, Motion::Full);
        let mut page = page_on_first_row(&w);
        let rows = selectable_rows(&page);
        let (first, second) = (rows[0], rows[1]);

        deliver(&mut page, &mut w, Phase::Click, first);
        assert_eq!(page.cursor, first);
        assert_eq!(
            page.detail_scroll.offset, 3,
            "re-clicking the selected row keeps the scroll"
        );
        deliver(&mut page, &mut w, Phase::Click, second);
        assert_eq!(page.cursor, second);
        assert_eq!(
            page.detail_scroll.offset, 0,
            "clicking a new row starts its detail at the top"
        );
    }

    #[test]
    fn secondary_clicks_reset_the_detail_scroll_only_when_the_row_changes() {
        let mut w = world_for(Scenario::RustDirty, Motion::Full);
        let mut page = page_on_first_row(&w);
        let rows = selectable_rows(&page);
        let (first, second) = (rows[0], rows[1]);

        deliver(&mut page, &mut w, Phase::Secondary, first);
        assert_eq!(page.cursor, first);
        assert_eq!(
            page.detail_scroll.offset, 3,
            "a secondary click on the selected row keeps the scroll"
        );

        deliver(&mut page, &mut w, Phase::Secondary, second);
        assert_eq!(page.cursor, second);
        assert_eq!(
            page.detail_scroll.offset, 0,
            "a secondary click on a new row starts its detail at the top"
        );
    }

    #[test]
    fn collapsing_from_a_candidate_rewinds_the_detail_to_the_category() {
        let mut w = world_for(Scenario::RustDirty, Motion::Full);
        let mut page = page_on_first_row(&w);
        let candidate = selectable_rows(&page)
            .into_iter()
            .find(|&i| matches!(page.rows[i], Row::Candidate(..)))
            .expect("a candidate row");
        page.cursor = candidate;

        scroll_detail_to_three(&mut page);
        press(&mut page, &mut w, KeyCode::Left);
        assert!(matches!(page.rows[page.cursor], Row::Category(_)));
        assert_eq!(page.detail_scroll.offset, 0, "the category is a new item");
    }
}

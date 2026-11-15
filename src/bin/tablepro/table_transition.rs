//! Every change to a table's rows or controls (sort, filters, refresh, discard)
//! passes one gate. A table with unsaved edits keeps them, and its shown
//! controls, until the user saves or discards. Nothing is replaced before the
//! replacement has been read and can be installed.

use super::*;
use junie_tui::widgets::chips::ChipAction;
use junie_tui::widgets::grid::{CellEditResult, FinishEdit};

use crate::tabs::{TableTab, TableViewState};

/// Shown when a second change arrives while one waits for its answer.
pub const PENDING_NOTICE: &str = "Finish the open confirmation first";

const STALE_NOTICE: &str = "The table changed since this was requested · nothing was applied";

const GUARD_DIALOG: WidgetId = WidgetId::of("dialog.table-transition");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionReason {
    Refresh,
    Sort,
    Filters,
}

/// What a gesture asks for: the complete next view of one table, named by the
/// table's key and the view it was made against. The view is captured whole, so
/// a later answer never replays a position-based edit against a changed list.
#[derive(Debug, Clone)]
pub struct TableChange {
    pub tab: ItemKey,
    pub expected_view_revision: u64,
    pub next: TableViewState,
    pub reason: TransitionReason,
    /// The status shown once the change is installed. `None` leaves it alone.
    pub notice: Option<String>,
}

impl TableChange {
    pub fn new(
        t: &TableTab,
        reason: TransitionReason,
        next: TableViewState,
        notice: Option<String>,
    ) -> Self {
        Self {
            tab: t.key,
            expected_view_revision: t.view_revision(),
            next,
            reason,
            notice,
        }
    }

    /// The same view, read again.
    pub fn refresh(t: &TableTab, notice: &str) -> Self {
        Self::new(
            t,
            TransitionReason::Refresh,
            t.view().clone(),
            Some(notice.to_owned()),
        )
    }

    pub fn sort(t: &TableTab, sort: Option<(usize, junie_tui::widgets::table::SortDir)>) -> Self {
        let mut next = t.view().clone();
        next.sort = sort;
        let notice = match sort {
            Some((c, d)) => format!(
                "Sorted by {} {}",
                t.columns[c].0,
                if d == junie_tui::widgets::table::SortDir::Asc {
                    "ascending"
                } else {
                    "descending"
                }
            ),
            None => "Sort cleared".to_owned(),
        };
        Self::new(t, TransitionReason::Sort, next, Some(notice))
    }

    pub fn filters(t: &TableTab, next: TableViewState, notice: Option<String>) -> Self {
        Self::new(t, TransitionReason::Filters, next, notice)
    }

    /// The change a filter chip gesture asks for. Activating a chip or adding
    /// one opens the filter editor instead and asks for nothing here.
    pub fn from_chip(t: &TableTab, action: &ChipAction) -> Option<Self> {
        let mut next = t.view().clone();
        let notice = match *action {
            ChipAction::Remove(i) if i < next.filters.len() => {
                next.filters.remove(i);
                Some("Filter removed".to_owned())
            }
            ChipAction::Toggle(i) if i < next.filters.len() => {
                next.filters[i].enabled = !next.filters[i].enabled;
                None
            }
            ChipAction::ClearAll => {
                next.filters.clear();
                Some("Filters cleared".to_owned())
            }
            ChipAction::Lead => {
                next.match_all = !next.match_all;
                None
            }
            _ => return None,
        };
        Some(Self::filters(t, next, notice))
    }
}

/// A change bound to the connection era it was made in.
#[derive(Debug, Clone)]
pub struct TableTransition {
    pub change: TableChange,
    pub connection_generation: u64,
}

/// The view a filter editor was opened against. Its answer is applied to this
/// table and view, never to whatever is active when it arrives.
#[derive(Debug, Clone, Copy)]
pub(super) struct FilterTarget {
    pub tab: ItemKey,
    pub connection_generation: u64,
    pub view_revision: u64,
}

/// A transition that waits for the user, other than the guard's own dialog.
#[derive(Debug, Clone)]
pub(super) enum TransitionWait {
    /// The discard confirmation is open.
    Discard(TableTransition),
    /// The user chose Save: the transition runs once this exact save succeeds.
    Save {
        transition: TableTransition,
        token: GridCommitToken,
    },
}

/// Which confirmation a transition with unsaved edits opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Review {
    /// Cancel, Discard or Save.
    Guard,
    /// The plain discard question.
    Discard,
}

enum Gate {
    Refused(String),
    Unchanged,
    Apply,
    Unsaved(usize),
}

pub(super) fn guard() -> LeaveGuard<TableTransition> {
    LeaveGuard::new(GUARD_DIALOG)
}

impl App {
    fn transition_busy(&self) -> bool {
        self.table_guard.pending().is_some() || self.transition_wait.is_some()
    }

    /// The table a transition names, if it is still the table and view the
    /// request was made against.
    fn transition_target(&mut self, tr: &TableTransition) -> Option<(&mut TableTab, Source<'_>)> {
        if tr.connection_generation != self.connection_generation {
            return None;
        }
        let (t, src) = self.workbench.as_mut()?.table_mut(tr.change.tab)?;
        (t.view_revision() == tr.change.expected_view_revision).then_some((t, src))
    }

    pub(super) fn request_table_change(&mut self, change: TableChange, review: Review) {
        let transition = TableTransition {
            change,
            connection_generation: self.connection_generation,
        };
        self.request_table_transition(transition, review);
    }

    /// The one entry for every change to a table's rows or controls. The live
    /// editor is finished first, once; a rejected input stops the request with
    /// the input kept. A change that would drop edits asks first and touches
    /// nothing until answered.
    pub(super) fn request_table_transition(&mut self, tr: TableTransition, review: Review) {
        if self.transition_busy() {
            self.set_status_as(StatusKind::Warn, PENDING_NOTICE.into());
            return;
        }
        let (gate, edited) = self.open_gate(&tr);
        if edited && let Some(w) = self.workbench.as_mut() {
            w.sync_strip();
        }
        match gate {
            Gate::Refused(text) => self.set_status_as(StatusKind::Warn, text),
            Gate::Unchanged => {}
            Gate::Apply => self.apply_and_report(&tr),
            Gate::Unsaved(changes) => match review {
                Review::Guard => {
                    let stakes = Stakes::Unsaved { changes };
                    if let Err(dialog) =
                        self.table_guard
                            .request(Leave::To(tr), &stakes, &Strings::DEFAULT)
                    {
                        self.open_dialog(*dialog);
                    }
                }
                Review::Discard => {
                    self.transition_wait = Some(TransitionWait::Discard(tr));
                    self.open_dialog(Dialog::destructive(
                        DISCARD_DIALOG,
                        "Discard unsaved changes?",
                        &format!(
                            "{changes} pending change{} will be dropped. The rows are reloaded from the server.",
                            if changes == 1 { "" } else { "s" }
                        ),
                        "Discard",
                    ));
                }
            },
        }
    }

    /// Resolves the target and finishes its editor. The flag says the editor
    /// held an edit that is now part of the draft.
    fn open_gate(&mut self, tr: &TableTransition) -> (Gate, bool) {
        let Some((t, _)) = self.transition_target(tr) else {
            return (Gate::Refused(STALE_NOTICE.into()), false);
        };
        if t.saving() {
            return (Gate::Refused(SAVING_NOTICE.into()), false);
        }
        t.restore_view_controls();
        let mut edited = false;
        match t.grid.finish_edit_session() {
            Err(e) => {
                return (
                    Gate::Refused(format!("Fix the open cell first: {e}")),
                    false,
                );
            }
            Ok(FinishEdit::Accepted {
                result: CellEditResult::Changed,
                ..
            }) => {
                t.promote();
                edited = true;
            }
            Ok(_) => {}
        }
        let gate = if tr.change.reason != TransitionReason::Refresh && tr.change.next == *t.view() {
            Gate::Unchanged
        } else if t.dirty_count() == 0 {
            Gate::Apply
        } else {
            Gate::Unsaved(t.dirty_count())
        };
        (gate, edited)
    }

    /// Reads the next view, then installs it and the rows together. A failure at
    /// either step leaves the rows, the draft and the controls as they were.
    fn apply_transition(&mut self, tr: &TableTransition) -> Result<Option<String>, String> {
        let Some((t, src)) = self.transition_target(tr) else {
            return Err(STALE_NOTICE.into());
        };
        let next = tr.change.next.clone();
        let loaded = t
            .prepare_load(src, &next)
            .map_err(|e| format!("Cannot load the rows: {} · nothing was changed", e.message))?;
        t.install_load(loaded, next).map_err(|e| match e {
            junie_tui::widgets::grid::GridDataError::Busy => SAVING_NOTICE.to_owned(),
            other => format!("Cannot install the rows: {other} · nothing was changed"),
        })?;
        if tr.change.reason == TransitionReason::Filters {
            t.promote();
        }
        if let Some(w) = self.workbench.as_mut() {
            w.sync_strip();
        }
        Ok(tr.change.notice.clone())
    }

    fn apply_and_report(&mut self, tr: &TableTransition) {
        match self.apply_transition(tr) {
            Ok(Some(notice)) => self.set_status(notice),
            Ok(None) => {}
            Err(text) => self.set_status_as(StatusKind::Fail, text),
        }
    }

    /// The answer to the guard's Cancel, Discard, Save dialog.
    pub(super) fn table_guard_closed(&mut self, result: DialogResult) {
        match self.table_guard.resolve(result) {
            Resolved::Discard(Leave::To(tr)) => self.apply_and_report(&tr),
            Resolved::Save(Leave::To(tr)) => self.save_then_transition(tr),
            _ => self.set_status("Changes kept pending".into()),
        }
    }

    /// The answer to the plain discard question.
    pub(super) fn discard_review_closed(&mut self, confirmed: bool) {
        match self.transition_wait.take() {
            Some(TransitionWait::Discard(tr)) if confirmed => self.apply_and_report(&tr),
            Some(TransitionWait::Discard(_)) => {}
            other => self.transition_wait = other,
        }
    }

    /// Reviews the table's own save, then runs the transition when that exact
    /// save succeeds. Any refusal, cancellation or failure drops the transition.
    fn save_then_transition(&mut self, tr: TableTransition) {
        if self.transition_target(&tr).is_none() {
            self.set_status_as(StatusKind::Warn, STALE_NOTICE.into());
            return;
        }
        if let Some(token) = self.begin_commit_for(Some(tr.change.tab)) {
            self.transition_wait = Some(TransitionWait::Save {
                transition: tr,
                token,
            });
        }
    }

    /// The transition that waits for `token`'s save, if there is one. Taking it
    /// is how a save's end, whatever it is, retires the wait.
    pub(super) fn take_save_continuation(
        &mut self,
        token: GridCommitToken,
    ) -> Option<TableTransition> {
        match self.transition_wait.take() {
            Some(TransitionWait::Save {
                transition,
                token: waiting,
            }) if waiting == token => Some(transition),
            other => {
                self.transition_wait = other;
                None
            }
        }
    }

    /// Runs the transition a successful save was waiting for and reports both.
    pub(super) fn finish_save_transition(&mut self, saved: &str, tr: &TableTransition) {
        match self.apply_transition(tr) {
            Ok(Some(notice)) => self.set_status_as(StatusKind::Ok, format!("{saved} · {notice}")),
            Ok(None) => self.set_status_as(StatusKind::Ok, saved.to_owned()),
            Err(text) => self.set_status_as(
                StatusKind::Warn,
                format!("{saved} · the view was not updated: {text}"),
            ),
        }
    }

    /// Drops everything that waited on the connection that just ended.
    pub(super) fn cancel_transitions(&mut self) {
        self.transition_wait = None;
        self.filter_target = None;
        let _ = self.table_guard.resolve(DialogResult::Cancelled);
    }

    /// The view a filter editor opens against, for its answer to find again.
    pub(super) fn capture_filter_target(&mut self, tab: ItemKey, view_revision: u64) {
        self.filter_target = Some(FilterTarget {
            tab,
            connection_generation: self.connection_generation,
            view_revision,
        });
    }

    /// The filter editor's answer: the filter list it was opened against, with
    /// the answer applied, as one transition.
    pub(super) fn filter_answered(
        &mut self,
        index: Option<usize>,
        filter: Filter,
    ) -> Option<WidgetId> {
        let target = self.filter_target.take()?;
        let transition_for = |t: &TableTab| {
            let mut next = t.view().clone();
            match index {
                Some(i) if i < next.filters.len() => next.filters[i] = filter.clone(),
                _ => next.filters.push(filter.clone()),
            }
            let n = next.filters.iter().filter(|f| f.enabled).count();
            let notice = format!("{n} filter{} applied", if n == 1 { "" } else { "s" });
            TableTransition {
                change: TableChange {
                    tab: target.tab,
                    expected_view_revision: target.view_revision,
                    next,
                    reason: TransitionReason::Filters,
                    notice: Some(notice),
                },
                connection_generation: target.connection_generation,
            }
        };
        let stale = target.connection_generation != self.connection_generation;
        let found = self
            .workbench
            .as_mut()
            .and_then(|w| w.table_mut(target.tab))
            .map(|(t, _)| t)
            .filter(|t| !stale && t.view_revision() == target.view_revision);
        let Some(t) = found else {
            self.set_status_as(
                StatusKind::Warn,
                "That filter was made for an older view of the table · nothing was applied".into(),
            );
            return None;
        };
        let (transition, grid) = (transition_for(t), t.grid.id);
        self.request_table_transition(transition, Review::Guard);
        Some(grid)
    }
}

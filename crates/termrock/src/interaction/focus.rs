//! Per-frame, stable-identity focus registration with scoped modal restoration.

use ratatui_core::layout::{Position, Rect};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    widgets::PanelEmphasis,
};

use super::ModalStack;

/// One focusable identity registered for the current frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusTarget<Id, ScopeId> {
    /// Stable identity used across layout and data changes.
    pub id: Id,
    /// Scope that owns this target.
    pub scope: ScopeId,
    /// Painted geometry, when pointer focus is supported.
    pub area: Option<Rect>,
    /// Whether traversal and pointer focus may select this target.
    pub enabled: bool,
}

/// Result of a focus operation.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusOutcome<Id> {
    /// Input or request did not belong to the focus ring.
    Ignored,
    /// The ring consumed the operation without changing focus.
    Unchanged,
    /// Focus changed between stable identities.
    Changed {
        /// Previous focused identity.
        from: Option<Id>,
        /// New focused identity.
        to: Option<Id>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FocusScope<Id, ScopeId> {
    id: ScopeId,
    restore: Option<Id>,
    restore_index: Option<usize>,
}

/// Persistent focus state backed by registrations rebuilt every frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusRing<Id, ScopeId> {
    focused: Option<Id>,
    targets: Vec<FocusTarget<Id, ScopeId>>,
    previous_targets: Vec<FocusTarget<Id, ScopeId>>,
    scopes: Vec<FocusScope<Id, ScopeId>>,
    pending_restore_index: Option<usize>,
}

impl<Id: Clone + Eq, ScopeId: Clone + Eq> FocusRing<Id, ScopeId> {
    /// Creates a ring with one permanent root scope.
    #[must_use]
    pub fn new(root_scope: ScopeId, focused: Option<Id>) -> Self {
        Self {
            focused,
            targets: Vec::new(),
            previous_targets: Vec::new(),
            scopes: vec![FocusScope {
                id: root_scope,
                restore: None,
                restore_index: None,
            }],
            pending_restore_index: None,
        }
    }

    /// Starts a registration pass while preserving the preceding order.
    pub fn begin_frame(&mut self) {
        self.previous_targets.clear();
        std::mem::swap(&mut self.targets, &mut self.previous_targets);
    }

    /// Registers one target. The first duplicate `(scope, id)` wins.
    pub fn register(&mut self, target: FocusTarget<Id, ScopeId>) {
        let duplicate = self
            .targets
            .iter()
            .any(|item| item.scope == target.scope && item.id == target.id);
        debug_assert!(!duplicate, "duplicate focus target in one scope");
        if !duplicate {
            self.targets.push(target);
        }
    }

    /// Registers an ordered group with independent geometry and enabled state.
    pub fn register_order(
        &mut self,
        scope: ScopeId,
        targets: impl IntoIterator<Item = (Id, Option<Rect>, bool)>,
    ) {
        for (id, area, enabled) in targets {
            self.register(FocusTarget {
                id,
                scope: scope.clone(),
                area,
                enabled,
            });
        }
    }

    /// Attaches canonical painted geometry after a composite widget renders.
    pub fn attach_region(&mut self, scope: &ScopeId, id: &Id, area: Rect) -> bool {
        let Some(target) = self
            .targets
            .iter_mut()
            .find(|target| &target.scope == scope && &target.id == id)
        else {
            return false;
        };
        target.area = Some(area);
        true
    }

    /// Reconciles focus against the active scope's current enabled targets.
    pub fn reconcile(&mut self) -> FocusOutcome<Id> {
        let before = self.focused.clone();
        let eligible = self.eligible_ids();
        if eligible.is_empty() {
            self.pending_restore_index = None;
            self.focused = None;
            return self.outcome(before);
        }
        if self
            .focused
            .as_ref()
            .is_some_and(|focused| eligible.contains(focused))
        {
            self.pending_restore_index = None;
            return FocusOutcome::Unchanged;
        }
        let previous_index = self.pending_restore_index.take().or_else(|| {
            self.focused.as_ref().and_then(|focused| {
                let active = self.active_scope();
                self.previous_targets
                    .iter()
                    .filter(|target| &target.scope == active && target.enabled)
                    .position(|target| &target.id == focused)
            })
        });
        self.focused = Some(eligible[previous_index.unwrap_or(0).min(eligible.len() - 1)].clone());
        self.outcome(before)
    }

    /// Handles inter-widget Tab and BackTab traversal.
    pub fn handle_key(&mut self, key: KeyEvent) -> FocusOutcome<Id> {
        if key.kind == KeyEventKind::Release {
            return FocusOutcome::Ignored;
        }
        match key.code {
            KeyCode::Tab if key.modifiers.is_empty() => self.move_relative(false),
            KeyCode::BackTab
                if key.modifiers.is_empty()
                    || key.modifiers == crate::input::KeyModifiers::SHIFT =>
            {
                self.move_relative(true)
            }
            _ => FocusOutcome::Ignored,
        }
    }

    /// Requests focus when the identity is enabled in the active scope.
    pub fn request_focus(&mut self, id: Id) -> FocusOutcome<Id> {
        if !self
            .targets
            .iter()
            .any(|target| target.id == id && &target.scope == self.active_scope() && target.enabled)
        {
            return FocusOutcome::Ignored;
        }
        let before = self.focused.replace(id);
        self.outcome(before)
    }

    /// Focuses the first enabled active-scope target containing `position`.
    pub fn focus_at(&mut self, position: Position) -> FocusOutcome<Id> {
        let active = self.active_scope();
        let Some(id) = self
            .targets
            .iter()
            .find(|target| {
                &target.scope == active
                    && target.enabled
                    && target.area.is_some_and(|area| area.contains(position))
            })
            .map(|target| target.id.clone())
        else {
            return FocusOutcome::Ignored;
        };
        let before = self.focused.replace(id);
        self.outcome(before)
    }

    /// Opens a root modal and pushes its matching focus scope atomically.
    pub fn open_modal<M>(&mut self, modals: &mut ModalStack<M>, modal: M, scope: ScopeId) {
        while self.scopes.len() > 1 {
            self.pop_scope();
        }
        modals.open(modal);
        self.push_scope(scope);
    }

    /// Opens a child modal and pushes its matching focus scope atomically.
    pub fn open_submodal<M>(&mut self, modals: &mut ModalStack<M>, child: M, scope: ScopeId) {
        modals.open_sub(child);
        self.push_scope(scope);
    }

    /// Pops the current modal and restores its parent scope focus atomically.
    pub fn pop_modal<M>(&mut self, modals: &mut ModalStack<M>) {
        if !modals.is_open() {
            return;
        }
        modals.pop();
        self.pop_scope();
    }

    /// Clears the full modal chain and restores the root opener atomically.
    pub fn clear_modals<M>(&mut self, modals: &mut ModalStack<M>) {
        modals.clear_chain();
        while self.scopes.len() > 1 {
            self.pop_scope();
        }
    }

    /// Returns the focused stable identity.
    #[must_use]
    pub const fn focused(&self) -> Option<&Id> {
        self.focused.as_ref()
    }

    /// Returns whether `id` currently owns focus.
    #[must_use]
    pub fn is_focused(&self, id: &Id) -> bool {
        self.focused.as_ref() == Some(id)
    }

    /// Projects focus into semantic panel emphasis without changing glyphs.
    #[must_use]
    pub fn panel_emphasis_for(&self, id: &Id) -> PanelEmphasis {
        if self.is_focused(id) {
            PanelEmphasis::Focused
        } else {
            PanelEmphasis::Normal
        }
    }

    /// Returns the active (topmost) focus scope.
    #[must_use]
    pub fn active_scope(&self) -> &ScopeId {
        &self.scopes.last().expect("root focus scope must exist").id
    }

    fn push_scope(&mut self, scope: ScopeId) {
        let active = self.active_scope().clone();
        let pending_restore_index = self.pending_restore_index.take();
        let restore_index = self
            .focused
            .as_ref()
            .and_then(|focused| {
                self.targets
                    .iter()
                    .filter(|target| target.scope == active && target.enabled)
                    .position(|target| &target.id == focused)
            })
            .or(pending_restore_index);
        self.scopes.push(FocusScope {
            id: scope,
            restore: self.focused.take(),
            restore_index,
        });
    }

    fn pop_scope(&mut self) {
        if self.scopes.len() == 1 {
            return;
        }
        let frame = self.scopes.pop().expect("checked non-root scope");
        self.focused = frame.restore;
        self.pending_restore_index = frame.restore_index;
    }

    fn eligible_ids(&self) -> Vec<Id> {
        let active = self.active_scope();
        self.targets
            .iter()
            .filter(|target| &target.scope == active && target.enabled)
            .map(|target| target.id.clone())
            .collect()
    }

    fn move_relative(&mut self, reverse: bool) -> FocusOutcome<Id> {
        let eligible = self.eligible_ids();
        if eligible.is_empty() {
            return FocusOutcome::Unchanged;
        }
        let current = self
            .focused
            .as_ref()
            .and_then(|focused| eligible.iter().position(|id| id == focused));
        let next = match (current, reverse) {
            (Some(0) | None, true) => eligible.len() - 1,
            (Some(index), true) => index - 1,
            (Some(index), false) => (index + 1) % eligible.len(),
            (None, false) => 0,
        };
        let before = self.focused.clone();
        self.focused = Some(eligible[next].clone());
        self.outcome(before)
    }

    fn outcome(&self, before: Option<Id>) -> FocusOutcome<Id> {
        if before == self.focused {
            FocusOutcome::Unchanged
        } else {
            FocusOutcome::Changed {
                from: before,
                to: self.focused.clone(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::input::{KeyEvent, KeyModifiers};

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Id {
        First,
        Second,
        Third,
        ModalFirst,
        ModalDisabled,
        ModalLast,
        Child,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Scope {
        Root,
        Modal,
        Child,
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn root(ring: &mut FocusRing<Id, Scope>, third: bool) {
        ring.begin_frame();
        ring.register_order(
            Scope::Root,
            [
                (Id::First, Some(Rect::new(0, 0, 2, 2)), true),
                (Id::Second, Some(Rect::new(2, 0, 2, 2)), true),
                (Id::Third, Some(Rect::new(4, 0, 2, 2)), third),
            ],
        );
        let _ = ring.reconcile();
    }

    #[test]
    fn traversal_wraps_reverses_and_reconciles_removed_target() {
        let mut ring = FocusRing::new(Scope::Root, Some(Id::First));
        root(&mut ring, true);
        assert!(matches!(
            ring.handle_key(key(KeyCode::BackTab)),
            FocusOutcome::Changed { .. }
        ));
        assert_eq!(ring.focused(), Some(&Id::Third));
        root(&mut ring, false);
        assert_eq!(ring.focused(), Some(&Id::Second));
        assert!(matches!(
            ring.handle_key(key(KeyCode::Tab)),
            FocusOutcome::Changed { .. }
        ));
        assert_eq!(ring.focused(), Some(&Id::First));
    }

    #[test]
    fn modified_tab_chords_remain_consumer_owned() {
        let mut ring = FocusRing::new(Scope::Root, Some(Id::First));
        root(&mut ring, true);
        let modified = KeyEvent::new(KeyCode::Tab, KeyModifiers::CONTROL);
        assert_eq!(ring.handle_key(modified), FocusOutcome::Ignored);
        assert_eq!(ring.focused(), Some(&Id::First));

        let reverse = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        assert!(matches!(
            ring.handle_key(reverse),
            FocusOutcome::Changed { .. }
        ));
        assert_eq!(ring.focused(), Some(&Id::Third));
    }

    #[test]
    fn empty_scope_traps_tab_and_pointer_requires_attached_region() {
        let mut ring = FocusRing::new(Scope::Root, None);
        ring.register(FocusTarget {
            id: Id::First,
            scope: Scope::Root,
            area: None,
            enabled: true,
        });
        let _ = ring.reconcile();
        assert_eq!(ring.focus_at(Position::new(0, 0)), FocusOutcome::Ignored);
        assert!(ring.attach_region(&Scope::Root, &Id::First, Rect::new(0, 0, 2, 2)));
        assert_eq!(ring.focus_at(Position::new(0, 0)), FocusOutcome::Unchanged);

        let mut modals = ModalStack::new();
        ring.open_modal(&mut modals, "modal", Scope::Modal);
        ring.begin_frame();
        assert_eq!(ring.reconcile(), FocusOutcome::Unchanged);
        assert_eq!(ring.handle_key(key(KeyCode::Tab)), FocusOutcome::Unchanged);
        assert_eq!(ring.request_focus(Id::First), FocusOutcome::Ignored);
    }

    #[test]
    fn modal_lifecycle_traps_skips_disabled_and_restores_nested_openers() {
        let mut ring = FocusRing::new(Scope::Root, Some(Id::Second));
        let mut modals = ModalStack::new();
        root(&mut ring, true);
        ring.open_modal(&mut modals, "parent", Scope::Modal);
        ring.begin_frame();
        ring.register_order(
            Scope::Modal,
            [
                (Id::ModalFirst, None, true),
                (Id::ModalDisabled, None, false),
                (Id::ModalLast, None, true),
            ],
        );
        let _ = ring.reconcile();
        assert_eq!(ring.focused(), Some(&Id::ModalFirst));
        let _ = ring.handle_key(key(KeyCode::Tab));
        assert_eq!(ring.focused(), Some(&Id::ModalLast));

        ring.open_submodal(&mut modals, "child", Scope::Child);
        ring.begin_frame();
        ring.register_order(Scope::Child, [(Id::Child, None, true)]);
        let _ = ring.reconcile();
        assert_eq!(modals.depth(), 2);
        assert_eq!(ring.focused(), Some(&Id::Child));
        ring.pop_modal(&mut modals);
        assert_eq!(ring.focused(), Some(&Id::ModalLast));
        ring.clear_modals(&mut modals);
        assert_eq!(ring.focused(), Some(&Id::Second));
        assert!(!modals.is_open());
    }

    #[test]
    fn removed_root_opener_restores_nearest_survivor() {
        let mut ring = FocusRing::new(Scope::Root, Some(Id::Third));
        let mut modals = ModalStack::new();
        root(&mut ring, true);
        ring.open_modal(&mut modals, "modal", Scope::Modal);
        ring.begin_frame();
        ring.register_order(
            Scope::Root,
            [(Id::First, None, true), (Id::Second, None, true)],
        );
        ring.register_order(Scope::Modal, [(Id::ModalFirst, None, true)]);
        let _ = ring.reconcile();
        ring.pop_modal(&mut modals);
        let _ = ring.reconcile();
        assert_eq!(ring.focused(), Some(&Id::Second));
    }

    #[test]
    fn immediate_modal_reopen_does_not_leak_parent_restore_index_into_child() {
        let mut ring = FocusRing::new(Scope::Root, Some(Id::Second));
        let mut modals = ModalStack::new();
        root(&mut ring, true);
        ring.open_modal(&mut modals, "first", Scope::Modal);
        ring.begin_frame();
        ring.register_order(
            Scope::Root,
            [
                (Id::First, None, true),
                (Id::Second, None, true),
                (Id::Third, None, true),
            ],
        );
        ring.register_order(
            Scope::Modal,
            [(Id::ModalFirst, None, true), (Id::ModalLast, None, true)],
        );
        let _ = ring.reconcile();
        ring.pop_modal(&mut modals);

        ring.open_modal(&mut modals, "replacement", Scope::Modal);
        ring.begin_frame();
        ring.register_order(
            Scope::Modal,
            [(Id::ModalFirst, None, true), (Id::ModalLast, None, true)],
        );
        let _ = ring.reconcile();

        assert_eq!(ring.focused(), Some(&Id::ModalFirst));
    }

    #[test]
    #[should_panic(expected = "duplicate focus target in one scope")]
    fn duplicate_target_is_rejected() {
        let mut ring = FocusRing::new(Scope::Root, None);
        ring.register_order(
            Scope::Root,
            [(Id::First, None, true), (Id::First, None, true)],
        );
    }
}

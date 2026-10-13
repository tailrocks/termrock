//! Local reference implementation for per-frame focus registration.

use ratatui::layout::{Position, Rect};
use termrock::{input::KeyCode, widgets::PanelEmphasis};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusId {
    Sidebar,
    Preview,
    Controls,
    ModalContinue,
    ModalDisabled,
    ModalCancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusScope {
    Screen,
    Modal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Registration {
    id: FocusId,
    scope: FocusScope,
    area: Rect,
    enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScopeFrame {
    scope: FocusScope,
    restore: Option<FocusId>,
    restore_index: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct FocusRing {
    focused: Option<FocusId>,
    registrations: Vec<Registration>,
    previous_registrations: Vec<Registration>,
    scopes: Vec<ScopeFrame>,
    pending_restore_index: Option<usize>,
}

impl FocusRing {
    pub(crate) fn new(focused: Option<FocusId>) -> Self {
        Self {
            focused,
            registrations: Vec::new(),
            previous_registrations: Vec::new(),
            scopes: vec![ScopeFrame {
                scope: FocusScope::Screen,
                restore: None,
                restore_index: None,
            }],
            pending_restore_index: None,
        }
    }

    pub(crate) fn begin_frame(&mut self) {
        self.previous_registrations.clear();
        self.previous_registrations
            .extend(self.registrations.iter().copied());
        self.registrations.clear();
    }

    pub(crate) fn register(&mut self, scope: FocusScope, id: FocusId, area: Rect, enabled: bool) {
        let duplicate = self
            .registrations
            .iter()
            .any(|registration| registration.scope == scope && registration.id == id);
        debug_assert!(!duplicate, "duplicate focus target in one scope");
        if duplicate {
            return;
        }
        self.registrations.push(Registration {
            id,
            scope,
            area,
            enabled,
        });
    }

    pub(crate) fn reconcile(&mut self) {
        let eligible = self.eligible();
        if eligible.is_empty() {
            self.pending_restore_index = None;
            self.focused = None;
            return;
        }
        if self
            .focused
            .is_some_and(|focused| eligible.contains(&focused))
        {
            self.pending_restore_index = None;
            return;
        }
        let previous_index = self.pending_restore_index.take().or_else(|| {
            self.focused.and_then(|focused| {
                let active = self.active_scope();
                self.previous_registrations
                    .iter()
                    .filter(|registration| registration.scope == active && registration.enabled)
                    .position(|registration| registration.id == focused)
            })
        });
        let previous_index = previous_index.unwrap_or(0);
        self.focused = Some(eligible[previous_index.min(eligible.len() - 1)]);
    }

    pub(crate) fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Tab => self.move_relative(false),
            KeyCode::BackTab => self.move_relative(true),
            _ => false,
        }
    }

    fn move_relative(&mut self, reverse: bool) -> bool {
        let eligible = self.eligible();
        if eligible.is_empty() {
            return false;
        }
        let current = self
            .focused
            .and_then(|focused| eligible.iter().position(|id| *id == focused));
        let next = match (current, reverse) {
            (Some(0), true) | (None, true) => eligible.len() - 1,
            (Some(index), true) => index - 1,
            (Some(index), false) => (index + 1) % eligible.len(),
            (None, false) => 0,
        };
        let changed = self.focused != Some(eligible[next]);
        self.focused = Some(eligible[next]);
        changed
    }

    fn eligible(&self) -> Vec<FocusId> {
        let active = self.active_scope();
        self.registrations
            .iter()
            .filter(|registration| registration.scope == active && registration.enabled)
            .map(|registration| registration.id)
            .collect()
    }

    pub(crate) fn push_scope(&mut self, scope: FocusScope) {
        let active = self.active_scope();
        let restore_index = self.focused.and_then(|focused| {
            self.registrations
                .iter()
                .filter(|registration| registration.scope == active && registration.enabled)
                .position(|registration| registration.id == focused)
        });
        self.scopes.push(ScopeFrame {
            scope,
            restore: self.focused,
            restore_index,
        });
        self.focused = None;
    }

    pub(crate) fn pop_scope(&mut self) {
        if self.scopes.len() == 1 {
            return;
        }
        if let Some(frame) = self.scopes.pop() {
            self.focused = frame.restore;
            self.pending_restore_index = frame.restore_index;
        }
    }

    pub(crate) fn active_scope(&self) -> FocusScope {
        self.scopes
            .last()
            .map_or(FocusScope::Screen, |frame| frame.scope)
    }

    pub(crate) const fn focused(&self) -> Option<FocusId> {
        self.focused
    }

    pub(crate) fn is_focused(&self, id: FocusId) -> bool {
        matches!(self.focused, Some(focused) if focused == id)
    }

    pub(crate) fn panel_emphasis_for(&self, id: FocusId) -> PanelEmphasis {
        if self.is_focused(id) {
            PanelEmphasis::Focused
        } else {
            PanelEmphasis::Normal
        }
    }

    pub(crate) fn request_focus(&mut self, id: FocusId) -> bool {
        let active = self.active_scope();
        let allowed = self.registrations.iter().any(|registration| {
            registration.id == id && registration.scope == active && registration.enabled
        });
        if !allowed {
            return false;
        }
        let changed = self.focused != Some(id);
        self.focused = Some(id);
        changed
    }

    pub(crate) fn focus_at(&mut self, position: Position) -> bool {
        let active = self.active_scope();
        let Some(target) = self.registrations.iter().find(|registration| {
            registration.scope == active
                && registration.enabled
                && registration.area.contains(position)
        }) else {
            return false;
        };
        let changed = self.focused != Some(target.id);
        self.focused = Some(target.id);
        changed
    }

    pub(crate) fn attach_area(&mut self, scope: FocusScope, id: FocusId, area: Rect) -> bool {
        let Some(target) = self
            .registrations
            .iter_mut()
            .find(|registration| registration.scope == scope && registration.id == id)
        else {
            return false;
        };
        target.area = area;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn register_screen(ring: &mut FocusRing, controls_enabled: bool) {
        ring.begin_frame();
        ring.register(
            FocusScope::Screen,
            FocusId::Sidebar,
            Rect::new(0, 0, 10, 10),
            true,
        );
        ring.register(
            FocusScope::Screen,
            FocusId::Preview,
            Rect::new(10, 0, 10, 10),
            true,
        );
        ring.register(
            FocusScope::Screen,
            FocusId::Controls,
            Rect::new(20, 0, 10, 10),
            controls_enabled,
        );
        ring.reconcile();
    }

    #[test]
    fn traversal_wraps_and_skips_disabled_targets() {
        let mut ring = FocusRing::new(Some(FocusId::Sidebar));
        register_screen(&mut ring, false);

        assert!(ring.handle_key(KeyCode::Tab));
        assert_eq!(ring.focused(), Some(FocusId::Preview));
        assert!(ring.handle_key(KeyCode::Tab));
        assert_eq!(ring.focused(), Some(FocusId::Sidebar));
        assert!(ring.handle_key(KeyCode::BackTab));
        assert_eq!(ring.focused(), Some(FocusId::Preview));
    }

    #[test]
    fn disappearing_target_reconciles_to_nearest_survivor() {
        let mut ring = FocusRing::new(Some(FocusId::Controls));
        register_screen(&mut ring, true);
        register_screen(&mut ring, false);

        assert_eq!(ring.focused(), Some(FocusId::Preview));
    }

    #[test]
    fn modal_scope_traps_skips_disabled_and_restores_opener() {
        let mut ring = FocusRing::new(Some(FocusId::Preview));
        register_screen(&mut ring, true);
        ring.push_scope(FocusScope::Modal);
        ring.begin_frame();
        assert!(!ring.request_focus(FocusId::Sidebar));
        ring.register(
            FocusScope::Modal,
            FocusId::ModalContinue,
            Rect::new(0, 0, 1, 1),
            true,
        );
        ring.register(
            FocusScope::Modal,
            FocusId::ModalDisabled,
            Rect::new(1, 0, 1, 1),
            false,
        );
        ring.register(
            FocusScope::Modal,
            FocusId::ModalCancel,
            Rect::new(2, 0, 1, 1),
            true,
        );
        ring.reconcile();

        assert_eq!(ring.focused(), Some(FocusId::ModalContinue));
        assert!(ring.handle_key(KeyCode::Tab));
        assert_eq!(ring.focused(), Some(FocusId::ModalCancel));
        assert!(ring.handle_key(KeyCode::Tab));
        assert_eq!(ring.focused(), Some(FocusId::ModalContinue));
        assert!(!ring.request_focus(FocusId::Sidebar));
        assert_eq!(ring.focused(), Some(FocusId::ModalContinue));

        ring.pop_scope();
        assert_eq!(ring.focused(), Some(FocusId::Preview));
    }

    #[test]
    fn removed_modal_opener_restores_nearest_parent_target() {
        let mut ring = FocusRing::new(Some(FocusId::Controls));
        register_screen(&mut ring, true);
        ring.push_scope(FocusScope::Modal);
        ring.begin_frame();
        ring.register(
            FocusScope::Screen,
            FocusId::Sidebar,
            Rect::new(0, 0, 10, 10),
            true,
        );
        ring.register(
            FocusScope::Screen,
            FocusId::Preview,
            Rect::new(10, 0, 10, 10),
            true,
        );
        ring.register(
            FocusScope::Modal,
            FocusId::ModalContinue,
            Rect::new(0, 0, 1, 1),
            true,
        );
        ring.reconcile();
        ring.begin_frame();
        ring.register(
            FocusScope::Screen,
            FocusId::Sidebar,
            Rect::new(0, 0, 10, 10),
            true,
        );
        ring.register(
            FocusScope::Screen,
            FocusId::Preview,
            Rect::new(10, 0, 10, 10),
            true,
        );
        ring.register(
            FocusScope::Modal,
            FocusId::ModalContinue,
            Rect::new(0, 0, 1, 1),
            true,
        );
        ring.reconcile();
        ring.pop_scope();
        ring.reconcile();

        assert_eq!(ring.focused(), Some(FocusId::Preview));
    }

    #[test]
    #[should_panic(expected = "duplicate focus target in one scope")]
    fn duplicate_target_in_one_scope_is_rejected() {
        let mut ring = FocusRing::new(None);
        ring.register(
            FocusScope::Screen,
            FocusId::Sidebar,
            Rect::new(0, 0, 1, 1),
            true,
        );
        ring.register(
            FocusScope::Screen,
            FocusId::Sidebar,
            Rect::new(1, 0, 1, 1),
            true,
        );
    }
}

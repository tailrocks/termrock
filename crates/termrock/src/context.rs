// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Per-frame [`UiContext`] — coordination without a retained DOM.
//!
//! TermRock stays **Ratatui immediate-mode**: hosts still paint into
//! `Frame`/`Buffer`. [`UiContext`] only bundles the authorities components need
//! for one frame (design, capabilities, intents/keymap, focus, overlays,
//! semantics, clock, diagnostics).
//!
//! ## Lifetime ergonomics
//!
//! - Own long-lived state in [`UiHost`].
//! - Call [`UiHost::begin_frame`] once per loop iteration to get a short-lived
//!   [`UiContext`] with exclusive mut access to scene/focus/overlays/semantics.
//! - Nested widgets take `&mut UiContext` or only the slice they need
//!   (`ctx.design()`, `ctx.scene_mut()`).
//! - Tests use [`UiHost::test`] + [`FrameTick::manual`] — no terminal required.
//!
//! ## What this is not
//!
//! - Not a widget tree or React-style context provider.
//! - Not a replacement for `Frame` / `Buffer`.
//! - Not domain state (messages stay in the app).
use std::fmt;
use std::hash::Hash;

use crate::capability::CapabilityProfile;
use crate::capability::{CapabilityBoundary, TerminalCapabilities};
use crate::input::KeyEvent;
use crate::interaction::{
    FocusGraph, FocusOutcome, InteractionScene, OverlayOutcome, OverlayStack, SemanticScene,
};
use crate::keymap::{KeyChord, Keymap};
use crate::runtime::{FrameClock, FrameTick, Instant};
use crate::style::DesignSystem;

/// Lightweight diagnostics collected for Studio / tests (not a retained DOM).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UiDiagnostics {
    /// Free-form host notes (collision, missing focus, …).
    pub notes: Vec<String>,
    /// Frame counter (host increments via [`UiHost::begin_frame`]).
    pub frame_index: u64,
}

impl UiDiagnostics {
    /// Empty diagnostics.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            notes: Vec::new(),
            frame_index: 0,
        }
    }

    /// Push a note.
    pub fn note(&mut self, msg: impl Into<String>) {
        self.notes.push(msg.into());
    }

    /// True when any note is present.
    #[must_use]
    pub fn has_issues(&self) -> bool {
        !self.notes.is_empty()
    }
}

/// Per-frame coordination object (borrows host state).
///
/// Generic over focus id, layer id, scene action, and keymap action so hosts
/// keep their own ID types.
pub struct UiContext<'a, Id, LayerId = Id, Action = (), MapAction: Clone + 'static = ()>
where
    Id: Clone + Eq + Hash,
    LayerId: Clone + Eq + Hash,
{
    design: &'a DesignSystem,
    capabilities: &'a TerminalCapabilities,
    boundary: Option<&'a CapabilityBoundary>,
    keymap: Option<&'a Keymap<MapAction>>,
    scene: &'a mut InteractionScene<Id, LayerId, Action>,
    focus: &'a mut FocusGraph<Id>,
    overlays: &'a mut OverlayStack<Id>,
    semantics: &'a mut SemanticScene<Id, Action>,
    tick: FrameTick,
    diagnostics: &'a mut UiDiagnostics,
}

impl<'a, Id, LayerId, Action, MapAction> UiContext<'a, Id, LayerId, Action, MapAction>
where
    Id: Clone + Eq + Hash,
    LayerId: Clone + Eq + Hash,
    MapAction: Clone + 'static,
{
    /// Builds a context from raw borrows (advanced hosts / tests).
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        design: &'a DesignSystem,
        capabilities: &'a TerminalCapabilities,
        boundary: Option<&'a CapabilityBoundary>,
        keymap: Option<&'a Keymap<MapAction>>,
        scene: &'a mut InteractionScene<Id, LayerId, Action>,
        focus: &'a mut FocusGraph<Id>,
        overlays: &'a mut OverlayStack<Id>,
        semantics: &'a mut SemanticScene<Id, Action>,
        tick: FrameTick,
        diagnostics: &'a mut UiDiagnostics,
    ) -> Self {
        Self {
            design,
            capabilities,
            boundary,
            keymap,
            scene,
            focus,
            overlays,
            semantics,
            tick,
            diagnostics,
        }
    }

    // ── read-only ──────────────────────────────────────────────────────────

    /// Sole paint authority for this frame.
    #[must_use]
    pub const fn design(&self) -> &DesignSystem {
        self.design
    }

    /// Resolved terminal capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> &TerminalCapabilities {
        self.capabilities
    }

    /// Optional capability boundary for progressive enhancement.
    #[must_use]
    pub const fn boundary(&self) -> Option<&CapabilityBoundary> {
        self.boundary
    }

    /// Optional host keymap (semantic actions).
    #[must_use]
    pub const fn keymap(&self) -> Option<&Keymap<MapAction>> {
        self.keymap
    }

    /// Immutable frame time (never sample clocks in widgets).
    #[must_use]
    pub const fn tick(&self) -> FrameTick {
        self.tick
    }

    /// Frame index from diagnostics.
    #[must_use]
    pub fn frame_index(&self) -> u64 {
        self.diagnostics.frame_index
    }

    /// Interaction scene (focus/hit/layers).
    #[must_use]
    pub fn scene(&self) -> &InteractionScene<Id, LayerId, Action> {
        self.scene
    }

    /// Focus graph.
    #[must_use]
    pub fn focus(&self) -> &FocusGraph<Id> {
        self.focus
    }

    /// Overlay stack.
    #[must_use]
    pub fn overlays(&self) -> &OverlayStack<Id> {
        self.overlays
    }

    /// Semantic scene (a11y / Studio / snapshots).
    #[must_use]
    pub fn semantics(&self) -> &SemanticScene<Id, Action> {
        self.semantics
    }

    /// Diagnostics for this frame.
    #[must_use]
    pub fn diagnostics(&self) -> &UiDiagnostics {
        self.diagnostics
    }

    // ── mut ────────────────────────────────────────────────────────────────

    /// Mutable interaction scene.
    pub fn scene_mut(&mut self) -> &mut InteractionScene<Id, LayerId, Action> {
        self.scene
    }

    /// Mutable focus graph.
    pub fn focus_mut(&mut self) -> &mut FocusGraph<Id> {
        self.focus
    }

    /// Mutable overlay stack.
    pub fn overlays_mut(&mut self) -> &mut OverlayStack<Id> {
        self.overlays
    }

    /// Mutable semantic scene.
    pub fn semantics_mut(&mut self) -> &mut SemanticScene<Id, Action> {
        self.semantics
    }

    /// Mutable diagnostics.
    pub fn diagnostics_mut(&mut self) -> &mut UiDiagnostics {
        self.diagnostics
    }

    /// Record a diagnostic note.
    pub fn note(&mut self, msg: impl Into<String>) {
        self.diagnostics.note(msg);
    }

    // ── event adapters ─────────────────────────────────────────────────────

    /// Esc through overlay stack (peels at most one conceptual layer).
    pub fn handle_overlay_escape(&mut self) -> OverlayOutcome<Id> {
        self.overlays.handle_escape()
    }

    /// Outside / pointer press against overlay stack.
    pub fn handle_overlay_pointer_down(
        &mut self,
        position: ratatui_core::layout::Position,
    ) -> OverlayOutcome<Id> {
        self.overlays.handle_pointer_down(position)
    }

    /// Outside / pointer release against the overlay stack.
    ///
    /// Dismissal commits only when press and release occur outside the same
    /// top layer, so hosts must route both halves of the gesture.
    pub fn handle_overlay_pointer_up(
        &mut self,
        position: ratatui_core::layout::Position,
    ) -> OverlayOutcome<Id> {
        self.overlays.handle_pointer_up(position)
    }

    /// Request focus via FocusGraph (programmatic).
    pub fn request_focus(&mut self, id: Id) -> FocusOutcome<Id>
    where
        Id: PartialEq,
    {
        self.focus.request_focus(id)
    }
}

impl<Id, LayerId, Action, MapAction> fmt::Debug for UiContext<'_, Id, LayerId, Action, MapAction>
where
    Id: Clone + Eq + Hash + fmt::Debug,
    LayerId: Clone + Eq + Hash + fmt::Debug,
    Action: fmt::Debug,
    MapAction: Clone + 'static + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UiContext")
            .field("frame_index", &self.diagnostics.frame_index)
            .field("tick_elapsed_ms", &self.tick.elapsed_ms())
            .field("has_keymap", &self.keymap.is_some())
            .field("has_boundary", &self.boundary.is_some())
            .finish_non_exhaustive()
    }
}

/// Long-lived host state that produces a [`UiContext`] each frame.
///
/// Owns design, capabilities, scene, focus, overlays, semantics, and clock.
/// Domain app state stays outside this type.
#[derive(Debug)]
pub struct UiHost<Id, LayerId = Id, Action = (), MapAction: Clone + 'static = ()>
where
    Id: Clone + Eq + Hash,
    LayerId: Clone + Eq + Hash,
{
    /// Paint authority.
    pub design: DesignSystem,
    /// Terminal capabilities (resolved once / on change).
    pub capabilities: TerminalCapabilities,
    /// Optional boundary snapshot for widgets.
    pub boundary: Option<CapabilityBoundary>,
    /// Optional host keymap.
    pub keymap: Option<Keymap<MapAction>>,
    /// Hit / layer / Esc authority.
    pub scene: InteractionScene<Id, LayerId, Action>,
    /// Tab / spatial focus graph.
    pub focus: FocusGraph<Id>,
    /// Floating UI authority.
    pub overlays: OverlayStack<Id>,
    /// Frame-local semantic tree.
    pub semantics: SemanticScene<Id, Action>,
    /// Frame clock.
    pub clock: FrameClock,
    /// Diagnostics (frame index lives here).
    pub diagnostics: UiDiagnostics,
}

impl<Id, LayerId, Action, MapAction> Default for UiHost<Id, LayerId, Action, MapAction>
where
    Id: Clone + Eq + Hash,
    LayerId: Clone + Eq + Hash,
    MapAction: Clone + 'static,
{
    fn default() -> Self {
        Self::new(
            DesignSystem::default(),
            TerminalCapabilities::for_profile(CapabilityProfile::Modern),
        )
    }
}

impl<Id, LayerId, Action, MapAction> UiHost<Id, LayerId, Action, MapAction>
where
    Id: Clone + Eq + Hash,
    LayerId: Clone + Eq + Hash,
    MapAction: Clone + 'static,
{
    /// Creates a host with design + capabilities.
    #[must_use]
    pub fn new(design: DesignSystem, capabilities: TerminalCapabilities) -> Self {
        Self {
            design,
            capabilities,
            boundary: None,
            keymap: None,
            scene: InteractionScene::new(),
            focus: FocusGraph::new(),
            overlays: OverlayStack::new(),
            semantics: SemanticScene::new(),
            clock: FrameClock::start(),
            diagnostics: UiDiagnostics::new(),
        }
    }

    /// Test host: injectable clock start, default design/caps (Modern profile).
    #[must_use]
    pub fn test() -> Self {
        let mut host = Self::new(
            DesignSystem::default(),
            TerminalCapabilities::for_profile(CapabilityProfile::Modern),
        );
        host.clock = FrameClock::from_start(Instant::now());
        host
    }

    /// Test host with fixed design.
    #[must_use]
    pub fn test_with_design(design: DesignSystem) -> Self {
        let mut host = Self::new(
            design,
            TerminalCapabilities::for_profile(CapabilityProfile::Modern),
        );
        host.clock = FrameClock::from_start(Instant::now());
        host
    }

    /// Attach keymap.
    #[must_use]
    pub fn with_keymap(mut self, keymap: Keymap<MapAction>) -> Self {
        self.keymap = Some(keymap);
        self
    }

    /// Attach capability boundary.
    #[must_use]
    pub fn with_boundary(mut self, boundary: CapabilityBoundary) -> Self {
        self.boundary = Some(boundary);
        self
    }

    /// Begin a frame: clear scene elements + semantics, tick clock, return context.
    ///
    /// Layers and focus **persist** across frames (InteractionScene contract).
    /// Element registration is immediate-mode each frame.
    pub fn begin_frame(&mut self) -> UiContext<'_, Id, LayerId, Action, MapAction> {
        self.diagnostics.frame_index = self.diagnostics.frame_index.saturating_add(1);
        self.diagnostics.notes.clear();
        self.scene.begin_frame();
        self.semantics.begin_frame();
        let tick = self.clock.tick();
        UiContext {
            design: &self.design,
            capabilities: &self.capabilities,
            boundary: self.boundary.as_ref(),
            keymap: self.keymap.as_ref(),
            scene: &mut self.scene,
            focus: &mut self.focus,
            overlays: &mut self.overlays,
            semantics: &mut self.semantics,
            tick,
            diagnostics: &mut self.diagnostics,
        }
    }

    /// Begin a frame with an injected tick (tests / Studio replay).
    pub fn begin_frame_at(
        &mut self,
        tick: FrameTick,
    ) -> UiContext<'_, Id, LayerId, Action, MapAction> {
        self.diagnostics.frame_index = self.diagnostics.frame_index.saturating_add(1);
        self.diagnostics.notes.clear();
        self.scene.begin_frame();
        self.semantics.begin_frame();
        UiContext {
            design: &self.design,
            capabilities: &self.capabilities,
            boundary: self.boundary.as_ref(),
            keymap: self.keymap.as_ref(),
            scene: &mut self.scene,
            focus: &mut self.focus,
            overlays: &mut self.overlays,
            semantics: &mut self.semantics,
            tick,
            diagnostics: &mut self.diagnostics,
        }
    }
}

/// Map a physical key through optional keymap to a cloned app action.
///
/// Release events are ignored; repeat events remain dispatchable for actions
/// that intentionally support held keys. This adapter does not replace
/// widget-local intent maps; use it for global chords.
#[must_use]
pub fn resolve_keymap_action<A: Clone + Copy + 'static>(
    keymap: Option<&Keymap<A>>,
    key: KeyEvent,
) -> Option<A> {
    let map = keymap?;
    if key.is_release() {
        return None;
    }
    let chord = KeyChord::from(key);
    map.dispatch(chord)
}

// Re-export UiIntent at context level for discoverability in docs.
pub use crate::interaction::UiIntent as ContextUiIntent;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyEventKind, KeyModifiers};
    use crate::interaction::UiIntent;
    use crate::interaction::{
        InteractionElement, InteractionLayer, LayerDismissPolicy, LayerKind, SemanticNode,
        SemanticRole,
    };
    use crate::keymap::{KeyBinding, Visibility};
    use crate::style::DesignSystem;
    use ratatui_core::layout::Rect;
    use std::time::Duration;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum Fid {
        A,
        B,
    }

    impl fmt::Display for Fid {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::A => write!(f, "A"),
                Self::B => write!(f, "B"),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum Lid {
        Root,
    }

    #[test]
    fn begin_frame_clears_elements_keeps_layers() {
        let mut host = UiHost::<Fid, Lid>::test();
        host.scene.ensure_root(InteractionLayer {
            id: Lid::Root,
            kind: LayerKind::Root,
            owns_input: true,
            esc: LayerDismissPolicy::Ignore,
            outside: LayerDismissPolicy::Ignore,
            focus_return: None,
        });
        {
            let mut ctx = host.begin_frame();
            let _ = ctx.scene_mut().register(
                InteractionElement::control(Fid::A, Lid::Root, Rect::new(0, 0, 10, 1))
                    .role(SemanticRole::Control)
                    .focusable(true),
            );
            assert_eq!(ctx.scene().elements().len(), 1);
        }
        // Next frame: elements cleared, root layer remains.
        let ctx = host.begin_frame();
        assert!(ctx.scene().elements().is_empty());
        assert!(!ctx.scene().layers().is_empty());
        assert_eq!(ctx.frame_index(), 2);
    }

    #[test]
    fn design_and_tick_accessible() {
        let design = DesignSystem::junie();
        let mut host = UiHost::<Fid, Lid>::test_with_design(design.clone());
        let tick = FrameTick::manual(
            Instant::now(),
            Duration::from_millis(100),
            Duration::from_millis(16),
        );
        let ctx = host.begin_frame_at(tick);
        assert_eq!(
            ctx.design().style(crate::style::Role::Accent).fg,
            design.style(crate::style::Role::Accent).fg
        );
        assert_eq!(ctx.tick().elapsed_ms(), 100);
    }

    #[test]
    fn nested_register_semantics() {
        let mut host = UiHost::<Fid, Lid>::test();
        host.scene.ensure_root(InteractionLayer {
            id: Lid::Root,
            kind: LayerKind::Root,
            owns_input: true,
            esc: LayerDismissPolicy::Ignore,
            outside: LayerDismissPolicy::Ignore,
            focus_return: None,
        });
        let mut ctx = host.begin_frame();
        let _ = ctx.semantics_mut().register(
            SemanticNode::control(Fid::A, Rect::new(0, 0, 4, 1))
                .role(SemanticRole::Button)
                .label("Go"),
        );
        let _ = ctx.semantics_mut().register(
            SemanticNode::control(Fid::B, Rect::new(0, 1, 10, 1))
                .role(SemanticRole::Input)
                .label("Name"),
        );
        assert_eq!(ctx.semantics().nodes().len(), 2);
        ctx.note("test note");
        assert!(ctx.diagnostics().has_issues() || !ctx.diagnostics().notes.is_empty());
    }

    #[test]
    fn focus_request_via_context() {
        use crate::interaction::FocusNode;
        let mut host = UiHost::<Fid, Lid>::test();
        let mut ctx = host.begin_frame();
        ctx.focus_mut()
            .register(FocusNode::leaf(Fid::A, Rect::new(0, 0, 4, 1)));
        ctx.focus_mut()
            .register(FocusNode::leaf(Fid::B, Rect::new(0, 1, 4, 1)));
        let _ = ctx.focus_mut().request_focus(Fid::A);
        assert_eq!(ctx.focus().focused(), Some(&Fid::A));
        let _ = ctx.request_focus(Fid::B);
        assert_eq!(ctx.focus().focused(), Some(&Fid::B));
    }

    #[test]
    fn context_routes_overlay_pointer_gesture_and_one_escape_layer() {
        use crate::interaction::{OverlayOutcome, OverlaySize, OverlaySpec};
        use ratatui_core::layout::Position;

        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(20, 10, 1, 1);
        let mut host = UiHost::<Fid, Lid>::test();
        let mut ctx = host.begin_frame();
        let _ = ctx.overlays_mut().open(
            bounds,
            OverlaySpec::menu("menu", anchor, OverlaySize::menu(16, 5), Some(Fid::A)),
        );
        assert_eq!(
            ctx.handle_overlay_pointer_down(Position::new(0, 0)),
            OverlayOutcome::Ignored
        );
        assert!(matches!(
            ctx.handle_overlay_pointer_up(Position::new(0, 0)),
            OverlayOutcome::Dismissed {
                focus: Some(Fid::A),
                ..
            }
        ));

        let _ = ctx.overlays_mut().open(
            bounds,
            OverlaySpec::menu("first", anchor, OverlaySize::menu(16, 5), None),
        );
        let _ = ctx.overlays_mut().open(
            bounds,
            OverlaySpec::menu("second", anchor, OverlaySize::menu(16, 5), None),
        );
        assert!(ctx.handle_overlay_escape().is_dismissed());
        assert_eq!(ctx.overlays().len(), 1, "Esc peels exactly one layer");
    }

    #[test]
    fn resolve_keymap_action_uses_canonical_key_event_conversion() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Action {
            Uppercase,
        }

        let keymap = Keymap::from_owned(vec![KeyBinding::owned(
            vec![KeyChord::plain(KeyCode::Char('Q'))],
            Action::Uppercase,
            Some("uppercase".to_owned()),
            Visibility::Shown,
            None,
        )]);
        let shifted_q = KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::SHIFT);
        assert_eq!(
            resolve_keymap_action(Some(&keymap), shifted_q),
            Some(Action::Uppercase)
        );

        let mut repeat = shifted_q;
        repeat.kind = KeyEventKind::Repeat;
        assert_eq!(
            resolve_keymap_action(Some(&keymap), repeat),
            Some(Action::Uppercase)
        );

        let mut release = KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert_eq!(resolve_keymap_action(Some(&keymap), release), None);
    }

    #[test]
    fn many_frames_are_cheap() {
        let mut host = UiHost::<Fid, Lid>::test();
        host.scene.ensure_root(InteractionLayer {
            id: Lid::Root,
            kind: LayerKind::Root,
            owns_input: true,
            esc: LayerDismissPolicy::Ignore,
            outside: LayerDismissPolicy::Ignore,
            focus_return: None,
        });
        for _ in 0..5_000 {
            let mut ctx = host.begin_frame();
            let _ = ctx.scene_mut().register(
                InteractionElement::control(Fid::A, Lid::Root, Rect::new(0, 0, 4, 1))
                    .focusable(true),
            );
        }
        assert!(host.diagnostics.frame_index >= 5_000);
    }

    #[test]
    fn unused_intent_import_typecheck() {
        let _ = UiIntent::Activate;
    }
}

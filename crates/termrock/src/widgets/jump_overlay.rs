// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **JumpMode** — terminal-native direct navigation over [`SemanticScene`] regions.
//!
//! **Mission.** Label focusable / actionable geometry with short collision-free
//! keys (easymotion-style), filter by role/action/label, support nested targets
//! and multi-key prefixes, then activate or cancel — without requiring widgets
//! to implement anything beyond semantic registration.
//!
//! **vs FocusLens.** JumpMode is *operational navigation* (activate by key).
//! [`crate::interaction::FocusLens`] is *inspection* (tab-order / focus debug).
//! Both read the same scene/graph; neither mutates component internals.
//!
//! Research: Vim easymotion, browser keyboard nav extensions, Posting jump,
//! accessibility focus inspectors.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::Widget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{
        HitRegion, OverlayId, OverlayOutcome, OverlaySpec, OverlayStack, SemanticNode,
        SemanticRole, SemanticScene, UiIntent,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

/// Default overlay id for jump mode (fullscreen-class, owns input).
pub const JUMP_OVERLAY_ID: &str = "termrock.jump";
/// Alphabet for single-key then multi-key expansion (deterministic).
pub const JUMP_LABEL_ALPHABET: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z',
];

// ── Overlay helpers ─────────────────────────────────────────────────────────

/// Opens jump mode as a fullscreen overlay layer (owns input; Esc dismissible).
pub fn open_jump_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::fullscreen(JUMP_OVERLAY_ID, opener_focus).with_policy(
            crate::interaction::OverlayPolicy {
                esc: crate::interaction::LayerDismissPolicy::Dismissible,
                outside: crate::interaction::LayerDismissPolicy::Dismissible,
                owns_input: true,
                focus_trap: true,
                wheel_captures: true,
                backdrop: crate::interaction::BackdropPolicy::None,
                prefer: crate::interaction::PlacementPrefer::Fullscreen,
                cover_anchor: true,
                narrow_fallback: crate::interaction::NarrowFallback::Fullscreen,
                narrow_cols: 0,
            },
        ),
    )
}

/// Dismisses the default jump overlay when present.
pub fn dismiss_jump_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(JUMP_OVERLAY_ID))
}

// ── Filter ──────────────────────────────────────────────────────────────────

/// Filter for which semantic nodes participate in JumpMode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JumpFilter {
    /// Restrict to these roles (`None` = any focusable/actionable).
    pub roles: Option<Vec<SemanticRole>>,
    /// Require an action whose `Display` contains this substring (case-insensitive).
    pub action_contains: Option<String>,
    /// Require label/id containing this substring (case-insensitive).
    pub label_contains: Option<String>,
    /// Include disabled nodes (default false).
    pub include_disabled: bool,
    /// Include hidden nodes (default false).
    pub include_hidden: bool,
    /// Include non-focusable nodes that still advertise actions (default true).
    pub include_actionable: bool,
    /// Max nesting depth (`None` = unlimited; root = 0).
    pub max_depth: Option<u8>,
    /// Only leaf nodes (no children in the scene tree).
    pub only_leaves: bool,
}

impl JumpFilter {
    /// Default: focusable + actionable, enabled, visible.
    #[must_use]
    pub fn new() -> Self {
        Self {
            include_actionable: true,
            ..Default::default()
        }
    }

    /// Roles allow-list.
    #[must_use]
    pub fn roles(mut self, roles: impl IntoIterator<Item = SemanticRole>) -> Self {
        self.roles = Some(roles.into_iter().collect());
        self
    }

    /// Action substring filter.
    #[must_use]
    pub fn action_contains(mut self, s: impl Into<String>) -> Self {
        self.action_contains = Some(s.into());
        self
    }

    /// Label substring filter.
    #[must_use]
    pub fn label_contains(mut self, s: impl Into<String>) -> Self {
        self.label_contains = Some(s.into());
        self
    }

    /// Include disabled.
    #[must_use]
    pub const fn include_disabled(mut self, on: bool) -> Self {
        self.include_disabled = on;
        self
    }

    /// Max depth.
    #[must_use]
    pub const fn max_depth(mut self, d: u8) -> Self {
        self.max_depth = Some(d);
        self
    }

    /// Only leaves.
    #[must_use]
    pub const fn only_leaves(mut self, on: bool) -> Self {
        self.only_leaves = on;
        self
    }
}

// ── Targets ─────────────────────────────────────────────────────────────────

/// One jump target with a collision-free key sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpTarget<Id> {
    /// Stable identity activated when keys match.
    pub id: Id,
    /// Painted geometry for the badge anchor (top-left of the region).
    pub area: Rect,
    /// Key sequence (`"a"`, `"ab"`, …) — lowercase ASCII letters.
    pub keys: String,
    /// Semantic role when known (filtering / FocusLens).
    pub role: Option<SemanticRole>,
    /// Nesting depth in the semantic tree (0 = root).
    pub depth: u8,
    /// Display label for dim/help chrome.
    pub label: Option<String>,
}

impl<Id> JumpTarget<Id> {
    /// Construct with keys.
    #[must_use]
    pub fn new(id: Id, area: Rect, keys: impl Into<String>) -> Self {
        Self {
            id,
            area,
            keys: keys.into(),
            role: None,
            depth: 0,
            label: None,
        }
    }

    /// Badge character derived from the first key in the sequence.
    #[must_use]
    pub fn badge(&self) -> char {
        self.keys.chars().next().unwrap_or('?')
    }

    /// Role.
    #[must_use]
    pub const fn role(mut self, role: SemanticRole) -> Self {
        self.role = Some(role);
        self
    }

    /// Depth.
    #[must_use]
    pub const fn depth(mut self, d: u8) -> Self {
        self.depth = d;
        self
    }

    /// Label.
    #[must_use]
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
}

/// Outcome of jump-mode input.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum JumpOutcome<Id> {
    /// Event not applicable.
    Ignored,
    /// Jump mode dismissed without activation.
    Dismissed,
    /// Prefix buffer advanced (multi-key); still open.
    Prefix {
        /// Current prefix.
        keys: String,
        /// How many targets still match.
        remaining: usize,
    },
    /// Target activated by key sequence or click.
    Activated(Id),
    /// Filter changed; host should rebuild targets.
    FilterChanged,
}

// ── Label generation (deterministic, collision-free) ────────────────────────

/// Generate `n` unique **prefix-free** labels (equal length within a batch).
///
/// - `n ≤ 26` → single letters `a`…  
/// - larger `n` → all labels share length 2+ (`aa`, `ab`, …) so no key is a
///   prefix of another (required for multi-key easymotion).
///
/// Deterministic for replay tests — same `n` always yields the same sequence.
#[must_use]
pub fn generate_jump_labels(n: usize) -> Vec<String> {
    if n == 0 {
        return Vec::new();
    }
    let alpha = JUMP_LABEL_ALPHABET;
    let base = alpha.len();
    // Minimum length so base^len >= n
    let mut len = 1usize;
    let mut capacity = base;
    while capacity < n {
        len += 1;
        capacity = capacity.saturating_mul(base);
        if len > 6 {
            break;
        }
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut x = i;
        let mut chars = vec![alpha[0]; len];
        for pos in (0..len).rev() {
            chars[pos] = alpha[x % base];
            x /= base;
        }
        out.push(chars.into_iter().collect());
    }
    out
}

/// Assign sequential labels to hit regions.
#[must_use]
pub fn assign_jump_badges<Id: Clone>(regions: &[HitRegion<Id>]) -> Vec<JumpTarget<Id>> {
    let labels = generate_jump_labels(regions.len());
    regions
        .iter()
        .zip(labels)
        .map(|(region, keys)| JumpTarget {
            id: region.id.clone(),
            area: region.area,
            keys,
            role: None,
            depth: 0,
            label: None,
        })
        .collect()
}

/// Assign labels to an ordered candidate list (already filtered).
#[must_use]
pub fn assign_jump_labels<Id: Clone>(
    candidates: impl IntoIterator<Item = JumpCandidate<Id>>,
) -> Vec<JumpTarget<Id>> {
    let cands: Vec<_> = candidates.into_iter().collect();
    let labels = generate_jump_labels(cands.len());
    cands
        .into_iter()
        .zip(labels)
        .map(|(c, keys)| JumpTarget {
            id: c.id,
            area: c.area,
            keys,
            role: c.role,
            depth: c.depth,
            label: c.label,
        })
        .collect()
}

/// Intermediate candidate before label assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpCandidate<Id> {
    /// Id.
    pub id: Id,
    /// Area.
    pub area: Rect,
    /// Role.
    pub role: Option<SemanticRole>,
    /// Depth.
    pub depth: u8,
    /// Label.
    pub label: Option<String>,
}

// ── Scene collection ────────────────────────────────────────────────────────

fn node_depth<Id: PartialEq, Action>(scene: &SemanticScene<Id, Action>, id: &Id) -> u8 {
    let mut depth = 0u8;
    let mut cur = scene.nodes().iter().find(|n| &n.id == id);
    while let Some(n) = cur {
        if let Some(parent) = &n.parent {
            depth = depth.saturating_add(1);
            cur = scene.nodes().iter().find(|x| &x.id == parent);
        } else {
            break;
        }
        if depth == u8::MAX {
            break;
        }
    }
    depth
}

fn has_children<Id: PartialEq, Action>(scene: &SemanticScene<Id, Action>, id: &Id) -> bool {
    scene.nodes().iter().any(|n| n.parent.as_ref() == Some(id))
}

fn action_matches<Action: std::fmt::Debug>(actions: &[Action], needle: &str) -> bool {
    let n = needle.to_ascii_lowercase();
    actions.iter().any(|a| {
        let s = format!("{a:?}").to_ascii_lowercase();
        s.contains(&n)
    })
}

/// Collect jump candidates from a semantic scene under `filter`.
///
/// Components participate only via prior semantic registration — this never
/// reaches into widget state machines.
#[must_use]
pub fn collect_jump_candidates<Id, Action>(
    scene: &SemanticScene<Id, Action>,
    filter: &JumpFilter,
) -> Vec<JumpCandidate<Id>>
where
    Id: Clone + PartialEq + std::fmt::Display,
    Action: Clone + std::fmt::Debug,
{
    let mut out = Vec::new();
    for node in scene.nodes() {
        if !filter.include_hidden && node.hidden {
            continue;
        }
        if !filter.include_disabled && node.disabled {
            continue;
        }
        if node.area.width == 0 || node.area.height == 0 {
            continue;
        }
        let actionable = !node.actions.is_empty();
        let eligible = node.focusable || (filter.include_actionable && actionable);
        if !eligible {
            continue;
        }
        if let Some(roles) = &filter.roles {
            if !roles.contains(&node.role) {
                continue;
            }
        }
        if let Some(ac) = &filter.action_contains {
            if !action_matches(&node.actions, ac) {
                continue;
            }
        }
        let display_label = node.label.clone().unwrap_or_else(|| node.id.to_string());
        if let Some(lc) = &filter.label_contains {
            if !display_label
                .to_ascii_lowercase()
                .contains(&lc.to_ascii_lowercase())
            {
                continue;
            }
        }
        let depth = node_depth(scene, &node.id);
        if let Some(max) = filter.max_depth {
            if depth > max {
                continue;
            }
        }
        if filter.only_leaves && has_children(scene, &node.id) {
            continue;
        }
        out.push(JumpCandidate {
            id: node.id.clone(),
            area: node.area,
            role: Some(node.role),
            depth,
            label: Some(display_label),
        });
    }
    // Stable order: depth then registration (y, x) for spatial predictability.
    out.sort_by(|a, b| {
        a.depth
            .cmp(&b.depth)
            .then_with(|| a.area.y.cmp(&b.area.y))
            .then_with(|| a.area.x.cmp(&b.area.x))
            .then_with(|| a.label.cmp(&b.label))
    });
    out
}

/// Builds labeled targets from a semantic scene (default filter).
#[must_use]
pub fn assign_jump_badges_from_semantics<Id, Action>(
    scene: &SemanticScene<Id, Action>,
) -> Vec<JumpTarget<Id>>
where
    Id: Clone + PartialEq + std::fmt::Display,
    Action: Clone + std::fmt::Debug,
{
    assign_jump_labels(collect_jump_candidates(scene, &JumpFilter::new()))
}

/// Builds labeled targets with an explicit filter.
#[must_use]
pub fn assign_jump_labels_from_semantics<Id, Action>(
    scene: &SemanticScene<Id, Action>,
    filter: &JumpFilter,
) -> Vec<JumpTarget<Id>>
where
    Id: Clone + PartialEq + std::fmt::Display,
    Action: Clone + std::fmt::Debug,
{
    assign_jump_labels(collect_jump_candidates(scene, filter))
}

// ── State ───────────────────────────────────────────────────────────────────

/// Jump-mode state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JumpOverlayState {
    open: bool,
    /// Multi-key prefix buffer.
    prefix: String,
    /// Active filter (host rebuilds targets when this changes).
    filter: JumpFilter,
    accepts_input: bool,
    /// Dim non-matching targets while prefix non-empty.
    dim_unmatched: bool,
}

impl JumpOverlayState {
    /// Creates closed jump state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: false,
            prefix: String::new(),
            filter: JumpFilter::new(),
            accepts_input: true,
            dim_unmatched: true,
        }
    }

    /// Opens jump mode (local flag; prefer [`Self::open_on_stack`]).
    pub fn open(&mut self) {
        self.open = true;
        self.prefix.clear();
    }

    /// Closes jump mode.
    pub fn close(&mut self) {
        self.open = false;
        self.prefix.clear();
    }

    /// Opens jump mode and registers a fullscreen-class layer on the overlay stack.
    pub fn open_on_stack<FocusId: Clone>(
        &mut self,
        stack: &mut OverlayStack<FocusId>,
        bounds: Rect,
        opener_focus: Option<FocusId>,
    ) -> OverlayOutcome<FocusId> {
        self.open();
        open_jump_overlay(stack, bounds, opener_focus)
    }

    /// Closes jump mode and dismisses the stack entry when present.
    pub fn close_on_stack<FocusId: Clone>(
        &mut self,
        stack: &mut OverlayStack<FocusId>,
    ) -> OverlayOutcome<FocusId> {
        self.close();
        dismiss_jump_overlay(stack)
    }

    /// Whether jump mode is active.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Current multi-key prefix.
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Active filter.
    #[must_use]
    pub fn filter(&self) -> &JumpFilter {
        &self.filter
    }

    /// Replace filter (returns [`JumpOutcome::FilterChanged`]).
    pub fn set_filter<Id>(&mut self, filter: JumpFilter) -> JumpOutcome<Id> {
        self.filter = filter;
        self.prefix.clear();
        JumpOutcome::FilterChanged
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Dim unmatched while typing a prefix.
    pub fn set_dim_unmatched(&mut self, on: bool) {
        self.dim_unmatched = on;
    }

    /// Whether dimming is enabled.
    #[must_use]
    pub const fn dim_unmatched(&self) -> bool {
        self.dim_unmatched
    }

    /// Targets matching current prefix (for paint).
    #[must_use]
    pub fn matching<'a, Id>(&self, targets: &'a [JumpTarget<Id>]) -> Vec<&'a JumpTarget<Id>> {
        if self.prefix.is_empty() {
            return targets.iter().collect();
        }
        let p = self.prefix.as_str();
        targets.iter().filter(|t| t.keys.starts_with(p)).collect()
    }

    /// Handles a key while jump mode is open.
    pub fn handle_key<Id: Clone>(
        &mut self,
        key: KeyEvent,
        targets: &[JumpTarget<Id>],
    ) -> JumpOutcome<Id> {
        if !self.open || !self.accepts_input || key.is_release() {
            return JumpOutcome::Ignored;
        }
        if key.is_insert() {
            // Allow Press primarily; ignore other.
        }
        match key.code {
            KeyCode::Esc => {
                if !self.prefix.is_empty() {
                    self.prefix.clear();
                    return JumpOutcome::Prefix {
                        keys: String::new(),
                        remaining: targets.len(),
                    };
                }
                self.close();
                JumpOutcome::Dismissed
            }
            KeyCode::Backspace => {
                if self.prefix.pop().is_some() {
                    let remaining = self.matching(targets).len();
                    JumpOutcome::Prefix {
                        keys: self.prefix.clone(),
                        remaining,
                    }
                } else {
                    JumpOutcome::Ignored
                }
            }
            KeyCode::Char(ch)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                    && ch.is_ascii_alphabetic() =>
            {
                let needle = ch.to_ascii_lowercase();
                self.prefix.push(needle);
                let matches: Vec<_> = self.matching(targets);
                if matches.is_empty() {
                    // Dead end — pop and ignore
                    self.prefix.pop();
                    return JumpOutcome::Ignored;
                }
                if let Some(exact) = matches.iter().find(|t| t.keys == self.prefix) {
                    let id = exact.id.clone();
                    self.close();
                    return JumpOutcome::Activated(id);
                }
                // Unique continuation that fully extends?
                if matches.len() == 1 {
                    let id = matches[0].id.clone();
                    self.close();
                    return JumpOutcome::Activated(id);
                }
                JumpOutcome::Prefix {
                    keys: self.prefix.clone(),
                    remaining: matches.len(),
                }
            }
            _ => JumpOutcome::Ignored,
        }
    }

    /// Intent path (Cancel → dismiss / clear prefix).
    pub fn handle_intent<Id: Clone>(
        &mut self,
        intent: UiIntent,
        targets: &[JumpTarget<Id>],
    ) -> JumpOutcome<Id> {
        if !self.open || !self.accepts_input {
            return JumpOutcome::Ignored;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close => {
                if !self.prefix.is_empty() {
                    self.prefix.clear();
                    JumpOutcome::Prefix {
                        keys: String::new(),
                        remaining: targets.len(),
                    }
                } else {
                    self.close();
                    JumpOutcome::Dismissed
                }
            }
            _ => JumpOutcome::Ignored,
        }
    }

    /// Handles a click against target regions.
    pub fn handle_click<Id: Clone>(
        &mut self,
        position: Position,
        targets: &[JumpTarget<Id>],
    ) -> JumpOutcome<Id> {
        if !self.open || !self.accepts_input {
            return JumpOutcome::Ignored;
        }
        // Prefer matching prefix subset
        let pool = self.matching(targets);
        if let Some(target) = pool.iter().find(|t| t.area.contains(position)) {
            let id = target.id.clone();
            self.close();
            JumpOutcome::Activated(id)
        } else if let Some(target) = targets.iter().find(|t| t.area.contains(position)) {
            let id = target.id.clone();
            self.close();
            JumpOutcome::Activated(id)
        } else {
            JumpOutcome::Ignored
        }
    }

    /// Mouse event adapter.
    pub fn handle_mouse<Id: Clone>(
        &mut self,
        event: MouseEvent,
        targets: &[JumpTarget<Id>],
    ) -> JumpOutcome<Id> {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => self.handle_click(event.position, targets),
            _ => JumpOutcome::Ignored,
        }
    }
}

// ── Paint ───────────────────────────────────────────────────────────────────

/// Renders letter/key badges for open jump mode.
#[derive(Debug, Clone, Copy)]
pub struct JumpOverlay<'a, Id> {
    targets: &'a [JumpTarget<Id>],
    system: &'a DesignSystem,
    colorless: bool,
    /// Prefix to highlight partial matches (from state).
    prefix: &'a str,
    dim_unmatched: bool,
}

impl<'a, Id> JumpOverlay<'a, Id> {
    /// Creates a jump overlay over borrowed targets.
    #[must_use]
    pub const fn new(targets: &'a [JumpTarget<Id>], system: &'a DesignSystem) -> Self {
        Self {
            targets,
            system,
            colorless: false,
            prefix: "",
            dim_unmatched: true,
        }
    }

    /// ASCII brackets only.
    #[must_use]
    /// Reduced-color roles.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Current prefix for dimming.
    #[must_use]
    pub const fn prefix(mut self, p: &'a str) -> Self {
        self.prefix = p;
        self
    }

    /// Dim non-matching targets.
    #[must_use]
    pub const fn dim_unmatched(mut self, on: bool) -> Self {
        self.dim_unmatched = on;
        self
    }

    /// Wire from state.
    #[must_use]
    pub fn from_state(
        targets: &'a [JumpTarget<Id>],
        system: &'a DesignSystem,
        state: &'a JumpOverlayState,
    ) -> Self {
        Self {
            targets,
            system,
            colorless: false,
            prefix: state.prefix(),
            dim_unmatched: state.dim_unmatched(),
        }
    }
}

impl<Id> Widget for &JumpOverlay<'_, Id> {
    fn render(self, _area: Rect, buffer: &mut Buffer) {
        for target in self.targets {
            if target.area.width == 0 || target.area.height == 0 {
                continue;
            }
            let matches_prefix = self.prefix.is_empty() || target.keys.starts_with(self.prefix);
            if self.dim_unmatched && !self.prefix.is_empty() && !matches_prefix {
                // Skip fully unmatched when dimming (cleaner dense UIs).
                continue;
            }
            let style = if self.colorless {
                if matches_prefix {
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD)
                } else {
                    self.system.style(Role::TextMuted)
                }
            } else if matches_prefix {
                self.system
                    .style(Role::ActionFocused)
                    .add_modifier(Modifier::BOLD)
            } else {
                self.system.style(Role::TextMuted)
            };

            let keys = &target.keys;
            let label = if !self.prefix.is_empty() && keys.starts_with(self.prefix) {
                // Show remaining suffix emphasized: [ab] with prefix a → [·b]
                let rest = &keys[self.prefix.len()..];
                if rest.is_empty() {
                    format!("[{keys}]")
                } else {
                    format!("[{}{rest}]", self.prefix)
                }
            } else {
                format!("[{keys}]")
            };
            let max = usize::from(target.area.width.max(1));
            let text = take_display_cols(&label, max);
            // Prefer painting within the region; if width tight, still mark 1 cell.
            buffer.set_stringn(target.area.x, target.area.y, &text, max, style);
        }
    }
}

impl<Id> Widget for JumpOverlay<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

/// Status strip helper: "Jump: ab (3)" for multi-key chrome.
#[must_use]
pub fn jump_status_line(state: &JumpOverlayState, target_count: usize, ascii: bool) -> String {
    if !state.is_open() {
        return String::new();
    }
    let mark = if ascii { "JUMP" } else { "Jump" };
    if state.prefix().is_empty() {
        format!("{mark}: {target_count} targets · type letter · esc")
    } else {
        format!("{mark}: [{}] · esc clear", state.prefix())
    }
}

// ── Replay / determinism helpers ────────────────────────────────────────────

/// Replay a key sequence against targets; returns final outcome and open state.
///
/// Used by tests and Studio determinism checks.
pub fn replay_jump_keys<Id: Clone>(
    state: &mut JumpOverlayState,
    targets: &[JumpTarget<Id>],
    keys: &str,
) -> JumpOutcome<Id> {
    let mut last = JumpOutcome::Ignored;
    for ch in keys.chars() {
        last = state.handle_key(
            KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
            targets,
        );
        if matches!(last, JumpOutcome::Activated(_) | JumpOutcome::Dismissed) {
            break;
        }
    }
    last
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;
    use crate::interaction::{SemanticNode, SemanticRole, SemanticScene};

    #[test]
    fn generate_labels_deterministic_and_unique() {
        let small = generate_jump_labels(3);
        assert_eq!(small, vec!["a", "b", "c"]);
        let a = generate_jump_labels(30);
        let b = generate_jump_labels(30);
        assert_eq!(a, b);
        // n>26 → uniform length-2 prefix-free set
        assert!(a.iter().all(|l| l.len() == 2), "{a:?}");
        let mut set = std::collections::HashSet::new();
        for l in &a {
            assert!(set.insert(l.clone()), "dup {l}");
        }
        // No label is a prefix of another
        for (i, x) in a.iter().enumerate() {
            for (j, y) in a.iter().enumerate() {
                if i != j {
                    assert!(!y.starts_with(x.as_str()) || x.len() == y.len());
                }
            }
        }
    }

    #[test]
    fn badge_key_activates_and_closes() {
        let mut state = JumpOverlayState::new();
        state.open();
        let targets = [JumpTarget::new("files", Rect::new(0, 0, 10, 3), "f")];
        let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE);
        assert_eq!(
            state.handle_key(key, &targets),
            JumpOutcome::Activated("files")
        );
        assert!(!state.is_open());
    }

    #[test]
    fn multi_key_prefix_then_activate() {
        let mut state = JumpOverlayState::new();
        state.open();
        // Force multi-key by many targets
        let mut targets = Vec::new();
        let labels = generate_jump_labels(28);
        for (i, keys) in labels.iter().enumerate() {
            targets.push(JumpTarget::new(
                format!("t{i}"),
                Rect::new(0, i as u16, 4, 1),
                keys.clone(),
            ));
        }
        // 26th is z, 27th is multi-key — find first len>=2
        let multi = targets.iter().find(|t| t.keys.len() >= 2).unwrap();
        let keys = multi.keys.clone();
        let id = multi.id.clone();
        let out = replay_jump_keys(&mut state, &targets, &keys);
        assert_eq!(out, JumpOutcome::Activated(id));
        assert!(!state.is_open());
    }

    #[test]
    fn esc_clears_prefix_then_dismisses() {
        let mut state = JumpOverlayState::new();
        state.open();
        let labels = generate_jump_labels(28);
        let targets: Vec<_> = labels
            .iter()
            .enumerate()
            .map(|(i, k)| JumpTarget::new(i, Rect::new(0, 0, 2, 1), k.clone()))
            .collect();
        let multi = targets.iter().find(|t| t.keys.len() >= 2).unwrap();
        let first = multi.keys.chars().next().unwrap();
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char(first), KeyModifiers::NONE),
            &targets,
        );
        assert!(!state.prefix().is_empty());
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &targets),
            JumpOutcome::Prefix { .. }
        ));
        assert!(state.prefix().is_empty());
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &targets),
            JumpOutcome::Dismissed
        ));
    }

    #[test]
    fn assign_badges_is_stable_order() {
        let regions = [
            HitRegion {
                id: "a",
                area: Rect::new(0, 0, 2, 1),
            },
            HitRegion {
                id: "b",
                area: Rect::new(3, 0, 2, 1),
            },
        ];
        let badges = assign_jump_badges(&regions);
        assert_eq!(badges[0].keys, "a");
        assert_eq!(badges[1].keys, "b");
        assert_eq!(badges[0].badge(), 'a');
    }

    #[test]
    fn jump_opens_fullscreen_layer_and_esc_restores_opener() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        let mut state = JumpOverlayState::new();
        let out = state.open_on_stack(&mut stack, bounds, Some("main.list"));
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert!(state.is_open());
        assert!(stack.top_owns_input());
        assert_eq!(stack.top().unwrap().rect, bounds);
        assert_eq!(
            stack.handle_escape(),
            OverlayOutcome::Dismissed {
                id: OverlayId::from_static(JUMP_OVERLAY_ID),
                focus: Some("main.list"),
            }
        );
        state.close();
        assert!(!state.is_open());
        assert!(stack.is_empty());
    }

    #[test]
    fn jump_targets_from_semantic_scene() {
        let mut scene = SemanticScene::<&str>::new();
        scene
            .register(SemanticNode::control("a", Rect::new(0, 0, 2, 1)).role(SemanticRole::Button))
            .unwrap();
        scene
            .register(
                SemanticNode::control("b", Rect::new(3, 0, 2, 1))
                    .role(SemanticRole::Button)
                    .disabled(true),
            )
            .unwrap();
        let targets = assign_jump_badges_from_semantics(&scene);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].id, "a");
        assert_eq!(targets[0].keys, "a");
    }

    #[test]
    fn filter_by_role_and_action() {
        let mut scene = SemanticScene::<&str, &str>::new();
        scene
            .register(
                SemanticNode::control("run", Rect::new(0, 0, 4, 1))
                    .role(SemanticRole::Button)
                    .label("Run")
                    .actions(vec!["activate"]),
            )
            .unwrap();
        scene
            .register(
                SemanticNode::control("input", Rect::new(0, 1, 8, 1))
                    .role(SemanticRole::Input)
                    .label("Query"),
            )
            .unwrap();
        let filter = JumpFilter::new().roles([SemanticRole::Button]);
        let t = assign_jump_labels_from_semantics(&scene, &filter);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].id, "run");

        let filter2 = JumpFilter::new().action_contains("act");
        let t2 = assign_jump_labels_from_semantics(&scene, &filter2);
        assert_eq!(t2.len(), 1);
        assert_eq!(t2[0].id, "run");
    }

    #[test]
    fn nested_depth_and_leaves() {
        let mut scene = SemanticScene::<&str>::new();
        scene
            .register(
                SemanticNode::content("root", Rect::new(0, 0, 20, 10))
                    .role(SemanticRole::Chrome)
                    .label("App"),
            )
            .unwrap();
        // content root not focusable — add focusable children
        scene
            .register_child(
                "root",
                SemanticNode::control("btn", Rect::new(1, 1, 4, 1))
                    .role(SemanticRole::Button)
                    .label("Go"),
            )
            .unwrap();
        let all = collect_jump_candidates(&scene, &JumpFilter::new());
        assert!(all.iter().any(|c| c.id == "btn"));
        let leaves = JumpFilter::new().only_leaves(true);
        let leaf = collect_jump_candidates(&scene, &leaves);
        assert!(leaf.iter().all(|c| c.id == "btn"));
    }

    #[test]
    fn paint_ascii_colorless_and_prefix_dim() {
        let system = DesignSystem::default();
        let targets = [
            JumpTarget::new("a", Rect::new(0, 0, 6, 1), "a"),
            JumpTarget::new("b", Rect::new(0, 1, 6, 1), "b"),
        ];
        let mut state = JumpOverlayState::new();
        state.open();
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            &targets,
        );
        // if activated already for single key — use multi
        let labels = generate_jump_labels(28);
        let targets: Vec<_> = labels
            .iter()
            .enumerate()
            .map(|(i, k)| JumpTarget::new(i, Rect::new(0, (i % 20) as u16, 8, 1), k.clone()))
            .collect();
        let mut state = JumpOverlayState::new();
        state.open();
        let multi = targets.iter().find(|t| t.keys.len() >= 2).unwrap();
        let first = multi.keys.chars().next().unwrap();
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char(first), KeyModifiers::NONE),
            &targets,
        );
        let area = Rect::new(0, 0, 40, 24);
        let mut buf = Buffer::empty(area);
        JumpOverlay::from_state(&targets, &system, &state)
            .colorless(true)
            .render(area, &mut buf);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains('[') || text.contains(first), "{text}");
    }

    #[test]
    fn replay_determinism() {
        let labels = generate_jump_labels(5);
        let targets: Vec<_> = labels
            .iter()
            .enumerate()
            .map(|(i, k)| JumpTarget::new(i, Rect::new(0, 0, 2, 1), k.clone()))
            .collect();
        let mut s1 = JumpOverlayState::new();
        s1.open();
        let mut s2 = JumpOverlayState::new();
        s2.open();
        let o1 = replay_jump_keys(&mut s1, &targets, "c");
        let o2 = replay_jump_keys(&mut s2, &targets, "c");
        assert_eq!(o1, o2);
        assert_eq!(o1, JumpOutcome::Activated(2));
    }

    #[test]
    fn fuzz_keys_no_panic() {
        let labels = generate_jump_labels(40);
        let targets: Vec<_> = labels
            .iter()
            .enumerate()
            .map(|(i, k)| JumpTarget::new(i, Rect::new(0, 0, 2, 1), k.clone()))
            .collect();
        let mut state = JumpOverlayState::new();
        state.open();
        let mut seed = 1u64;
        for _ in 0..300 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let ch = JUMP_LABEL_ALPHABET[(seed as usize) % JUMP_LABEL_ALPHABET.len()];
            let _ = state.handle_key(
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
                &targets,
            );
            if !state.is_open() {
                state.open();
            }
        }
    }

    #[test]
    fn status_line() {
        let mut s = JumpOverlayState::new();
        assert!(jump_status_line(&s, 0, true).is_empty());
        s.open();
        let line = jump_status_line(&s, 3, true);
        assert!(line.contains("JUMP") || line.contains("3"));
    }

    #[test]
    fn click_activates() {
        let mut state = JumpOverlayState::new();
        state.open();
        let targets = [JumpTarget::new("x", Rect::new(5, 5, 4, 2), "x")];
        assert_eq!(
            state.handle_click(Position::new(6, 6), &targets),
            JumpOutcome::Activated("x")
        );
    }
}

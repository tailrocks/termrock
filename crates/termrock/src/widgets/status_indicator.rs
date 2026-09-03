// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **StatusIndicator** — compact semantic status primitive.
//!
//! **Mission.** Shared vocabulary for connections, tasks, agents, rows, and
//! services. Always pairs **glyph + style** (and usually a label); color alone
//! is never sufficient.
//!
//! **Variants.** Dot-like compact, labeled, and elapsed-time. Domain enums map
//! into [`SemanticStatus`] so components do not invent private status sets.
//!
//! Research: btop, process monitors, collaboration presence, agent status.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier};

use crate::{
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState},
    style::{DesignSystem, Glyph},
    text::{display_cols, take_display_cols},
};

use super::agent::ToolStatus;
use super::identity::PresenceStatus;
use super::progress::ProgressStatus;
use super::progress_steps::ProgressStepStatus;
use super::semantic_status::SemanticStatus;
use super::toast::Severity;

// ── Domain → SemanticStatus mappings ────────────────────────────────────────

impl SemanticStatus {
    /// From toast/banner severity.
    #[must_use]
    pub const fn from_severity(s: Severity) -> Self {
        match s {
            Severity::Info => Self::Idle,
            Severity::Success => Self::Success,
            Severity::Warning => Self::Warning,
            Severity::Error => Self::Failed,
        }
    }

    /// From tool card status.
    #[must_use]
    pub const fn from_tool_status(s: ToolStatus) -> Self {
        match s {
            ToolStatus::Queued | ToolStatus::Preparing => Self::Queued,
            ToolStatus::Running | ToolStatus::Streaming | ToolStatus::Detached => Self::Running,
            ToolStatus::WaitingInput | ToolStatus::WaitingPermission | ToolStatus::Cancelled => {
                Self::Paused
            }
            ToolStatus::Success => Self::Success,
            ToolStatus::Warning => Self::Warning,
            ToolStatus::Failed => Self::Failed,
        }
    }

    /// From identity presence (`None` → [`Self::Unknown`]).
    #[must_use]
    pub const fn from_presence(s: PresenceStatus) -> Self {
        match s {
            PresenceStatus::None => Self::Unknown,
            PresenceStatus::Online => Self::Online,
            PresenceStatus::Away => Self::Idle,
            PresenceStatus::Busy => Self::Running,
            PresenceStatus::Offline => Self::Offline,
            PresenceStatus::Error => Self::Failed,
        }
    }

    /// From progress bar lifecycle.
    #[must_use]
    pub const fn from_progress_status(s: ProgressStatus) -> Self {
        match s {
            ProgressStatus::Running => Self::Running,
            ProgressStatus::Paused => Self::Paused,
            ProgressStatus::Buffering => Self::Waiting,
            ProgressStatus::Cancelled => Self::Paused,
            ProgressStatus::Complete => Self::Success,
            ProgressStatus::Failed => Self::Failed,
        }
    }

    /// From pipeline step status.
    #[must_use]
    pub const fn from_progress_step_status(s: ProgressStepStatus) -> Self {
        match s {
            ProgressStepStatus::Queued => Self::Queued,
            ProgressStepStatus::Running | ProgressStepStatus::Retrying => Self::Running,
            ProgressStepStatus::Waiting => Self::Waiting,
            ProgressStepStatus::Complete => Self::Success,
            ProgressStepStatus::Skipped => Self::Idle,
            ProgressStepStatus::Warning => Self::Warning,
            ProgressStepStatus::Failed => Self::Failed,
            ProgressStepStatus::Cancelled => Self::Paused,
        }
    }
}

// ── Variant ─────────────────────────────────────────────────────────────────

/// Presentation density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum StatusIndicatorVariant {
    /// Embedded single-cell glyph only — still has accessible label in semantics.
    /// Use only where a host row already supplies the status verb.
    Compact,
    /// Glyph + short label (default).
    #[default]
    Labeled,
    /// Glyph + label + elapsed time suffix.
    Elapsed,
}

impl StatusIndicatorVariant {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Labeled => "labeled",
            Self::Elapsed => "elapsed",
        }
    }
}

// ── State (optional elapsed) ────────────────────────────────────────────────

/// Optional runtime for elapsed display.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatusIndicatorState {
    /// Elapsed seconds (host-owned clock).
    elapsed_secs: Option<u64>,
    /// Accessible name override for compact dots.
    accessible_label: Option<String>,
}

impl StatusIndicatorState {
    /// Empty.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            elapsed_secs: None,
            accessible_label: None,
        }
    }
    /// Elapsed.
    #[must_use]
    pub const fn elapsed_secs(&self) -> Option<u64> {
        self.elapsed_secs
    }

    /// Accessible label for compact mode.
    pub fn set_accessible_label(&mut self, label: impl Into<String>) {
        self.accessible_label = Some(label.into());
    }

    /// Accessible label borrow.
    #[must_use]
    pub fn accessible_label(&self) -> Option<&str> {
        self.accessible_label.as_deref()
    }
}

fn format_elapsed(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Compact status indicator (glyph + optional label + optional elapsed).
///
/// # Examples
///
/// ```
/// use termrock::style::DesignSystem;
/// use termrock::widgets::{StatusIndicator, SemanticStatus};
///
/// let system = DesignSystem::default();
/// let ind = StatusIndicator::new(SemanticStatus::Running, &system).label("agent");
/// ```
#[derive(Debug, Clone, Copy)]
pub struct StatusIndicator<'a> {
    kind: SemanticStatus,
    system: &'a DesignSystem,
    label: Option<&'a str>,
    variant: StatusIndicatorVariant,
    colorless: bool,
    elapsed_secs: Option<u64>,
    strong: bool,
}

impl<'a> StatusIndicator<'a> {
    /// Kind + system (labeled variant with default label).
    #[must_use]
    pub const fn new(kind: SemanticStatus, system: &'a DesignSystem) -> Self {
        Self {
            kind,
            system,
            label: None,
            variant: StatusIndicatorVariant::Labeled,
            colorless: false,
            elapsed_secs: None,
            strong: false,
        }
    }

    /// Compact dot only.
    #[must_use]
    pub const fn compact(kind: SemanticStatus, system: &'a DesignSystem) -> Self {
        Self {
            kind,
            system,
            label: None,
            variant: StatusIndicatorVariant::Compact,
            colorless: false,
            elapsed_secs: None,
            strong: false,
        }
    }

    /// Override label (defaults to [`SemanticStatus::default_label`]).
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Variant.
    #[must_use]
    pub const fn variant(mut self, v: StatusIndicatorVariant) -> Self {
        self.variant = v;
        self
    }

    /// Elapsed seconds (switches presentation toward Elapsed if set).
    #[must_use]
    pub const fn elapsed_secs(mut self, secs: u64) -> Self {
        self.elapsed_secs = Some(secs);
        if matches!(self.variant, StatusIndicatorVariant::Labeled) {
            self.variant = StatusIndicatorVariant::Elapsed;
        }
        self
    }

    /// Remove hue without changing the glyph vocabulary.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Emphasize (bold).
    #[must_use]
    pub const fn strong(mut self, on: bool) -> Self {
        self.strong = on;
        self
    }

    /// Resolved kind.
    #[must_use]
    pub const fn kind(self) -> SemanticStatus {
        self.kind
    }

    /// Glyph for current capability.
    #[must_use]
    pub fn glyph(&self) -> &'static str {
        self.kind.glyph()
    }

    /// Status rail for labeled presentations.
    #[must_use]
    pub fn rail(&self) -> &'static str {
        self.system.glyphs.resolve(Glyph::RailHeavy).text
    }

    /// Label text used when not compact-only.
    #[must_use]
    pub fn resolved_label(&self) -> &'a str {
        self.label.unwrap_or_else(|| self.kind.default_label())
    }

    /// Full paint string (for tests / measurement).
    #[must_use]
    pub fn text(&self, state: Option<&StatusIndicatorState>) -> String {
        let g = self.glyph();
        let elapsed = self
            .elapsed_secs
            .or_else(|| state.and_then(|s| s.elapsed_secs()));
        match self.variant {
            StatusIndicatorVariant::Compact => g.to_string(),
            StatusIndicatorVariant::Labeled => {
                format!("{} {g} {}", self.rail(), self.resolved_label())
            }
            StatusIndicatorVariant::Elapsed => {
                let mut s = format!("{} {g} {}", self.rail(), self.resolved_label());
                if let Some(secs) = elapsed {
                    s = format!("{s} {}", format_elapsed(secs));
                }
                s
            }
        }
    }

    /// Display width in cells.
    #[must_use]
    pub fn measure_width(&self, state: Option<&StatusIndicatorState>) -> u16 {
        display_cols(&self.text(state)) as u16
    }

    /// Paint; `state` carries host-clock elapsed time and a11y text.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: Option<&StatusIndicatorState>) {
        if area.is_empty() {
            return;
        }
        let text = self.text(state);
        // The label is words, not a signal: it stays in the body tone and the
        // status color lands on the rail + glyph cells (plans/007).
        let mut style = self.system.style(crate::style::Role::Text);
        if self.strong {
            style = style.add_modifier(Modifier::BOLD);
        }
        buffer.set_stringn(
            area.x,
            area.y,
            take_display_cols(&text, usize::from(area.width)).as_ref(),
            usize::from(area.width),
            style,
        );
        let mut glyph_style = self.system.style(if self.colorless {
            crate::style::Role::TextStrong
        } else {
            self.kind.role()
        });
        if self.strong
            || matches!(
                self.kind,
                SemanticStatus::Running | SemanticStatus::Failed | SemanticStatus::Online
            )
        {
            glyph_style = glyph_style.add_modifier(Modifier::BOLD);
        }
        let glyph_column = if matches!(self.variant, StatusIndicatorVariant::Compact) {
            0
        } else {
            crate::widgets::row_chrome::paint_status_glyph(
                buffer,
                area,
                0,
                self.rail(),
                glyph_style,
            );
            u16::try_from(display_cols(self.rail()).saturating_add(1)).unwrap_or(u16::MAX)
        };
        crate::widgets::row_chrome::paint_status_glyph(
            buffer,
            area,
            glyph_column,
            self.glyph(),
            glyph_style,
        );
    }

    /// Semantic registration — always exposes text name even for compact dots.
    pub fn register_semantic<Sid, Act>(
        &self,
        scene: &mut SemanticScene<Sid, Act>,
        id: Sid,
        area: Rect,
        state: Option<&StatusIndicatorState>,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Act: Clone,
    {
        if area.is_empty() {
            return;
        }
        let a11y = state
            .and_then(|s| s.accessible_label())
            .unwrap_or_else(|| self.resolved_label());
        let desc = format!(
            "status-indicator kind={} variant={} label={a11y} glyph={}",
            self.kind.id(),
            self.variant.id(),
            self.glyph(),
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Status)
                .label("status-indicator")
                .description(desc)
                .focusable(false)
                .state(SemanticState {
                    busy: matches!(self.kind, SemanticStatus::Running | SemanticStatus::Waiting),
                    ..Default::default()
                }),
        );
    }
}

// ── Catalog helpers ─────────────────────────────────────────────────────────

/// All status kinds with default labels (for Studio / docs).
#[must_use]
pub fn example_status_catalog() -> [(SemanticStatus, &'static str); 11] {
    [
        (SemanticStatus::Online, "online"),
        (SemanticStatus::Offline, "offline"),
        (SemanticStatus::Idle, "idle"),
        (SemanticStatus::Queued, "queued"),
        (SemanticStatus::Running, "running"),
        (SemanticStatus::Waiting, "waiting"),
        (SemanticStatus::Success, "ok"),
        (SemanticStatus::Warning, "warn"),
        (SemanticStatus::Failed, "failed"),
        (SemanticStatus::Paused, "paused"),
        (SemanticStatus::Unknown, "unknown"),
    ]
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {

    use super::*;

    fn system() -> DesignSystem {
        DesignSystem::default()
    }

    #[test]
    fn all_kinds_have_glyph_label_role() {
        for k in SemanticStatus::ALL {
            assert!(!k.id().is_empty());
            assert!(!k.default_label().is_empty());
            assert!(!k.glyph_unicode().is_empty());
            assert!(!k.glyph_ascii().is_empty());
            assert_eq!(SemanticStatus::from_id(k.id()), Some(k));
        }
    }

    #[test]
    fn color_alone_insufficient_compact_has_distinct_glyphs() {
        let mut glyphs_u = std::collections::BTreeSet::new();
        let mut glyphs_a = std::collections::BTreeSet::new();
        for k in SemanticStatus::ALL {
            glyphs_u.insert(k.glyph_unicode());
            glyphs_a.insert(k.glyph_ascii());
        }
        // Not all collapsed to one glyph
        assert!(glyphs_u.len() >= 8, "{glyphs_u:?}");
        assert!(glyphs_a.len() >= 8, "{glyphs_a:?}");
    }

    #[test]
    fn mapping_severity() {
        assert_eq!(
            SemanticStatus::from_severity(Severity::Success),
            SemanticStatus::Success
        );
        assert_eq!(
            SemanticStatus::from_severity(Severity::Error),
            SemanticStatus::Failed
        );
        assert_eq!(
            SemanticStatus::from_severity(Severity::Warning),
            SemanticStatus::Warning
        );
    }

    #[test]
    fn mapping_tool_status() {
        assert_eq!(
            SemanticStatus::from_tool_status(ToolStatus::Queued),
            SemanticStatus::Queued
        );
        assert_eq!(
            SemanticStatus::from_tool_status(ToolStatus::Running),
            SemanticStatus::Running
        );
        assert_eq!(
            SemanticStatus::from_tool_status(ToolStatus::Success),
            SemanticStatus::Success
        );
        assert_eq!(
            SemanticStatus::from_tool_status(ToolStatus::Failed),
            SemanticStatus::Failed
        );
        assert_eq!(
            SemanticStatus::from_tool_status(ToolStatus::Cancelled),
            SemanticStatus::Paused
        );
        assert_eq!(
            SemanticStatus::from_tool_status(ToolStatus::WaitingPermission),
            SemanticStatus::Paused
        );
    }

    #[test]
    fn mapping_presence() {
        assert_eq!(
            SemanticStatus::from_presence(PresenceStatus::Online),
            SemanticStatus::Online
        );
        assert_eq!(
            SemanticStatus::from_presence(PresenceStatus::Offline),
            SemanticStatus::Offline
        );
        assert_eq!(
            SemanticStatus::from_presence(PresenceStatus::Away),
            SemanticStatus::Idle
        );
        assert_eq!(
            SemanticStatus::from_presence(PresenceStatus::Busy),
            SemanticStatus::Running
        );
        assert_eq!(
            SemanticStatus::from_presence(PresenceStatus::Error),
            SemanticStatus::Failed
        );
    }

    #[test]
    fn mapping_progress_status() {
        assert_eq!(
            SemanticStatus::from_progress_status(ProgressStatus::Running),
            SemanticStatus::Running
        );
        assert_eq!(
            SemanticStatus::from_progress_status(ProgressStatus::Buffering),
            SemanticStatus::Waiting
        );
        assert_eq!(
            SemanticStatus::from_progress_status(ProgressStatus::Complete),
            SemanticStatus::Success
        );
        assert_eq!(
            SemanticStatus::from_progress_status(ProgressStatus::Failed),
            SemanticStatus::Failed
        );
        assert_eq!(
            SemanticStatus::from_progress_status(ProgressStatus::Paused),
            SemanticStatus::Paused
        );
    }

    #[test]
    fn mapping_progress_step_status() {
        assert_eq!(
            SemanticStatus::from_progress_step_status(ProgressStepStatus::Queued),
            SemanticStatus::Queued
        );
        assert_eq!(
            SemanticStatus::from_progress_step_status(ProgressStepStatus::Retrying),
            SemanticStatus::Running
        );
        assert_eq!(
            SemanticStatus::from_progress_step_status(ProgressStepStatus::Warning),
            SemanticStatus::Warning
        );
        assert_eq!(
            SemanticStatus::from_progress_step_status(ProgressStepStatus::Skipped),
            SemanticStatus::Idle
        );
    }

    #[test]
    fn glyph_comes_from_the_one_vocabulary() {
        let sys = system();
        let s = StatusIndicator::new(SemanticStatus::Online, &sys);
        assert_eq!(s.glyph(), SemanticStatus::Online.glyph_unicode());
        assert_eq!(
            SemanticStatus::Failed.glyph(),
            SemanticStatus::Failed.glyph_unicode()
        );
    }

    #[test]
    fn variants_compact_labeled_elapsed() {
        let system = system();
        let c = StatusIndicator::compact(SemanticStatus::Running, &system);
        assert_eq!(c.variant, StatusIndicatorVariant::Compact);
        assert_eq!(c.text(None), c.glyph());

        let l = StatusIndicator::new(SemanticStatus::Success, &system).label("saved");
        assert!(l.text(None).contains("saved"));
        assert!(l.text(None).contains(l.glyph()));
        assert!(l.text(None).starts_with(l.rail()));

        let e = StatusIndicator::new(SemanticStatus::Running, &system)
            .label("job")
            .elapsed_secs(125);
        let t = e.text(None);
        assert!(t.contains("job"), "{t}");
        assert!(t.contains("2m") || t.contains("125"), "{t}");
    }

    #[test]
    fn paint_includes_non_color_glyph() {
        let system = system();
        let area = Rect::new(0, 0, 16, 1);
        let mut buf = Buffer::empty(area);
        StatusIndicator::new(SemanticStatus::Failed, &system)
            .label("err")
            .paint(area, &mut buf, None);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains('✗') || text.contains('x') || text.contains("err"),
            "{text}"
        );
    }

    #[test]
    fn semantic_compact_exposes_label() {
        let system = system();
        let mut scene = SemanticScene::<&str, ()>::default();
        let mut st = StatusIndicatorState::new();
        st.set_accessible_label("agent online");
        StatusIndicator::compact(SemanticStatus::Online, &system).register_semantic(
            &mut scene,
            "s",
            Rect::new(0, 0, 2, 1),
            Some(&st),
        );
        let n = scene
            .nodes()
            .iter()
            .find(|n| n.label.as_deref() == Some("status-indicator"))
            .expect("node");
        assert!(
            n.description
                .as_deref()
                .is_some_and(|d| d.contains("agent online") || d.contains("online")),
            "{:?}",
            n.description
        );
    }

    #[test]
    fn tiny_width_safe() {
        let system = system();
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 2));
        StatusIndicator::compact(SemanticStatus::Online, &system).paint(
            Rect::new(0, 0, 1, 1),
            &mut buf,
            None,
        );
        StatusIndicator::new(SemanticStatus::Unknown, &system).paint(
            Rect::new(0, 0, 0, 0),
            &mut buf,
            None,
        );
    }

    #[test]
    fn resize_cjk_combining_and_ascii_safe() {
        let system = system();
        let label = "実行 Cafe\u{301}";
        for _ in 0..2 {
            let indicator = StatusIndicator::new(SemanticStatus::Running, &system)
                .label(label)
                .variant(StatusIndicatorVariant::Labeled);
            for width in [32, 12, 1, 0] {
                let area = Rect::new(0, 0, width, 1);
                let mut buffer = Buffer::empty(area);
                indicator.paint(area, &mut buffer, None);
                if width == 32 {
                    let text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
                    assert!(text.contains('実'), "{text:?}");
                    assert!(text.contains("Cafe\u{301}"), "{text:?}");
                }
            }
        }
    }

    #[test]
    fn fuzz_kinds_variants() {
        let system = system();
        let mut seed = 3u64;
        for _ in 0..50 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let k = SemanticStatus::ALL[(seed as usize) % SemanticStatus::ALL.len()];
            let v = match seed % 3 {
                0 => StatusIndicatorVariant::Compact,
                1 => StatusIndicatorVariant::Labeled,
                _ => StatusIndicatorVariant::Elapsed,
            };
            let mut ind = StatusIndicator::new(k, &system).variant(v);
            if matches!(v, StatusIndicatorVariant::Elapsed) {
                ind = ind.elapsed_secs(seed % 10_000);
            }
            let w = (seed % 20) as u16 + 1;
            let area = Rect::new(0, 0, w, 1);
            let mut buf = Buffer::empty(area);
            ind.paint(area, &mut buf, None);
            assert!(!ind.glyph().is_empty());
        }
    }

    #[test]
    fn paint_perf_smoke() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = system();
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..150 {
            terminal
                .draw(|f| {
                    let mut y = f.area().y;
                    for k in SemanticStatus::ALL {
                        if y >= f.area().bottom() {
                            break;
                        }
                        StatusIndicator::new(k, &system).paint(
                            Rect::new(f.area().x, y, f.area().width, 1),
                            f.buffer_mut(),
                            None,
                        );
                        y = y.saturating_add(1);
                    }
                })
                .unwrap();
        }
        assert!(start.elapsed().as_millis() < 5_000);
    }

    #[test]
    fn pty_snapshot_stable() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = system();
        let paint = || {
            let mut t = Terminal::new(TestBackend::new(24, 1)).unwrap();
            t.draw(|f| {
                StatusIndicator::new(SemanticStatus::Running, &system)
                    .label("agent")
                    .elapsed_secs(42)
                    .paint(f.area(), f.buffer_mut(), None);
            })
            .unwrap();
            t.backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol().to_string())
                .collect::<String>()
        };
        assert_eq!(paint(), paint());
    }

    #[test]
    fn catalog_len() {
        assert_eq!(example_status_catalog().len(), 11);
    }
}

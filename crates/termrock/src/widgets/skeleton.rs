// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Skeleton** — low-noise structural placeholders for async content.
//!
//! **Mission.** When the **final structure is known**, paint terminal-cell
//! placeholders (text lines, rows, cards, tables, custom rects) so layout does
//! not jump when data arrives. Prefer [`LoadingView`](super::LoadingView) /
//! [`Spinner`](super::Spinner) when structure is unknown.
//!
//! **Motion.** Default is **static** (no shimmer). Optional pulse only under
//! [`MotionPolicy::Full`] and when the host enables it — reduced/off motion never
//! animates.
//!
//! Research: shadcn Skeleton, terminal loading placeholders — no gratuitous
//! web shimmer mimicry.
use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState},
    runtime::{AnimationDemand, FrameTick},
    style::{DesignSystem, MotionPolicy, Role},
};

/// Default fill glyph (Unicode block).
pub const SKELETON_FILL_UNICODE: &str = "░";
/// ASCII fill glyph.
pub const SKELETON_FILL_ASCII: &str = "#";
/// Default pulse period when shimmer is enabled (ms).
pub const SKELETON_SHIMMER_PERIOD_MS: u64 = 1_500;

// ── Shapes ──────────────────────────────────────────────────────────────────

/// One structural placeholder block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SkeletonShape {
    /// Single text-like line (width as % of available, 1–100).
    TextLine {
        /// Indent cells from left.
        indent: u16,
        /// Width fraction percent of remaining width after indent.
        width_pct: u8,
    },
    /// List/table row bar (same as TextLine; semantic alias).
    Row {
        /// Indent.
        indent: u16,
        /// Width percent.
        width_pct: u8,
    },
    /// Card: optional header bar + N body lines.
    Card {
        /// Paint a shorter header bar.
        header: bool,
        /// Body line count.
        body_lines: u16,
    },
    /// Table grid of row bars with column-like gaps.
    Table {
        /// Column count (visual chunks).
        cols: u16,
        /// Row count.
        rows: u16,
    },
    /// Explicit cell block.
    Custom {
        /// Width in cells (`0` = fill remaining).
        width: u16,
        /// Height in rows (`0` = 1).
        height: u16,
        /// Left indent.
        indent: u16,
    },
}

impl SkeletonShape {
    /// Full-width text line.
    #[must_use]
    pub const fn line() -> Self {
        Self::TextLine {
            indent: 0,
            width_pct: 75,
        }
    }

    /// Indented secondary line.
    #[must_use]
    pub const fn line_indent(indent: u16) -> Self {
        Self::TextLine {
            indent,
            width_pct: 60,
        }
    }

    /// Card with header + body lines.
    #[must_use]
    pub const fn card(body_lines: u16) -> Self {
        Self::Card {
            header: true,
            body_lines,
        }
    }

    /// Table grid.
    #[must_use]
    pub const fn table(cols: u16, rows: u16) -> Self {
        Self::Table { cols, rows }
    }

    /// Rows consumed by this shape (before gap).
    #[must_use]
    pub fn height(self) -> u16 {
        match self {
            Self::TextLine { .. } | Self::Row { .. } => 1,
            Self::Card { header, body_lines } => u16::from(header) + body_lines.max(1),
            Self::Table { rows, .. } => rows.max(1),
            Self::Custom { height, .. } => height.max(1),
        }
    }
}

// ── Layout / recipe ─────────────────────────────────────────────────────────

/// Ordered shapes that reserve vertical space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonLayout {
    shapes: Vec<SkeletonShape>,
    /// Gap rows between shapes.
    gap: u16,
    /// Hard reserve height (extra empty rows to match final content).
    reserved_height: Option<u16>,
}

impl SkeletonLayout {
    /// Empty layout.
    #[must_use]
    pub fn new() -> Self {
        Self {
            shapes: Vec::new(),
            gap: 0,
            reserved_height: None,
        }
    }

    /// Classic staggered list lines (legacy `Skeleton::new(n)`).
    #[must_use]
    pub fn lines(n: u16) -> Self {
        let mut shapes = Vec::with_capacity(usize::from(n));
        for i in 0..n {
            let indent = if i % 2 == 0 { 0 } else { 2 };
            shapes.push(SkeletonShape::TextLine {
                indent,
                width_pct: 75,
            });
        }
        Self {
            shapes,
            gap: 0,
            reserved_height: Some(n),
        }
    }

    /// Single card placeholder.
    #[must_use]
    pub fn card(body_lines: u16) -> Self {
        Self {
            shapes: vec![SkeletonShape::card(body_lines)],
            gap: 0,
            reserved_height: None,
        }
    }

    /// Table placeholder.
    #[must_use]
    pub fn table(cols: u16, rows: u16) -> Self {
        Self {
            shapes: vec![SkeletonShape::table(cols, rows)],
            gap: 0,
            reserved_height: Some(rows.max(1)),
        }
    }

    /// Custom shape list.
    #[must_use]
    pub fn shapes(shapes: impl IntoIterator<Item = SkeletonShape>) -> Self {
        Self {
            shapes: shapes.into_iter().collect(),
            gap: 0,
            reserved_height: None,
        }
    }

    /// Gap between shapes.
    #[must_use]
    pub const fn gap(mut self, g: u16) -> Self {
        self.gap = g;
        self
    }

    /// Reserve at least this many rows (layout stability).
    #[must_use]
    pub const fn reserved_height(mut self, h: u16) -> Self {
        self.reserved_height = Some(h);
        self
    }

    /// Measured height including gaps.
    #[must_use]
    pub fn measure_height(&self) -> u16 {
        if self.shapes.is_empty() {
            return self.reserved_height.unwrap_or(0);
        }
        let mut h = 0u16;
        for (i, s) in self.shapes.iter().enumerate() {
            if i > 0 {
                h = h.saturating_add(self.gap);
            }
            h = h.saturating_add(s.height());
        }
        match self.reserved_height {
            Some(r) => h.max(r),
            None => h,
        }
    }

    /// Borrow shapes.
    #[must_use]
    pub fn shapes_slice(&self) -> &[SkeletonShape] {
        &self.shapes
    }
}

impl Default for SkeletonLayout {
    fn default() -> Self {
        Self::lines(3)
    }
}

/// Named recipes for common surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SkeletonRecipe {
    /// Staggered text lines (default).
    #[default]
    Lines,
    /// Card body.
    Card,
    /// Multi-column table rows.
    Table,
    /// Dense row stack (list).
    Rows,
}

impl SkeletonRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Lines => "lines",
            Self::Card => "card",
            Self::Table => "table",
            Self::Rows => "rows",
        }
    }

    /// Build layout with size hint.
    #[must_use]
    pub fn layout(self, n: u16) -> SkeletonLayout {
        match self {
            Self::Lines => SkeletonLayout::lines(n.max(1)),
            Self::Card => SkeletonLayout::card(n.max(1)).reserved_height(n.saturating_add(1)),
            Self::Table => SkeletonLayout::table(3, n.max(1)),
            Self::Rows => {
                let mut shapes = Vec::new();
                for _ in 0..n.max(1) {
                    shapes.push(SkeletonShape::Row {
                        indent: 0,
                        width_pct: 90,
                    });
                }
                SkeletonLayout {
                    shapes,
                    gap: 0,
                    reserved_height: Some(n.max(1)),
                }
            }
        }
    }
}

// ── State ───────────────────────────────────────────────────────────────────

/// Optional shimmer control (off by default).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonState {
    /// Host allows pulse under Full motion.
    shimmer: bool,
    /// Visible / on-screen.
    visible: bool,
    /// Active loading (if false, still static paint is ok).
    active: bool,
}

impl Default for SkeletonState {
    fn default() -> Self {
        Self::new()
    }
}

impl SkeletonState {
    /// Static skeleton (no shimmer).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            shimmer: false,
            visible: true,
            active: true,
        }
    }

    /// Enable optional Full-motion pulse (still never under Reduced/Off).
    pub fn set_shimmer(&mut self, on: bool) {
        self.shimmer = on;
    }

    /// Shimmer allowed by host?
    #[must_use]
    pub const fn shimmer(&self) -> bool {
        self.shimmer
    }

    /// Visible.
    pub fn set_visible(&mut self, on: bool) {
        self.visible = on;
    }

    /// Active.
    pub fn set_active(&mut self, on: bool) {
        self.active = on;
    }

    /// Whether host should schedule redraw for pulse.
    #[must_use]
    pub fn should_tick(&self, motion: MotionPolicy) -> bool {
        self.shimmer && self.visible && self.active && matches!(motion, MotionPolicy::Full)
    }

    /// Animation demand — **idle by default**.
    #[must_use]
    pub fn animation_demand(&self, tick: FrameTick, motion: MotionPolicy) -> AnimationDemand {
        if !self.should_tick(motion) {
            return AnimationDemand::idle();
        }
        AnimationDemand {
            needs_redraw: true,
            next_deadline: Some(
                tick.now() + std::time::Duration::from_millis(SKELETON_SHIMMER_PERIOD_MS),
            ),
        }
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Structural skeleton placeholder.
///
/// # Examples
///
/// ```
/// use termrock::style::DesignSystem;
/// use termrock::widgets::Skeleton;
///
/// let system = DesignSystem::default();
/// let sk = Skeleton::new(4, &system); // legacy staggered lines
/// ```
#[derive(Debug, Clone)]
pub struct Skeleton<'a> {
    system: &'a DesignSystem,
    layout: SkeletonLayout,
    /// When set, overrides layout from recipe + count.
    recipe: Option<(SkeletonRecipe, u16)>,
}

impl<'a> Skeleton<'a> {
    /// Creates a skeleton with the requested row count (staggered lines).
    ///
    /// **Preserved** constructor used by Panel / lookbook.
    #[must_use]
    pub fn new(rows: u16, system: &'a DesignSystem) -> Self {
        Self {
            system,
            layout: SkeletonLayout::lines(rows.max(1)),
            recipe: None,
        }
    }

    /// From explicit layout.
    #[must_use]
    pub fn layout(layout: SkeletonLayout, system: &'a DesignSystem) -> Self {
        Self {
            system,
            layout,
            recipe: None,
        }
    }

    /// Named recipe + size.
    #[must_use]
    pub fn recipe(recipe: SkeletonRecipe, n: u16, system: &'a DesignSystem) -> Self {
        Self {
            system,
            layout: recipe.layout(n),
            recipe: Some((recipe, n)),
        }
    }

    /// Card placeholder.
    #[must_use]
    pub fn card(body_lines: u16, system: &'a DesignSystem) -> Self {
        Self::layout(SkeletonLayout::card(body_lines), system)
    }

    /// Table placeholder.
    #[must_use]
    pub fn table(cols: u16, rows: u16, system: &'a DesignSystem) -> Self {
        Self::layout(SkeletonLayout::table(cols, rows), system)
    }

    /// ASCII fill.
    #[must_use]
    /// Measured height for layout reservation.
    pub fn measure_height(&self) -> u16 {
        self.layout.measure_height()
    }

    /// Borrow layout.
    #[must_use]
    pub fn skeleton_layout(&self) -> &SkeletonLayout {
        &self.layout
    }

    /// Paint skeleton bars for this frame.
    ///
    /// `state.visible` gates the paint; shimmer motion is the host's concern
    /// (the bars themselves are static filler), so there is no tick/motion
    /// parameter to ignore.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &SkeletonState) {
        if area.is_empty() || !state.visible {
            return;
        }
        let fill_ch = { SKELETON_FILL_UNICODE };
        let shimmer = Shimmer {
            base: self.system.style(Role::TextDisabled),
        };

        let mut y = area.y;
        let bottom = area.bottom();
        let shapes = self.layout.shapes_slice();
        for (i, shape) in shapes.iter().enumerate() {
            if y >= bottom {
                break;
            }
            if i > 0 {
                y = y.saturating_add(self.layout.gap);
                if y >= bottom {
                    break;
                }
            }
            let remain_h = bottom.saturating_sub(y);
            if remain_h == 0 {
                break;
            }
            y = paint_shape(
                *shape,
                Rect::new(area.x, y, area.width, remain_h),
                buffer,
                fill_ch,
                &shimmer,
            );
        }
        // Reserved height: leave rest empty (structure hold)
        let _ = self.layout.reserved_height;
    }

    /// Semantic registration (not focusable; structure-known loading).
    pub fn register_semantic<Sid, Act>(
        &self,
        scene: &mut SemanticScene<Sid, Act>,
        id: Sid,
        area: Rect,
        state: &SkeletonState,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Act: Clone,
    {
        if area.is_empty() || !state.visible {
            return;
        }
        let recipe = self.recipe.map(|(r, _)| r.id()).unwrap_or("custom");
        let desc = format!(
            "skeleton recipe={recipe} h={} shimmer={} shapes={}",
            self.measure_height(),
            state.shimmer(),
            self.layout.shapes_slice().len(),
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Content)
                .label("skeleton")
                .description(desc)
                .focusable(false)
                .state(SemanticState {
                    busy: state.active,
                    ..Default::default()
                }),
        );
    }
}

fn paint_shape(
    shape: SkeletonShape,
    area: Rect,
    buffer: &mut Buffer,
    fill_ch: &str,
    shimmer: &Shimmer,
) -> u16 {
    if area.is_empty() {
        return area.y;
    }
    match shape {
        SkeletonShape::TextLine { indent, width_pct }
        | SkeletonShape::Row { indent, width_pct } => {
            if area.height > 0 {
                paint_bar(
                    area.x, area.y, area.width, indent, width_pct, buffer, fill_ch, shimmer,
                );
            }
            area.y.saturating_add(1).min(area.bottom())
        }
        SkeletonShape::Card { header, body_lines } => {
            let mut y = area.y;
            if header && y < area.bottom() {
                paint_bar(area.x, y, area.width, 0, 50, buffer, fill_ch, shimmer);
                y = y.saturating_add(1);
            }
            let n = body_lines.max(1);
            for i in 0..n {
                if y >= area.bottom() {
                    break;
                }
                let pct = if i + 1 == n { 55 } else { 90 };
                paint_bar(area.x, y, area.width, 0, pct, buffer, fill_ch, shimmer);
                y = y.saturating_add(1);
            }
            y.min(area.bottom())
        }
        SkeletonShape::Table { cols, rows } => {
            let cols = cols.max(1);
            let rows = rows.max(1);
            let mut y = area.y;
            for _ in 0..rows {
                if y >= area.bottom() {
                    break;
                }
                let gap = 1u16;
                let usable = area
                    .width
                    .saturating_sub(gap.saturating_mul(cols.saturating_sub(1)));
                let col_w = (usable / cols).max(1);
                let mut x = area.x;
                for c in 0..cols {
                    if x >= area.right() {
                        break;
                    }
                    let w = col_w.min(area.right().saturating_sub(x));
                    if w > 0 {
                        paint_run(x, y, w, buffer, fill_ch, shimmer);
                    }
                    x = x
                        .saturating_add(w)
                        .saturating_add(if c + 1 < cols { gap } else { 0 });
                }
                y = y.saturating_add(1);
            }
            y.min(area.bottom())
        }
        SkeletonShape::Custom {
            width,
            height,
            indent,
        } => {
            let h = height.max(1).min(area.height);
            let mut y = area.y;
            for _ in 0..h {
                if y >= area.bottom() {
                    break;
                }
                let max_w = area.width.saturating_sub(indent.min(area.width));
                let w = if width == 0 { max_w } else { width.min(max_w) };
                if w > 0 {
                    let x = area
                        .x
                        .saturating_add(indent.min(area.width.saturating_sub(1)));
                    paint_run(x, y, w, buffer, fill_ch, shimmer);
                }
                y = y.saturating_add(1);
            }
            y.min(area.bottom())
        }
    }
}

fn paint_bar(
    origin_x: u16,
    y: u16,
    area_w: u16,
    indent: u16,
    width_pct: u8,
    buffer: &mut Buffer,
    fill_ch: &str,
    shimmer: &Shimmer,
) {
    if area_w == 0 {
        return;
    }
    let indent = indent.min(area_w.saturating_sub(1));
    let avail = area_w.saturating_sub(indent);
    if avail == 0 {
        return;
    }
    let pct = (width_pct as u16).clamp(1, 100);
    let w = ((u32::from(avail) * u32::from(pct)) / 100).max(1) as u16;
    let w = w.min(avail);
    let x = origin_x.saturating_add(indent);
    paint_run(x, y, w, buffer, fill_ch, shimmer);
}

/// Paint `w` filled cells at `(x, y)`, sampling the sweep per column.
///
/// One write per distinct style rather than one per cell: the band only changes
/// tone every few columns, so a bar costs a handful of writes.
fn paint_run(x: u16, y: u16, w: u16, buffer: &mut Buffer, fill_ch: &str, shimmer: &Shimmer) {
    let cells = (0..w).map(|i| (fill_ch, shimmer.style_at(x.saturating_add(i))));
    let mut cursor = x;
    for (text, style) in coalesce_runs(cells) {
        let cols = u16::try_from(text.chars().count()).unwrap_or(u16::MAX);
        buffer.set_stringn(cursor, y, &text, usize::from(cols), style);
        cursor = cursor.saturating_add(cols);
    }
}

/// Merge neighbouring cells that share a style into one write.
fn coalesce_runs<'a>(
    cells: impl Iterator<Item = (&'a str, ratatui_core::style::Style)>,
) -> Vec<(String, ratatui_core::style::Style)> {
    let mut runs: Vec<(String, ratatui_core::style::Style)> = Vec::new();
    for (text, style) in cells {
        match runs.last_mut() {
            Some((run, previous)) if *previous == style => run.push_str(text),
            _ => runs.push((text.to_string(), style)),
        }
    }
    runs
}

/// The single tone every skeleton cell paints.
struct Shimmer {
    base: ratatui_core::style::Style,
}

impl Shimmer {
    fn style_at(&self, _x: u16) -> ratatui_core::style::Style {
        self.base
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn system() -> DesignSystem {
        DesignSystem::default()
    }

    #[test]
    fn legacy_new_paints_staggered_lines() {
        let system = system();
        let area = Rect::new(0, 0, 20, 4);
        let mut buf = Buffer::empty(area);
        Skeleton::new(4, &system).paint(area, &mut buf, &SkeletonState::new());
        // Row 0 starts at x=0 with fill; row 1 indented
        assert_eq!(buf[(0, 0)].symbol(), "░");
        // indent 2 on odd rows
        assert_eq!(buf[(0, 1)].symbol(), " ");
        assert_eq!(buf[(2, 1)].symbol(), "░");
    }

    #[test]
    fn measure_height_reserves_layout() {
        let layout = SkeletonLayout::lines(5).reserved_height(8);
        assert_eq!(layout.measure_height(), 8);
        let card = SkeletonLayout::card(3);
        assert!(card.measure_height() >= 4);
    }

    #[test]
    fn recipes_card_table_rows() {
        let system = system();
        for recipe in [
            SkeletonRecipe::Lines,
            SkeletonRecipe::Card,
            SkeletonRecipe::Table,
            SkeletonRecipe::Rows,
        ] {
            let sk = Skeleton::recipe(recipe, 3, &system);
            let area = Rect::new(0, 0, 30, 10);
            let mut buf = Buffer::empty(area);
            sk.paint(area, &mut buf, &SkeletonState::new());
            assert!(sk.measure_height() >= 1);
        }
    }

    #[test]
    fn shimmer_off_by_default_no_redraw() {
        let state = SkeletonState::new();
        let tick = FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO);
        assert!(!state.should_tick(MotionPolicy::Full));
        assert!(
            !state
                .animation_demand(tick, MotionPolicy::Full)
                .needs_redraw
        );
        assert!(!state.animation_demand(tick, MotionPolicy::Off).needs_redraw);
    }

    #[test]
    fn shimmer_only_full_motion() {
        let mut state = SkeletonState::new();
        state.set_shimmer(true);
        let tick = FrameTick::manual(Instant::now(), Duration::from_millis(800), Duration::ZERO);
        assert!(state.should_tick(MotionPolicy::Full));
        assert!(!state.should_tick(MotionPolicy::Off));
        assert!(
            state
                .animation_demand(tick, MotionPolicy::Full)
                .needs_redraw
        );
    }

    #[test]
    fn shimmer_implies_no_spinner_frames() {
        // A skeleton says "this shape is coming", never "work is happening at
        // this rate" — so no cell may carry a spinner frame.
        let system = system();
        let mut state = SkeletonState::new();
        state.set_shimmer(true);
        let area = Rect::new(0, 0, 24, 4);
        let mut buf = Buffer::empty(area);
        Skeleton::card(2, &system).paint(area, &mut buf, &state);
        let painted: String = buf.content().iter().map(|c| c.symbol()).collect();
        for frame in crate::style::SPINNER_BRAILLE_FRAMES.iter() {
            assert!(
                !painted.contains(frame),
                "skeleton painted spinner frame {frame:?}"
            );
        }
    }

    #[test]
    fn tiny_size_safe() {
        let system = system();
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 8));
        Skeleton::new(3, &system).paint(Rect::new(0, 0, 0, 0), &mut buf, &SkeletonState::new());
        Skeleton::new(3, &system).paint(Rect::new(0, 0, 1, 1), &mut buf, &SkeletonState::new());
        Skeleton::table(4, 4, &system).paint(
            Rect::new(0, 0, 2, 2),
            &mut buf,
            &SkeletonState::new(),
        );
        Skeleton::card(2, &system).paint(Rect::new(0, 0, 3, 2), &mut buf, &SkeletonState::new());
    }

    #[test]
    fn custom_shape() {
        let system = system();
        let layout = SkeletonLayout::shapes([SkeletonShape::Custom {
            width: 5,
            height: 2,
            indent: 1,
        }]);
        let sk = Skeleton::layout(layout, &system);
        assert_eq!(sk.measure_height(), 2);
        let area = Rect::new(0, 0, 10, 3);
        let mut buf = Buffer::empty(area);
        sk.paint(area, &mut buf, &SkeletonState::new());
        assert_eq!(buf[(1, 0)].symbol(), "░");
    }

    #[test]
    fn semantic_registers() {
        let system = system();
        let state = SkeletonState::new();
        let mut scene = SemanticScene::<&str, ()>::default();
        Skeleton::new(2, &system).register_semantic(
            &mut scene,
            "sk",
            Rect::new(0, 0, 20, 4),
            &state,
        );
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("skeleton"))
        );
    }

    #[test]
    fn fuzz_shapes() {
        let system = system();
        let mut seed = 9u64;
        for _ in 0..40 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let shapes = [
                SkeletonShape::line(),
                SkeletonShape::line_indent((seed % 4) as u16),
                SkeletonShape::card((seed % 3) as u16 + 1),
                SkeletonShape::table((seed % 4) as u16 + 1, (seed % 5) as u16 + 1),
                SkeletonShape::Custom {
                    width: (seed % 10) as u16,
                    height: (seed % 3) as u16 + 1,
                    indent: (seed % 3) as u16,
                },
            ];
            let layout = SkeletonLayout::shapes([shapes[(seed as usize) % shapes.len()]]);
            let sk = Skeleton::layout(layout, &system);
            let w = (seed % 40) as u16 + 1;
            let h = (seed % 12) as u16 + 1;
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            sk.paint(area, &mut buf, &SkeletonState::new());
        }
    }

    #[test]
    fn paint_perf_smoke() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = system();
        let sk = Skeleton::recipe(SkeletonRecipe::Table, 8, &system);
        let mut terminal = Terminal::new(TestBackend::new(40, 16)).unwrap();
        let start = Instant::now();
        for _ in 0..200 {
            terminal
                .draw(|f| {
                    sk.paint(f.area(), f.buffer_mut(), &SkeletonState::new());
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
            let mut t = Terminal::new(TestBackend::new(24, 6)).unwrap();
            t.draw(|f| {
                let st = SkeletonState::new();
                Skeleton::new(4, &system).paint(f.area(), f.buffer_mut(), &st);
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
    fn capability_tiny_stories_contract() {
        // Documented capability: one fill glyph everywhere, no ascii ladder
        let system = system();
        let area = Rect::new(0, 0, 8, 3);
        let mut buf = Buffer::empty(area);
        Skeleton::new(3, &system).paint(area, &mut buf, &SkeletonState::new());
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains('\u{2591}'), "{text}");
    }
}

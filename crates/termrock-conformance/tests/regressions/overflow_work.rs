//! Overflow fitting does work linear in the number of items. The counters read
//! real work (display-width evaluations, fitting-loop visits, sort comparisons),
//! never elapsed time, and every case goes through the public widget path.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use termrock_controls::hintbar::{HintBar, HintLayer};
use termrock_controls::keyhint::Hint;
use termrock_controls::segments::{self, Segment};
use termrock_core::focus::FocusRing;
use termrock_core::hit::HitRegistry;
use termrock_core::id::WidgetId;
use termrock_feedback::statusbar::{StatusBar, StatusItem};
use termrock_fields::chips::{Chip, ChipBar, overflow_run_cells};
use termrock_metrics::{Metric, WorkDelta, measure};
use termrock_navigation::breadcrumb::Breadcrumb;
use termrock_navigation::tabs::Tabs;
use termrock_navigation::toolbar::{ToolItem, Toolbar};
use termrock_runtime::ctx::{Interaction, RenderCtx};
use termrock_theme::{ColorLevel, Theme, Tone};

const ID: WidgetId = WidgetId::of("overflow.work");
const SIZES: [usize; 3] = [250, 500, 1000];

fn theme() -> Theme {
    Theme::for_level(ColorLevel::TrueColor)
}

fn frame<R>(w: u16, h: u16, f: impl FnOnce(&mut Buffer, &mut RenderCtx<'_>) -> R) -> R {
    let theme = theme();
    let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    f(&mut buf, &mut ctx)
}

fn ceil_log2(n: usize) -> u64 {
    u64::from(usize::BITS - n.saturating_sub(1).leading_zeros())
}

/// Doubling `n` adds exactly `widths_per_item` evaluations per added item (every
/// source string measured once, hidden ones never again), at most
/// `visits_per_item` loop visits, and a sort that stays within `n log n`. `run(n)`
/// lays out `n` items per collection and `collections` of them.
fn assert_linear(
    name: &str,
    collections: u64,
    widths_per_item: u64,
    visits_per_item: u64,
    run: impl Fn(usize) -> WorkDelta,
) {
    let runs: Vec<(usize, WorkDelta)> = SIZES.iter().map(|&n| (n, run(n))).collect();
    for pair in runs.windows(2) {
        let ((n1, w1), (n2, w2)) = (pair[0], pair[1]);
        let added = (n2 - n1) as u64 * collections;
        assert_eq!(
            w2.get(Metric::IntrinsicWidthCalls) - w1.get(Metric::IntrinsicWidthCalls),
            widths_per_item * added,
            "{name}: width evaluations for {n1} -> {n2} items ({w1:?} -> {w2:?})"
        );
        assert!(
            w2.get(Metric::FitVisits) - w1.get(Metric::FitVisits) <= visits_per_item * added,
            "{name}: loop visits for {n1} -> {n2} items ({w1:?} -> {w2:?})"
        );
    }
    for &(n, w) in &runs {
        let items = n as u64 * collections;
        assert!(
            w.get(Metric::FitCompares) <= 2 * items * ceil_log2(items as usize),
            "{name}: {} comparisons for {items} items",
            w.get(Metric::FitCompares)
        );
        assert!(
            w.get(Metric::FitVisits) <= visits_per_item * items + 4,
            "{name}: {} visits for {items} items",
            w.get(Metric::FitVisits)
        );
    }
}

#[test]
fn large_overflow_chip_prefix_work_is_linear() {
    assert_linear("overflow_run_cells", 1, 0, 2, |n| {
        let widths = vec![1usize; n];
        measure(|| overflow_run_cells(&widths, 80, 1)).1
    });
    assert_linear("ChipBar::render", 1, 1, 2, |n| {
        let mut bar = ChipBar::new(ID);
        bar.set_chips((0..n).map(|_| Chip::new("x")).collect());
        frame(80, 1, |buf, ctx| {
            let bg = ctx.theme.canvas;
            measure(|| bar.render(Rect::new(0, 0, 80, 1), buf, ctx, bg)).1
        })
    });
}

#[test]
fn large_overflow_breadcrumb_work_is_linear() {
    assert_linear("Breadcrumb::render", 1, 1, 2, |n| {
        let mut crumbs = Breadcrumb::new(ID, vec!["x".to_owned(); n]);
        frame(80, 1, |buf, ctx| {
            let bg = ctx.theme.canvas;
            measure(|| crumbs.render(Rect::new(0, 0, 80, 1), buf, ctx, bg)).1
        })
    });
}

#[test]
fn large_overflow_segments_work_is_linear() {
    assert_linear("segments::render", 2, 1, 2, |n| {
        let seg = |i: usize| Segment::new("x", Tone::Normal).priority((i % 5) as u8);
        let left: Vec<Segment> = (0..n).map(seg).collect();
        let right: Vec<Segment> = (0..n).map(seg).collect();
        frame(80, 1, |buf, ctx| {
            let bg = ctx.theme.canvas;
            measure(|| segments::render(Rect::new(0, 0, 80, 1), buf, ctx, &left, &right, bg)).1
        })
    });
}

#[test]
fn large_overflow_statusbar_work_is_linear() {
    assert_linear("StatusBar::layout", 3, 1, 2, |n| {
        let item = |i: usize| StatusItem::new("x", Tone::Normal).priority((i % 5) as u8);
        let mut bar = StatusBar::new();
        bar.left = (0..n).map(item).collect();
        bar.center = (0..n).map(item).collect();
        bar.right = (0..n).map(item).collect();
        measure(|| bar.layout(Rect::new(0, 0, 80, 1))).1
    });
}

#[test]
fn large_overflow_keyhint_work_is_linear() {
    assert_linear("HintBar::render", 1, 2, 2, |n| {
        let layer = HintLayer::new((0..n).map(|_| Hint::new("k", "act")).collect());
        let theme = theme();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 1));
        measure(|| HintBar::render(Rect::new(0, 0, 80, 1), &mut buf, &theme, &layer)).1
    });
}

#[test]
fn large_overflow_toolbar_work_is_linear() {
    assert_linear("Toolbar::render", 1, 1, 2, |n| {
        let mut bar = Toolbar::new(ID, (0..n).map(|_| ToolItem::new("x")).collect());
        frame(80, 1, |buf, ctx| {
            let bg = ctx.theme.canvas;
            measure(|| bar.render(Rect::new(0, 0, 80, 1), buf, ctx, bg)).1
        })
    });
}

#[test]
fn tabs_render_measures_each_tab_once() {
    for n in [1usize, 7, 40, 300] {
        let labels = vec!["x"; n];
        let mut tabs = Tabs::new(ID, &labels);
        let work = frame(80, 2, |buf, ctx| {
            let bg = ctx.theme.canvas;
            measure(|| tabs.render(Rect::new(0, 0, 80, 2), buf, ctx, bg)).1
        });
        let drawn = tabs.areas.iter().filter(|r| !r.is_empty()).count() as u64;
        // The width pass measures each item's label once, and adds one more
        // measure per prefix and per suffix when set. Painting adds, per drawn
        // tab, one label re-measure, one close_button.width call, and one
        // measure per prefix and per suffix. These labels have neither, so each
        // drawn tab adds two. When tabs are hidden, the grapheme painter also
        // measures each cell of the overflow marker, the glyph and the digits
        // of the hidden count, which is O(log n) and not per tab.
        let painting = 2 * drawn;
        let marker = if drawn < n as u64 {
            1 + (n as u64 - drawn).to_string().len() as u64
        } else {
            0
        };
        assert_eq!(
            work.get(Metric::IntrinsicWidthCalls) - painting - marker,
            n as u64,
            "Tabs::render of {n} tabs with {drawn} drawn ({work:?})"
        );
    }
}

#[test]
fn large_overflow_tabs_work_is_linear() {
    assert_linear("Tabs::render", 1, 1, 2, |n| {
        let labels = vec!["x"; n];
        let mut tabs = Tabs::new(ID, &labels);
        frame(80, 2, |buf, ctx| {
            let bg = ctx.theme.canvas;
            measure(|| tabs.render(Rect::new(0, 0, 80, 2), buf, ctx, bg)).1
        })
    });
}

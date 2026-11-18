//! The one-pass overflow fitters keep exactly what the quadratic ones kept.
//! `reference` holds the pre-#76 algorithms verbatim, written against the same
//! inputs, and each matrix runs both over small exhaustive inputs and fixed-seed
//! generated ones.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use termrock_controls::hintbar::{HintBar, HintLayer};
use termrock_controls::keyhint::{self, Hint, HintPriority};
use termrock_controls::segments::{self, Segment};
use termrock_core::focus::FocusRing;
use termrock_core::hit::HitRegistry;
use termrock_core::id::WidgetId;
use termrock_feedback::statusbar::{Emphasis, StatusBar, StatusItem};
use termrock_fields::chips::{Chip, ChipBar, overflow_run_cells};
use termrock_navigation::breadcrumb::Breadcrumb;
use termrock_navigation::toolbar::{ToolItem, Toolbar};
use termrock_runtime::ctx::{Interaction, RenderCtx};
use termrock_theme::{ColorLevel, HintBarStyle, Theme, Tone};

const ID: WidgetId = WidgetId::of("overflow.parity");

mod reference {
    //! The fitters as they were before #76, with the same inputs and results.

    use termrock_controls::keyhint::{self, Hint};
    use termrock_controls::segments::Segment;
    use termrock_feedback::statusbar::{Group, StatusBar, StatusItem};
    use termrock_text::cells::width;

    pub fn overflow_run(widths: &[usize], avail: usize, gap: usize) -> (usize, Option<String>) {
        let n = widths.len();
        let total = widths
            .iter()
            .fold(0usize, |a, &w| a.saturating_add(w))
            .saturating_add(gap.saturating_mul(n.saturating_sub(1)));
        if total <= avail {
            return (n, None);
        }
        for k in (0..n).rev() {
            let marker = format!("+{}", n - k);
            let used = widths[..k]
                .iter()
                .inspect(|_| termrock_metrics::record_work!(FitVisits, 1usize))
                .fold(0usize, |a, &w| a.saturating_add(w).saturating_add(gap))
                .saturating_add(width(&marker));
            if used <= avail {
                return (k, Some(marker));
            }
        }
        (0, None)
    }

    fn suffix_width(segments: &[String], from: usize, ellipsis_w: usize, sep_w: usize) -> usize {
        let lead = if from > 0 {
            ellipsis_w.saturating_add(sep_w)
        } else {
            0
        };
        let labels = segments[from..]
            .iter()
            .map(|s| width(s))
            .fold(0usize, usize::saturating_add);
        lead.saturating_add(labels)
            .saturating_add(sep_w.saturating_mul(segments.len() - from - 1))
    }

    pub fn breadcrumb_from(
        segments: &[String],
        ellipsis_w: usize,
        sep_w: usize,
        avail: usize,
    ) -> usize {
        let n = segments.len();
        let mut from = 0;
        while from + 1 < n && suffix_width(segments, from, ellipsis_w, sep_w) > avail {
            from += 1;
        }
        from
    }

    pub fn segments_keep(
        left: &[Segment],
        right: &[Segment],
        edge: u16,
        avail: u16,
    ) -> (Vec<bool>, Vec<bool>) {
        const SEP: usize = 2;
        let w = |s: &Segment| {
            width(&match &s.lead {
                Some((g, _)) => format!("{g} {}", s.text),
                None => s.text.clone(),
            })
        };
        let mut keep_l: Vec<bool> = vec![true; left.len()];
        let mut keep_r: Vec<bool> = vec![true; right.len()];
        let total = |kl: &[bool], kr: &[bool]| -> usize {
            let kept = |segs: &[Segment], keep: &[bool]| {
                segs.iter()
                    .zip(keep)
                    .filter(|(_, k)| **k)
                    .map(|(s, _)| w(s).saturating_add(SEP))
                    .fold(0usize, usize::saturating_add)
            };
            let gone = kl.iter().chain(kr).any(|k| !k);
            let more = if gone { 1 + SEP } else { 0 };
            kept(left, kl)
                .saturating_add(kept(right, kr))
                .saturating_add(more)
                .saturating_sub(SEP)
                .saturating_add(2 * usize::from(edge))
        };
        while total(&keep_l, &keep_r) > usize::from(avail) {
            let mut best: Option<(u8, bool, usize)> = None;
            for (i, s) in left.iter().enumerate() {
                if keep_l[i] && best.is_none_or(|b| s.priority < b.0) {
                    best = Some((s.priority, true, i));
                }
            }
            for (i, s) in right.iter().enumerate() {
                if keep_r[i] && best.is_none_or(|b| s.priority <= b.0) {
                    best = Some((s.priority, false, i));
                }
            }
            match best {
                Some((_, true, i)) => keep_l[i] = false,
                Some((_, false, i)) => keep_r[i] = false,
                None => break,
            }
        }
        (keep_l, keep_r)
    }

    fn group(bar: &StatusBar, g: Group) -> &[StatusItem] {
        match g {
            Group::Left => &bar.left,
            Group::Center => &bar.center,
            Group::Right => &bar.right,
        }
    }

    fn group_width(bar: &StatusBar, g: Group, keep: &[bool]) -> usize {
        let mut w = 0usize;
        let mut n = 0usize;
        for (it, k) in group(bar, g).iter().zip(keep) {
            if *k {
                w = w.saturating_add(it.intrinsic_width());
                n += 1;
            }
        }
        if n > 0 {
            w.saturating_add((n - 1).saturating_mul(3))
        } else {
            0
        }
    }

    fn needed_width(bar: &StatusBar, keep: &[Vec<bool>; 3]) -> usize {
        let ws = [
            group_width(bar, Group::Left, &keep[0]),
            group_width(bar, Group::Center, &keep[1]),
            group_width(bar, Group::Right, &keep[2]),
        ];
        let present = ws.iter().filter(|w| **w > 0).count();
        let gaps = present.saturating_sub(1) * 3;
        let gone = keep.iter().flatten().any(|k| !k);
        let more = if gone { 1 + 3 } else { 0 };
        ws.iter()
            .fold(0usize, |a, w| a.saturating_add(*w))
            .saturating_add(gaps)
            .saturating_add(more)
            .saturating_add(2)
    }

    pub fn statusbar_keep(bar: &StatusBar, avail: u16) -> [Vec<bool>; 3] {
        let mut keep: [Vec<bool>; 3] = [
            vec![true; bar.left.len()],
            vec![true; bar.center.len()],
            vec![true; bar.right.len()],
        ];
        let groups = [Group::Left, Group::Center, Group::Right];
        while needed_width(bar, &keep) > usize::from(avail) {
            let mut victim: Option<(u8, usize, usize)> = None;
            for g in [1usize, 2, 0] {
                let items = group(bar, groups[g]);
                let alive: Vec<usize> = (0..items.len()).filter(|i| keep[g][*i]).collect();
                if alive.is_empty() || (g == 0 && alive.len() == 1) {
                    continue;
                }
                let i = alive
                    .iter()
                    .copied()
                    .min_by_key(|i| (items[*i].priority, usize::MAX - *i))
                    .unwrap();
                let p = items[i].priority;
                if victim.is_none_or(|(vp, _, _)| p < vp) {
                    victim = Some((p, g, i));
                }
            }
            match victim {
                Some((_, g, i)) => keep[g][i] = false,
                None => break,
            }
        }
        keep
    }

    pub fn keyhint_fit(
        hints: &[Hint],
        room: u16,
        style: &termrock_theme::HintBarStyle,
    ) -> (Vec<usize>, Vec<bool>) {
        let room = usize::from(room);
        let all: Vec<usize> = (0..hints.len()).collect();
        let full = vec![false; hints.len()];
        if keyhint::cost(hints, &all, &full) <= room {
            return (all, full);
        }
        let pinned: Vec<bool> = hints.iter().map(|h| h.is_pinned(style)).collect();
        if style.compact {
            let compact: Vec<bool> = pinned.iter().map(|p| !p).collect();
            if keyhint::cost(hints, &all, &compact) <= room {
                return (all, compact);
            }
        }
        let mut keep = all;
        let mut order: Vec<usize> = (0..hints.len()).rev().filter(|&i| !pinned[i]).collect();
        order.extend((0..hints.len()).rev().filter(|&i| pinned[i]));
        for gone in order {
            if keyhint::cost(hints, &keep, &full) <= room {
                break;
            }
            keep.retain(|&i| i != gone);
        }
        (keep, full)
    }
}

/// Fixed-seed xorshift, so generated inputs are the same on every run.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

fn text(cells: usize) -> String {
    "x".repeat(cells)
}

#[test]
fn chip_overflow_prefix_matches_reference() {
    let mut cases = 0;
    let mut check = |widths: &[usize], gap: usize, room: usize| {
        cases += 1;
        assert_eq!(
            overflow_run_cells(widths, room, gap),
            reference::overflow_run(widths, room, gap),
            "widths {widths:?} gap {gap} room {room}"
        );
    };
    for len in 0..=4usize {
        let mut widths = vec![0usize; len];
        loop {
            for gap in 0..=3 {
                for room in 0..=40 {
                    check(&widths, gap, room);
                }
            }
            let Some(at) = widths.iter().position(|&w| w < 5) else {
                break;
            };
            widths[..at].fill(0);
            widths[at] += 1;
        }
    }
    let mut rng = Rng(0x76);
    for len in 5..=8 {
        for _ in 0..400 {
            let widths: Vec<usize> = (0..len).map(|_| rng.below(6)).collect();
            check(&widths, rng.below(4), rng.below(41));
        }
    }
    for n in [9usize, 10, 11, 99, 100, 101, 999, 1000, 1001] {
        for (width, gap) in [(1usize, 0usize), (1, 1), (0, 1)] {
            let widths = vec![width; n];
            let full = n * width + (n - 1) * gap;
            for room in (0..=40).chain(full.saturating_sub(12)..=full + 2) {
                check(&widths, gap, room);
            }
        }
    }
    assert!(cases > 100_000, "{cases} cases");
}

#[test]
fn breadcrumb_suffix_matches_reference() {
    let theme = theme();
    let mut cases = 0;
    let mut check = |segments: &[String], ellipsis_w: usize, sep_w: usize, avail: usize| {
        cases += 1;
        let crumbs = Breadcrumb::new(ID, segments.to_vec());
        assert_eq!(
            crumbs.first_shown(ellipsis_w, sep_w, avail),
            reference::breadcrumb_from(segments, ellipsis_w, sep_w, avail),
            "segments {segments:?} ellipsis {ellipsis_w} separator {sep_w} avail {avail}"
        );
    };
    let labels = ["", "x", "世", "abcde"];
    for len in 0..=4usize {
        let mut pick = vec![0usize; len];
        loop {
            let segments: Vec<String> = pick.iter().map(|&p| labels[p].to_owned()).collect();
            for ellipsis_w in 0..=2 {
                for sep_w in 0..=3 {
                    for avail in 0..=24 {
                        check(&segments, ellipsis_w, sep_w, avail);
                    }
                }
            }
            let Some(at) = pick.iter().position(|&p| p + 1 < labels.len()) else {
                break;
            };
            pick[..at].fill(0);
            pick[at] += 1;
        }
    }
    let mut rng = Rng(0x76b);
    for _ in 0..3000 {
        let len = 5 + rng.below(4);
        let segments: Vec<String> = (0..len).map(|_| text(rng.below(7))).collect();
        check(&segments, rng.below(3), rng.below(4), rng.below(50));
    }
    assert!(cases > 100_000, "{cases} cases");

    let mut crumbs = Breadcrumb::new(
        ID,
        vec!["root".into(), "usr".into(), "local".into(), "bin".into()],
    );
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    let mut buf = Buffer::empty(Rect::new(0, 0, 16, 1));
    crumbs.render(Rect::new(0, 0, 16, 1), &mut buf, &mut ctx, theme.canvas);
    assert!(
        crumbs.areas[0].is_empty(),
        "the dropped segment has no area"
    );
    assert!(ctx.hits.part_of(crumbs.segment_id(0)).is_none());
    assert!(
        !crumbs.areas[3].is_empty(),
        "the current segment always shows"
    );
    assert!(ctx.hits.part_of(crumbs.segment_id(2)).is_some());
}

fn segment_of(cells: usize, priority: u8, lead: bool) -> Segment {
    let s = Segment::new(text(cells), Tone::Normal).priority(priority);
    if lead { s.lead("*", Tone::Normal) } else { s }
}

#[test]
fn segments_priority_ties_match_reference() {
    let mut cases = 0;
    let mut check = |left: &[Segment], right: &[Segment], edge: u16, avail: u16| {
        cases += 1;
        let plan = segments::plan(left, right, edge, avail);
        let (keep_l, keep_r) = reference::segments_keep(left, right, edge, avail);
        let mut expected = keep_l;
        expected.extend(keep_r);
        assert_eq!(plan.keep, expected, "edge {edge} avail {avail}");
    };
    let options: Vec<Segment> = [1usize, 3]
        .iter()
        .flat_map(|&w| [0u8, 1].map(|p| segment_of(w, p, false)))
        .collect();
    let lists = |max: usize| -> Vec<Vec<Segment>> {
        let mut out = vec![vec![]];
        let mut frontier = vec![vec![]];
        for _ in 0..max {
            frontier = frontier
                .iter()
                .flat_map(|l: &Vec<Segment>| {
                    options.iter().map(move |o| {
                        let mut next = l.clone();
                        next.push(o.clone());
                        next
                    })
                })
                .collect();
            out.extend(frontier.clone());
        }
        out
    };
    let all = lists(3);
    for left in &all {
        for right in &all {
            for edge in 0..=1 {
                for avail in (0..=20).step_by(1) {
                    check(left, right, edge, avail);
                }
            }
        }
    }
    let mut rng = Rng(0x7a6);
    for _ in 0..4000 {
        let mut side = |max: usize| -> Vec<Segment> {
            (0..rng.below(max))
                .map(|_| segment_of(rng.below(6), rng.below(4) as u8, rng.below(2) == 0))
                .collect()
        };
        let left = side(8);
        let right = side(8);
        check(&left, &right, rng.below(3) as u16, rng.below(60) as u16);
    }
    assert!(cases > 100_000, "{cases} cases");
}

fn item_of(cells: usize, priority: u8, variant: usize) -> StatusItem {
    let mut item = StatusItem::new(text(cells), Tone::Normal).priority(priority);
    match variant {
        1 => item.busy = true,
        2 => item.emphasis = Emphasis::Chip,
        3 => item = item.meter(Some(40), termrock_feedback::progress::MeterTone::Normal),
        _ => {}
    }
    item
}

#[test]
fn statusbar_priority_and_last_left_match_reference() {
    let mut cases = 0;
    let mut check = |bar: &StatusBar, avail: u16| {
        cases += 1;
        let widths = bar.item_widths();
        let got = bar.keep_masks(&widths, usize::from(avail));
        let want = reference::statusbar_keep(bar, avail);
        assert_eq!(got, want, "avail {avail}");
    };
    let options: Vec<StatusItem> = [0usize, 2]
        .iter()
        .flat_map(|&w| [0u8, 1].map(|p| item_of(w, p, 0)))
        .collect();
    let lists = |max: usize| -> Vec<Vec<StatusItem>> {
        let mut out = vec![vec![]];
        let mut frontier = vec![vec![]];
        for _ in 0..max {
            frontier = frontier
                .iter()
                .flat_map(|l: &Vec<StatusItem>| {
                    options.iter().map(move |o| {
                        let mut next = l.clone();
                        next.push(o.clone());
                        next
                    })
                })
                .collect();
            out.extend(frontier.clone());
        }
        out
    };
    let all = lists(2);
    for left in &all {
        for center in &all {
            for right in &all {
                let bar = StatusBar {
                    left: left.clone(),
                    center: center.clone(),
                    right: right.clone(),
                    reveal: None,
                };
                for avail in 0..=24 {
                    check(&bar, avail);
                }
            }
        }
    }
    let mut rng = Rng(0x57a7);
    for _ in 0..4000 {
        let mut side = |max: usize| -> Vec<StatusItem> {
            (0..rng.below(max))
                .map(|_| item_of(rng.below(6), rng.below(4) as u8, rng.below(4)))
                .collect()
        };
        let bar = StatusBar {
            left: side(6),
            center: side(6),
            right: side(6),
            reveal: None,
        };
        check(&bar, rng.below(70) as u16);
    }
    assert!(cases > 100_000, "{cases} cases");
}

#[test]
fn keyhint_compact_and_pinned_order_match_reference() {
    let mut cases = 0;
    let mut check = |hints: &[Hint], room: u16, compact: bool| {
        cases += 1;
        let style = HintBarStyle {
            compact,
            ..HintBarStyle::DEFAULT
        };
        assert_eq!(
            keyhint::fit(hints, room, &style),
            reference::keyhint_fit(hints, room, &style),
            "room {room} compact {compact}"
        );
    };
    let options: Vec<Hint> = ["k", "Esc"]
        .iter()
        .flat_map(|&key| {
            ["a", "abcd"].iter().flat_map(move |&action| {
                [
                    HintPriority::Auto,
                    HintPriority::Normal,
                    HintPriority::Pinned,
                ]
                .map(|p| Hint::new(key, action).priority(p))
            })
        })
        .collect();
    let mut lists: Vec<Vec<Hint>> = vec![vec![]];
    let mut frontier: Vec<Vec<Hint>> = vec![vec![]];
    for _ in 0..3 {
        frontier = frontier
            .iter()
            .flat_map(|l| {
                options.iter().map(move |o| {
                    let mut next = l.clone();
                    next.push(o.clone());
                    next
                })
            })
            .collect();
        lists.extend(frontier.clone());
    }
    for hints in &lists {
        for compact in [false, true] {
            for room in 0..=34 {
                check(hints, room, compact);
            }
        }
    }
    let mut rng = Rng(0x4e7);
    for _ in 0..3000 {
        let hints: Vec<Hint> = (0..rng.below(10))
            .map(|_| options[rng.below(options.len())].clone())
            .collect();
        check(&hints, rng.below(80) as u16, rng.below(2) == 0);
    }
    assert!(cases > 40_000, "{cases} cases");
}

fn theme() -> Theme {
    Theme::for_level(ColorLevel::TrueColor)
}

#[cfg(feature = "work-metrics")]
#[test]
fn reference_fitters_do_superlinear_work() {
    use termrock_metrics::{Metric, WorkDelta, measure};
    let doubled = |small: WorkDelta, big: WorkDelta, metric: Metric| {
        big.get(metric) as f64 / small.get(metric) as f64
    };
    let at = |n: usize, run: &dyn Fn(usize)| measure(|| run(n)).1;

    let chips = |n: usize| {
        reference::overflow_run(&vec![1; n], 80, 1);
    };
    let crumbs = |n: usize| {
        let segments = vec!["x".to_owned(); n];
        reference::breadcrumb_from(&segments, 1, 3, 80);
    };
    let segs = |n: usize| {
        let side: Vec<Segment> = (0..n)
            .map(|i| segment_of(1, (i % 5) as u8, false))
            .collect();
        reference::segments_keep(&side, &side, 1, 80);
    };
    let status = |n: usize| {
        let items: Vec<StatusItem> = (0..n).map(|i| item_of(1, (i % 5) as u8, 0)).collect();
        let bar = StatusBar {
            left: items.clone(),
            center: items.clone(),
            right: items,
            reveal: None,
        };
        reference::statusbar_keep(&bar, 80);
    };
    let hints = |n: usize| {
        let hints: Vec<Hint> = (0..n).map(|_| Hint::new("k", "act")).collect();
        reference::keyhint_fit(&hints, 80, &HintBarStyle::DEFAULT);
    };
    for (name, run, metric) in [
        ("chips", &chips as &dyn Fn(usize), Metric::FitVisits),
        ("breadcrumb", &crumbs, Metric::IntrinsicWidthCalls),
        ("segments", &segs, Metric::IntrinsicWidthCalls),
        ("statusbar", &status, Metric::IntrinsicWidthCalls),
        ("keyhint", &hints, Metric::IntrinsicWidthCalls),
    ] {
        let ratio = doubled(at(250, run), at(500, run), metric);
        assert!(
            ratio > 3.5,
            "{name} reference work grew {ratio}x for 2x items"
        );
    }
}

fn cjk(cells: usize) -> String {
    "世".repeat(cells / 2)
}

fn row(buf: &Buffer) -> String {
    (0..buf.area().width)
        .map(|x| buf[(x, 0)].symbol())
        .collect::<String>()
}

fn render_row(w: u16, f: impl FnOnce(&mut Buffer, &mut RenderCtx<'_>, &Theme)) -> (String, usize) {
    let theme = theme();
    let mut buf = Buffer::empty(Rect::new(0, 0, w, 1));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    f(&mut buf, &mut ctx, &theme);
    let regions = ctx.hits.regions().len();
    (row(&buf), regions)
}

type Painter = Box<dyn Fn(&str, &mut Buffer, &mut RenderCtx<'_>, &Theme)>;

/// What a label wider than the allocation must leave in an 80-cell row.
fn assert_wide_label_row(name: &str, row: &str) {
    let ink = |c: char| row.chars().any(|r| r == c);
    match name {
        "chips" => assert!(row.starts_with("+1") && !ink('x'), "{name}: {row:?}"),
        "breadcrumb" => assert!(row.starts_with("… › ") && ink('…'), "{name}: {row:?}"),
        "segments" => assert!(ink('…') && !ink('x'), "{name}: {row:?}"),
        "statusbar" => assert!(
            row.trim_end().ends_with('…') && row.starts_with(" x"),
            "{name}: {row:?}"
        ),
        "hintbar" => assert!(ink('…') && !ink('x'), "{name}: {row:?}"),
        "toolbar" => assert!(
            row.trim_end().ends_with('»') && !ink('x'),
            "{name}: {row:?}"
        ),
        _ => unreachable!(),
    }
}

/// One label wider than any `u16`, painted at 80 cells. Its row must not depend
/// on how far past the allocation the label runs, so no width wraps small.
#[test]
fn render_one_wide_label_at_80_cols() {
    let area = Rect::new(0, 0, 80, 1);
    let widgets: Vec<(&str, Painter)> = vec![
        (
            "chips",
            Box::new(move |label, buf, ctx, t| {
                let mut bar = ChipBar::new(ID);
                bar.set_chips(vec![Chip::new(label)]);
                bar.render(area, buf, ctx, t.canvas);
            }),
        ),
        (
            "breadcrumb",
            Box::new(move |label, buf, ctx, t| {
                Breadcrumb::new(ID, vec!["home".into(), label.to_owned()])
                    .render(area, buf, ctx, t.canvas);
            }),
        ),
        (
            "segments",
            Box::new(move |label, buf, ctx, t| {
                let left = [Segment::new(label, Tone::Normal)];
                segments::render(area, buf, ctx, &left, &[], t.canvas);
            }),
        ),
        (
            "statusbar",
            Box::new(move |label, buf, ctx, _| {
                let mut bar = StatusBar::new();
                bar.left = vec![StatusItem::new(label, Tone::Normal)];
                bar.render(area, buf, ctx);
            }),
        ),
        (
            "hintbar",
            Box::new(move |label, buf, _, t| {
                let layer = HintLayer::new(vec![Hint::new("k", label.to_owned())]);
                HintBar::render(area, buf, t, &layer);
            }),
        ),
        (
            "toolbar",
            Box::new(move |label, buf, ctx, t| {
                Toolbar::new(ID, vec![ToolItem::new(label)]).render(area, buf, ctx, t.canvas);
            }),
        ),
    ];
    for (name, paint) in &widgets {
        let rows: Vec<(String, usize)> = [text(65_536), cjk(65_536), text(200_000)]
            .iter()
            .map(|label| render_row(80, |buf, ctx, t| paint(label, buf, ctx, t)))
            .collect();
        assert_eq!(
            rows[0], rows[2],
            "{name}: 65536 and 200000 cells paint alike"
        );
        assert_wide_label_row(name, &rows[0].0);
        assert_eq!(
            rows[0].1, rows[1].1,
            "{name}: hit regions follow the fit, not the script"
        );
    }
}

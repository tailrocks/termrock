use junie_tui::core::focus::{Focus, FocusRing};
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::master_detail::{DetailSize, MasterDetail, MdLayout, Mode};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

const MASTER: WidgetId = WidgetId::of("t.mdb.master");
const DETAIL: WidgetId = WidgetId::of("t.mdb.detail");

fn contains(parent: Rect, child: Rect) -> bool {
    child.is_empty()
        || (child.x >= parent.x
            && child.y >= parent.y
            && child.right() <= parent.right()
            && child.bottom() <= parent.bottom())
}

fn layout_at(md: &mut MasterDetail, area: Rect, focus: Option<WidgetId>) -> (MdLayout, FocusRing) {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(Rect::new(0, 0, area.right().max(1), area.bottom().max(1)));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let interaction = Interaction {
        focus,
        ..Default::default()
    };
    let l = {
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        md.layout(area, &mut buf, &mut ctx, Color::Reset)
    };
    (l, ring)
}

type Share = fn(MasterDetail, u16, u16, u16) -> MasterDetail;

fn shares() -> [Share; 2] {
    [
        |md, p, lo, hi| md.detail_share(p, lo, hi),
        |md, p, lo, hi| md.master_share(p, lo, hi),
    ]
}

#[test]
fn master_detail_infeasible_share_uses_collapsed_mode() {
    let parent = Rect::new(5, 3, 40, 8);
    for build in shares() {
        let mut md = build(
            MasterDetail::new(MASTER, DETAIL).collapse_below(0),
            50,
            100,
            100,
        );
        assert_eq!(md.split_rects(parent), None, "min 100 cannot fit width 40");

        let (l, _) = layout_at(&mut md, parent, None);
        assert_eq!(l.mode, Mode::Master);
        assert_eq!(l.master, parent);
        assert!(l.detail.is_empty());
        assert!(contains(parent, l.master) && contains(parent, l.detail));

        let (l, _) = layout_at(&mut md, parent, Some(DETAIL));
        assert_eq!(l.mode, Mode::Drawer);
        assert_eq!(l.detail, parent);
        assert!(l.master.is_empty());
        assert!(contains(parent, l.detail));
    }
}

#[test]
fn master_detail_share_minimum_exact_boundary() {
    let parent = Rect::new(5, 3, 40, 8);
    let md = MasterDetail::new(MASTER, DETAIL)
        .collapse_below(0)
        .detail_share(50, 38, 38);
    let (master, detail) = md.split_rects(parent).expect("min == usable fits");
    assert!(master.is_empty());
    assert_eq!(detail, Rect::new(7, 3, 38, 8));
    assert!(contains(parent, detail));

    let md = MasterDetail::new(MASTER, DETAIL)
        .collapse_below(0)
        .master_share(50, 38, 38);
    let (master, detail) = md.split_rects(parent).expect("min == usable fits");
    assert_eq!(master, Rect::new(5, 3, 38, 8));
    assert!(detail.is_empty());
    assert!(contains(parent, master));
    assert_eq!(detail.x, parent.right());

    for build in shares() {
        let md = build(
            MasterDetail::new(MASTER, DETAIL).collapse_below(0),
            50,
            39,
            39,
        );
        assert_eq!(md.split_rects(parent), None, "one column over the boundary");
    }
}

#[test]
fn master_detail_public_percent_and_gap_limits_are_safe() {
    let parents = [
        Rect::new(0, 0, 40, 8),
        Rect::new(5, 3, 40, 8),
        Rect::new(65_000, 3, 535, 8),
        Rect::new(0, 0, 1, 1),
    ];
    let pcts = [0, 50, 100, 101, u16::MAX];
    let gaps = [0, 2, u16::MAX];
    let bounds = [
        (0, 0),
        (0, 10),
        (30, 10),
        (10, 30),
        (100, 100),
        (u16::MAX, u16::MAX),
    ];
    for parent in parents {
        for build in shares() {
            for pct in pcts {
                for gap in gaps {
                    for (lo, hi) in bounds {
                        let md = build(
                            MasterDetail::new(MASTER, DETAIL).collapse_below(0).gap(gap),
                            pct,
                            lo,
                            hi,
                        );
                        if let Some((m, d)) = md.split_rects(parent) {
                            assert!(
                                contains(parent, m),
                                "{parent:?} {pct} {gap} {lo} {hi}: master {m:?}"
                            );
                            assert!(
                                contains(parent, d),
                                "{parent:?} {pct} {gap} {lo} {hi}: detail {d:?}"
                            );
                            assert!(
                                m.is_empty() || d.is_empty() || m.right() <= d.x,
                                "panes overlap: {m:?} {d:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn master_detail_grow_restores_split_without_policy_mutation() {
    let mut md = MasterDetail::new(MASTER, DETAIL)
        .collapse_below(0)
        .detail_share(50, 60, 100);
    let policy = (md.size, md.gap, md.collapse_below, md.detail, md.master);
    let mut focus = Focus::default();
    focus.focus(DETAIL);

    let narrow = Rect::new(0, 0, 40, 8);
    let (l, _) = layout_at(&mut md, narrow, focus.current());
    assert_eq!(l.mode, Mode::Drawer);
    assert_eq!(l.detail, narrow);

    let wide = Rect::new(0, 0, 160, 8);
    let (l, _) = layout_at(&mut md, wide, focus.current());
    assert_eq!(l.mode, Mode::Split);
    assert_eq!(l.detail, Rect::new(80, 0, 80, 8));
    assert_eq!(l.master, Rect::new(0, 0, 78, 8));
    assert!(contains(wide, l.master) && contains(wide, l.detail));
    assert_eq!(
        policy,
        (md.size, md.gap, md.collapse_below, md.detail, md.master)
    );
    assert_eq!(
        md.size,
        DetailSize::Share {
            pct: 50,
            min: 60,
            max: 100
        }
    );
    assert!(md.in_detail(DETAIL));
    assert_eq!(focus.current(), Some(DETAIL));
}

#[test]
fn master_detail_normal_share_pixels_are_compatible() {
    let parent = Rect::new(3, 1, 120, 10);

    let md = MasterDetail::new(MASTER, DETAIL)
        .collapse_below(0)
        .detail_share(40, 10, 80);
    let (m, d) = md.split_rects(parent).unwrap();
    assert_eq!(d, Rect::new(3 + 70 + 2, 1, 48, 10));
    assert_eq!(m, Rect::new(3, 1, 70, 10));

    let md = MasterDetail::new(MASTER, DETAIL)
        .collapse_below(0)
        .detail_share(60, 10, 80);
    let (m, d) = md.split_rects(parent).unwrap();
    assert_eq!(d.width, 72);
    assert_eq!(m.width, 46);
    assert_eq!(d.x, 3 + 46 + 2);

    let md = MasterDetail::new(MASTER, DETAIL)
        .collapse_below(0)
        .master_share(40, 10, 80);
    let (m, d) = md.split_rects(parent).unwrap();
    assert_eq!(m, Rect::new(3, 1, 48, 10));
    assert_eq!(d, Rect::new(3 + 50, 1, 70, 10));

    let md = MasterDetail::new(MASTER, DETAIL)
        .collapse_below(0)
        .master_share(60, 10, 80);
    let (m, d) = md.split_rects(parent).unwrap();
    assert_eq!(m.width, 72);
    assert_eq!(d, Rect::new(3 + 74, 1, 46, 10));

    let odd = Rect::new(0, 0, 125, 4);
    let md = MasterDetail::new(MASTER, DETAIL)
        .collapse_below(0)
        .detail_share(35, 1, 100);
    let (_, d) = md.split_rects(odd).unwrap();
    assert_eq!(d.width, 43, "Share floors 43.75");
    let md = MasterDetail::new(MASTER, DETAIL)
        .collapse_below(0)
        .master_share(35, 1, 100);
    let (m, _) = md.split_rects(odd).unwrap();
    assert_eq!(m.width, 44, "MasterShare rounds 43.75");

    let md = MasterDetail::new(MASTER, DETAIL).detail_share(38, 34, 50);
    assert_eq!(
        md.split_rects(Rect::new(0, 0, 100, 5)),
        None,
        "below collapse_below"
    );
}

#[test]
fn master_detail_desired_width_saturates_when_impossible() {
    for build in shares() {
        let md = build(
            MasterDetail::new(MASTER, DETAIL).collapse_below(0),
            50,
            u16::MAX,
            u16::MAX,
        );
        assert_eq!(md.panes_width(10, 10), u16::MAX);
        assert_eq!(md.natural_width(10, 10), u16::MAX);

        let md = build(
            MasterDetail::new(MASTER, DETAIL)
                .collapse_below(0)
                .gap(u16::MAX),
            50,
            10,
            20,
        );
        assert_eq!(md.panes_width(10, 10), u16::MAX);

        let md = build(
            MasterDetail::new(MASTER, DETAIL).collapse_below(0),
            u16::MAX,
            10,
            20,
        );
        let w = md.panes_width(10, 10);
        assert!(
            w >= 22,
            "desired width {w} must hold both panes and the gap"
        );
        let (m, d) = md
            .split_rects(Rect::new(0, 0, w, 1))
            .expect("desired width is feasible");
        assert!(m.width >= 10 && d.width >= 10, "{m:?} {d:?}");
    }

    let md = MasterDetail::new(MASTER, DETAIL)
        .collapse_below(0)
        .detail_share(40, 10, 80);
    let w = md.panes_width(30, 20);
    let (m, d) = md.split_rects(Rect::new(0, 0, w, 1)).unwrap();
    assert!(
        m.width >= 30 && d.width >= 20,
        "desired width {w} holds {m:?} {d:?}"
    );
}

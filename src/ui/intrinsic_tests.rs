//! Intrinsic text measurements stay wide until they are fitted against a finite
//! allocation. These tests go through the public widget paths, so they run
//! unchanged against the pre-fix code.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Rect, Size};
use ratatui::widgets::Padding;

use crate::core::event::Key;
use crate::core::focus::FocusRing;
use crate::core::hit::HitRegistry;
use crate::core::id::WidgetId;
use crate::theme::{ButtonKind, ColorLevel, Theme, Tone};
use crate::ui::ctx::{Interaction, RenderCtx};
use crate::ui::layout::SplitDir;
use crate::ui::measure::{Measure, Tile, tiles};
use crate::ui::scroll_view::ScrollView;
use crate::widgets::action_row::ActionRow;
use crate::widgets::brand::Lockup;
use crate::widgets::breadcrumb::Breadcrumb;
use crate::widgets::button::{Button, ButtonRow, RowOverflow, row_layout, row_lines};
use crate::widgets::chips::{Chip, ChipBar, overflow_run};
use crate::widgets::choice::{Checkbox, RadioGroup, Toggle};
use crate::widgets::detail_card::DetailCard;
use crate::widgets::empty::EmptyState;
use crate::widgets::icon_button::IconButton;
use crate::widgets::keyhint::{self, Hint};
use crate::widgets::meter_list::{MeterList, MeterRow};
use crate::widgets::notice::Notice;
use crate::widgets::page_header::{Heading, PageHeader};
use crate::widgets::progress::Meter;
use crate::widgets::props::{self, Prop};
use crate::widgets::segments::{self, Segment};
use crate::widgets::splitter::Splitter;
use crate::widgets::statusbar::{StatusBar, StatusItem};
use crate::widgets::steps::{Step, StepRail};
use crate::widgets::tabs::Tabs;
use crate::widgets::toolbar::{ToolItem, Toolbar, ToolbarAction};

const ID: WidgetId = WidgetId::of("intrinsic");
const MAX: u16 = u16::MAX;

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

fn ascii(cells: usize) -> String {
    "x".repeat(cells)
}

fn cjk(cells: usize) -> String {
    let mut s = "世".repeat(cells / 2);
    if cells % 2 == 1 {
        s.push('x');
    }
    s
}

fn labels() -> Vec<(&'static str, String)> {
    vec![
        ("ascii 65535", ascii(65_535)),
        ("ascii 65536", ascii(65_536)),
        ("cjk 65535", cjk(65_535)),
        ("cjk 65536", cjk(65_536)),
    ]
}

fn toolbar_of(count: usize) -> Toolbar {
    Toolbar::new(
        ID,
        (0..count).map(|_| ToolItem::new("x")).collect::<Vec<_>>(),
    )
}

#[test]
fn toolbar_large_intrinsic_width_keeps_more_actions() {
    let mut bar = toolbar_of(16_385);
    assert_eq!(
        bar.width(),
        MAX,
        "true width 65539 saturates, never wraps to 3"
    );
    frame(80, 1, |buf, ctx| {
        bar.render(Rect::new(0, 0, 80, 1), buf, ctx, ctx.theme.canvas);
        assert!(!bar.hidden().is_empty(), "overflow must hide the rest");
        assert!(
            ctx.hits.part_of(bar.more_id()).is_some(),
            "the More route must be registered"
        );
        let shown = 16_385 - bar.hidden().len();
        assert!(shown > 0 && shown < 80, "only a prefix fits in 80 cells");
        assert_eq!(bar.hidden().first(), Some(&shown));
    });

    let first_hidden = bar.hidden()[0];
    bar.cursor = 16_385;
    let (_, opened) = bar.on_key(&Key {
        code: KeyCode::Enter,
        mods: ratatui::crossterm::event::KeyModifiers::NONE,
    });
    assert!(
        opened.is_none() && bar.menu_open(),
        "Enter on More opens the menu"
    );
    let (_, chosen) = bar.on_key(&Key {
        code: KeyCode::Enter,
        mods: ratatui::crossterm::event::KeyModifiers::NONE,
    });
    assert_eq!(
        chosen,
        Some(ToolbarAction::Activated(first_hidden)),
        "activation through More reaches the first hidden action"
    );
}

#[test]
fn intrinsic_collection_sums_saturate_at_public_boundary() {
    let ctx_size = Size::new(80, 1);

    let bar = toolbar_of(20_000);
    assert_eq!(bar.width(), MAX);

    let crumbs = Breadcrumb::new(ID, vec!["x".to_owned(); 20_000]);
    frame(80, 1, |_, ctx| {
        assert_eq!(crumbs.measure(ctx_size, ctx).width, MAX);
    });

    let mut status = StatusBar::new();
    status.left = (0..20_000)
        .map(|_| StatusItem::new("x", Tone::Normal))
        .collect();
    frame(80, 1, |_, ctx| {
        assert_eq!(status.measure(ctx_size, ctx).width, MAX);
    });

    let mut chips = ChipBar::new(ID);
    chips.set_chips((0..10_000).map(|_| Chip::new("x")).collect());
    frame(80, 1, |_, ctx| {
        assert_eq!(chips.measure(ctx_size, ctx).width, MAX);
    });
}

#[test]
fn intrinsic_long_labels_do_not_wrap_small() {
    let t = theme();
    let wide = Size::new(80, 1);
    for (name, label) in labels() {
        let bar = Toolbar::new(ID, vec![ToolItem::new(&label)]);
        assert_eq!(bar.width(), MAX, "toolbar {name}");

        assert_eq!(
            Button::new(ID, &label, ButtonKind::Primary).width(),
            MAX,
            "button {name}"
        );
        assert_eq!(
            Checkbox::new(ID, &label, false).width(),
            MAX,
            "checkbox {name}"
        );
        assert_eq!(Toggle::new(ID, &label, false).width(), MAX, "toggle {name}");
        assert_eq!(
            RadioGroup::new(ID, "g", &[label.as_str()], 0).width(),
            MAX,
            "radio {name}"
        );
        assert_eq!(Lockup::new(label.clone()).width(), MAX, "lockup {name}");
        assert_eq!(
            IconButton::new(ID, &label).width(&t),
            MAX,
            "icon button {name}"
        );
        assert_eq!(
            StatusItem::new(label.clone(), Tone::Normal).width(),
            MAX,
            "status item {name}"
        );
        assert_eq!(
            segments::width(&[Segment::new(label.clone(), Tone::Normal)], &[], &t),
            MAX,
            "segments {name}"
        );
        assert_eq!(
            keyhint::width(&[Hint::new("k", label.clone())], None, None),
            MAX,
            "keyhint {name}"
        );
        assert_eq!(
            props::width(&[Prop::new(&label, "v")]),
            MAX,
            "props label {name}"
        );

        frame(80, 1, |_, ctx| {
            assert_eq!(
                Breadcrumb::new(ID, vec![label.clone()])
                    .measure(wide, ctx)
                    .width,
                MAX,
                "breadcrumb {name}"
            );
            let mut chips = ChipBar::new(ID);
            chips.set_chips(vec![Chip::new(&label)]);
            assert_eq!(chips.measure(wide, ctx).width, MAX, "chips {name}");
            let mut tabs = Tabs::new(ID, &[label.as_str()]);
            assert_eq!(tabs.measure(wide, ctx).width, MAX, "tabs {name}");
            tabs.allow_new = true;
            assert_eq!(tabs.min_width(wide, ctx), MAX, "tabs min {name}");
            assert_eq!(
                StepRail::new(ID, vec![Step::new(&label)])
                    .measure(wide, ctx)
                    .width,
                MAX,
                "steps {name}"
            );
            assert_eq!(
                EmptyState::new(&label).measure(wide, ctx).width,
                MAX,
                "empty {name}"
            );
            assert_eq!(
                Notice::error(label.clone()).measure(wide, ctx).width,
                MAX,
                "notice {name}"
            );
            assert_eq!(
                PageHeader::new(&label).measure(wide, ctx).width,
                MAX,
                "page header {name}"
            );
            assert_eq!(
                Heading::new(&label).measure(wide, ctx).width,
                MAX,
                "heading {name}"
            );
            assert_eq!(
                Meter::new(Some(10))
                    .value(label.clone())
                    .measure(wide, ctx)
                    .width,
                MAX,
                "meter {name}"
            );
            assert_eq!(
                MeterList::new(vec![MeterRow::new(label.clone(), Meter::new(Some(1)))])
                    .measure(wide, ctx)
                    .width,
                MAX,
                "meter list {name}"
            );
        });

        let one = [Button::new(ID, &label, ButtonKind::Primary)];
        assert_eq!(ButtonRow::new().width(&one, &t), MAX, "button row {name}");
        let refs: Vec<&Button> = one.iter().collect();
        assert_eq!(ActionRow::new().width(&refs), MAX, "action row {name}");
    }
}

#[test]
fn intrinsic_lockup_long_label_saturates() {
    let lockup = Lockup::new(ascii(65_534));
    assert_eq!(lockup.width(), MAX);
    frame(80, 1, |buf, ctx| {
        assert_eq!(lockup.render_clickable(0, 0, buf, ctx, ID), MAX);
        let regions = ctx.hits.regions();
        assert!(!regions.is_empty(), "the lockup is clickable");
        for r in regions {
            assert!(r.area.right() <= 80 && r.area.bottom() <= 1, "{:?}", r.area);
        }
    });
}

#[test]
fn intrinsic_no_false_all_fit_for_a_label_as_wide_as_the_owner() {
    let t = theme();
    let owner = MAX;
    let b = Button::new(ID, &ascii(65_534), ButtonKind::Primary);
    let row = ButtonRow::new().overflow(RowOverflow::Wrap);
    let two = [b.clone(), b];
    assert_eq!(
        row.rows(owner, &two, &t),
        2,
        "two buttons each as wide as the owner wrap onto two rows"
    );
    assert_eq!(row_lines(owner, &[MAX, MAX], 0), 2);
}

#[test]
fn intrinsic_gap_padding_and_count_boundaries() {
    let t = theme();
    let area = Rect::new(0, 0, 80, 24);
    for count in [65_535usize, 65_536] {
        let one = Button::new(ID, "x", ButtonKind::Primary);
        let buttons = vec![one; count];
        for gap in [0u16, 1, MAX] {
            let wrap = ButtonRow::new().gap(gap).overflow(RowOverflow::Wrap);
            let rows = wrap.rows(80, &buttons, &t);
            assert!(rows >= 1);
            let more = ButtonRow::new().gap(gap).overflow(RowOverflow::More);
            let lay = more.layout(area, &buttons, &t);
            for r in lay.rects.iter().flatten().chain(lay.more.iter()) {
                assert!(area.contains((r.x, r.y).into()) || r.is_empty(), "{r:?}");
                assert!(r.right() <= area.right() && r.bottom() <= area.bottom());
            }
            let lay = wrap.layout(area, &buttons, &t);
            for r in lay.rects.iter().flatten() {
                assert!(r.right() <= area.right() && r.bottom() <= area.bottom());
            }
        }
        let widths = vec![3u16; count];
        assert_eq!(row_lines(80, &widths, MAX), MAX);
        let rects = row_layout(area, &widths, MAX);
        assert_eq!(rects.len(), count);

        let rows: Vec<MeterRow> = (0..count)
            .map(|_| MeterRow::new("m", Meter::new(Some(5))))
            .collect();
        let list = MeterList::new(rows);
        assert_eq!(list.height(), MAX);
        frame(80, 24, |buf, ctx| {
            let drawn = list.render(area, buf, ctx, ctx.theme.canvas);
            assert_eq!(drawn, 24, "only the rows inside the owner are drawn");
        });

        let steps = StepRail::new(ID, (0..count).map(|_| Step::new("s")).collect());
        frame(80, 1, |_, ctx| {
            assert_eq!(steps.measure(Size::new(80, 24), ctx).height, MAX);
        });
    }

    for gap in [0u16, 1, MAX] {
        let mut bar = toolbar_of(6).gap(gap);
        frame(80, 1, |buf, ctx| {
            let owner = Rect::new(0, 0, 80, 1);
            bar.render(owner, buf, ctx, ctx.theme.canvas);
            for r in &bar.areas {
                assert!(
                    r.is_empty() || (r.right() <= owner.right() && r.bottom() <= owner.bottom())
                );
            }
        });
        let action = ActionRow::new().gap(gap).message("note", Tone::Normal);
        let mut a = Button::new(ID, "a", ButtonKind::Primary);
        let mut b = Button::new(WidgetId::of("b"), "b", ButtonKind::Secondary);
        frame(80, 1, |buf, ctx| {
            let owner = Rect::new(0, 0, 80, 1);
            let rects = action.render(owner, buf, ctx, ctx.theme.canvas, &mut [&mut a, &mut b]);
            for r in rects {
                assert!(r.right() <= owner.right());
            }
        });
    }

    for pad in [0u16, 1, MAX] {
        let icon = IconButton::new(ID, "x").pad(pad);
        assert_eq!(icon.width(&t), if pad == MAX { MAX } else { 1 + 2 * pad });
        frame(80, 1, |buf, ctx| {
            let r = icon.render(0, 0, buf, ctx, ratatui::style::Style::new(), None);
            assert!(r.right() <= 80);
        });
    }

    for gap in [0u16, 1, MAX] {
        let list = MeterList::new(vec![
            MeterRow::new("label", Meter::new(Some(5))).detail("d"),
        ])
        .label_gap(gap)
        .detail_gap(gap)
        .reserve(gap);
        let (mx, mw) = list.meter_column(area);
        assert!(
            u32::from(mx) + u32::from(mw) <= u32::from(area.right()),
            "{gap}"
        );
        frame(80, 24, |buf, ctx| {
            list.render(area, buf, ctx, ctx.theme.canvas);
        });
    }
}

#[test]
fn intrinsic_public_geometry_limits() {
    let area = Rect::new(0, 0, 80, 24);

    let mut split = Splitter::new(ID, SplitDir::Horizontal).hit_pad(MAX);
    split.area = Rect::new(40, 0, 1, 24);
    let hit = split.hit_area(area);
    assert!(hit.left() >= area.left() && hit.right() <= area.right());
    assert_eq!((hit.top(), hit.bottom()), (0, 24));
    let mut v = Splitter::new(ID, SplitDir::Vertical).hit_pad(MAX);
    v.area = Rect::new(0, 12, 80, 1);
    let hit = v.hit_area(area);
    assert!(hit.top() >= area.top() && hit.bottom() <= area.bottom());

    let view = ScrollView::new(ID)
        .scrollbar_gap(MAX)
        .padding(Padding::new(MAX, MAX, MAX, MAX));
    let fitted = view.fit(Size::new(80, 24), |a| a);
    assert_eq!(
        fitted,
        Size::new(80, 24),
        "no scrollbar room fits, so the measure is kept"
    );
    let layout = view.layout(area, Size::new(10, 10));
    assert!(layout.viewport.right() <= area.right() && layout.viewport.bottom() <= area.bottom());
    for r in layout
        .v_track
        .iter()
        .chain(&layout.h_track)
        .chain(&layout.corner)
    {
        assert!(
            r.right() <= area.right() && r.bottom() <= area.bottom(),
            "{r:?}"
        );
    }
}

#[test]
fn intrinsic_overflow_run_matches_the_widths_it_is_given() {
    assert_eq!(overflow_run(&[3, 3, 3], 80, 1), (3, None));
    assert_eq!(overflow_run(&[MAX, 3], MAX, 1), (0, Some("+2".to_owned())));
    let (shown, marker) = overflow_run(&[10, 10, 10], 25, 1);
    assert_eq!((shown, marker.as_deref()), (2, Some("+1")));
}

#[test]
fn intrinsic_normal_layout_parity() {
    let t = theme();

    let bar = Toolbar::new(
        ID,
        vec![
            ToolItem::new("Open").shortcut("Ctrl+O"),
            ToolItem::new("Wrap").toggle(true).separator(),
            ToolItem::new("Save"),
        ],
    );
    assert_eq!(
        bar.width(),
        (1 + 4 + 1 + 6 + 1) + (1 + 4 + 1 + 2) + (1 + 4 + 1) + 2 + 2
    );

    let crumbs = Breadcrumb::new(ID, vec!["Home".into(), "Docs".into(), "Guide".into()]);
    frame(80, 1, |_, ctx| {
        let sep = crate::ui::text::width(&format!(" {} ", ctx.theme.glyphs.overflow_right));
        assert_eq!(
            usize::from(crumbs.measure(Size::new(80, 1), ctx).width),
            4 + 4 + 5 + 2 * sep
        );
    });

    let btn = Button::new(ID, "Save", ButtonKind::Primary);
    assert_eq!(btn.width(), 6);
    let rects = row_layout(Rect::new(2, 3, 20, 2), &[6, 8, 4], 2);
    assert_eq!(
        rects,
        vec![
            Rect::new(2, 3, 6, 1),
            Rect::new(10, 3, 8, 1),
            Rect::new(2, 4, 4, 1)
        ]
    );
    assert_eq!(row_lines(20, &[6, 8, 4], 2), 2);

    let hints = [Hint::new("Enter", "Open"), Hint::new("Esc", "Close")];
    assert_eq!(
        keyhint::width(&hints, None, None),
        1 + (5 + 1 + 4 + 2) + (3 + 1 + 5 + 2)
    );

    frame(80, 1, |_, ctx| {
        let m = Meter::new(Some(40))
            .value("40%")
            .measure(Size::new(80, 1), ctx);
        assert_eq!(m, Size::new(3 + 3 + 6, 1));
    });

    let mut status = StatusBar::new();
    status.left = vec![StatusItem::new("ready", Tone::Normal)];
    status.right = vec![StatusItem::new("utf-8", Tone::Normal)];
    frame(80, 1, |_, ctx| {
        assert_eq!(status.measure(Size::new(80, 1), ctx).width, 5 + 5 + 3 + 2);
    });
    let placed = status.layout(Rect::new(0, 0, 40, 1));
    assert_eq!((placed[0].x, placed[0].width), (1, 5));
    assert_eq!((placed[1].x, placed[1].width), (34, 5));

    assert_eq!(
        segments::width(
            &[Segment::new("one", Tone::Normal)],
            &[Segment::new("two", Tone::Normal)],
            &t
        ),
        3 + 2 + 3 + 2 * t.spacing.chrome_edge
    );
    assert_eq!(props::width(&[Prop::new("Name", "value")]), 6 + 12);
}

#[test]
fn intrinsic_tiles_sum_minima_beyond_u16() {
    let h = |_: u16| 1;
    let wide = [
        Tile::new(40_000, &h),
        Tile::new(40_000, &h),
        Tile::new(40_000, &h),
    ];
    let row: &[Tile] = &wide;
    let laid = tiles(80, &[row], (1, 0));
    assert_eq!(laid.size.width, 40_000);
    assert_eq!(laid.rects.len(), 3);
    assert!(
        laid.rects.iter().all(|r| r.width == 40_000 && r.x == 0),
        "tiles that cannot share a row stack at the widest minimum: {:?}",
        laid.rects
    );
    assert_eq!(laid.size.height, 3);

    let many: Vec<Tile> = (0..70_000).map(|_| Tile::new(0, &h)).collect();
    let laid = tiles(80, &[many.as_slice()], (0, 0));
    assert_eq!(laid.rects.len(), 70_000);
    let total: u32 = laid.rects.iter().map(|r| u32::from(r.width)).sum();
    assert_eq!(
        total, 80,
        "the 80 free cells are shared, not lost to a wrapped count"
    );
}

#[test]
fn intrinsic_detail_card_fit_height_saturates_before_it_is_fitted() {
    let props: Vec<Prop> = (0..65_533).map(|n| Prop::new("k", n.to_string())).collect();
    let card = DetailCard::new("Many").fit(true).props(props);
    let area = Rect::new(0, 0, 60, 24);
    let layout = frame(60, 24, |buf, ctx| {
        card.render(area, buf, ctx, ctx.theme.canvas, &mut [])
    });
    assert_eq!(
        layout.card, area,
        "the card fills the owner and never exceeds it"
    );
    assert!(layout.props_h <= area.height);
}

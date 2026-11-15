mod common;

use common::*;
use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::widgets::button::{Button, row_layout, row_layout_right};
use junie_tui::widgets::choice_dialog::ChoiceDialog;
use junie_tui::widgets::completion::Completion;
use junie_tui::widgets::form_dialog::{FormDialog, FormField};
use junie_tui::widgets::icon_button::IconButton;
use junie_tui::widgets::info_dialog::InfoDialog;
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::notice::{Notice, NoticeKind};
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::picker::Picker;
use junie_tui::widgets::progress::{Meter, MeterVisual, render_spinner};
use junie_tui::widgets::props::Prop;
use junie_tui::widgets::steps::{Step, StepRail, StepState};
use junie_tui::widgets::too_small::TooSmall;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

const SCREEN: Rect = Rect::new(0, 0, 80, 20);

const ACCENT: &str = "e\u{301}";
const FAMILY: &str = "👨‍👩‍👧";
const FLAG: &str = "🇫🇷";
const SKIN: &str = "👍🏽";
const CJK: &str = "界";

fn clusters() -> [(&'static str, usize); 5] {
    [(ACCENT, 1), (FAMILY, 2), (FLAG, 2), (SKIN, 2), (CJK, 2)]
}

fn with_ctx(paint: impl FnOnce(&mut Buffer, &mut RenderCtx)) -> Buffer {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(SCREEN);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    paint(&mut buf, &mut ctx);
    ctx.flush_overlays(&mut buf);
    buf
}

fn find_all(buf: &Buffer, symbol: &str) -> Vec<(u16, u16)> {
    let mut found = Vec::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            if buf[(x, y)].symbol() == symbol {
                found.push((x, y));
            }
        }
    }
    found
}

fn find(buf: &Buffer, symbol: &str) -> (u16, u16) {
    *find_all(buf, symbol)
        .first()
        .unwrap_or_else(|| panic!("no cell with symbol {symbol:?}"))
}

fn row_text(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width)
        .map(|x| buf[(x, y)].symbol().to_owned())
        .collect()
}

fn rows_item(cluster: &str, n: usize) -> Item {
    Item::new(format!("{cluster}{n}")).detail(format!("Q{n}"))
}

fn assert_rows(buf: &Buffer, cluster: &str, cells: usize, ends: &[&str]) {
    let at = find_all(buf, cluster);
    assert_eq!(at.len(), ends.len(), "{cluster:?} painted once per row");
    let mut detail_x = None;
    for ((x, y), end) in at.into_iter().zip(ends) {
        assert_eq!(
            buf[(x + cells as u16, y)].symbol(),
            *end,
            "{cluster:?} advances {cells} cell(s): {}",
            row_text(buf, y)
        );
        let q = find_all(buf, "Q")
            .into_iter()
            .find(|(_, qy)| *qy == y)
            .unwrap_or_else(|| panic!("detail on row {y}: {}", row_text(buf, y)));
        assert!(q.0 > x + cells as u16, "detail stays right of the label");
        assert_eq!(
            *detail_x.get_or_insert(q.0),
            q.0,
            "detail column is aligned"
        );
    }
}

#[test]
fn picker_label_keeps_graphemes_and_detail_columns() {
    for (cluster, cells) in clusters() {
        let items = (1..=3)
            .map(|n| rows_item(cluster, n).disabled(n == 3))
            .collect();
        let mut picker = Picker::new(WidgetId::of("t.picker"), "Choose").width(40);
        picker.set_items(items).expect("valid items");
        let buf = with_ctx(|buf, ctx| picker.draw(SCREEN, buf, ctx));
        assert_rows(&buf, cluster, cells, &["1", "2", "3"]);
    }
}

#[test]
fn picker_matched_byte_offset_styles_the_whole_cluster() {
    let mut picker = Picker::new(WidgetId::of("t.picker.match"), "Choose").width(40);
    picker
        .set_items(vec![
            Item::new("first"),
            Item::new(format!("{ACCENT}Z")).matched(vec![1]),
        ])
        .expect("valid items");
    let buf = with_ctx(|buf, ctx| picker.draw(SCREEN, buf, ctx));
    let (x, y) = find(&buf, ACCENT);
    assert!(
        buf[(x, y)].modifier.contains(Modifier::BOLD),
        "matched cluster"
    );
    assert_eq!(buf[(x + 1, y)].symbol(), "Z");
    assert!(
        !buf[(x + 1, y)].modifier.contains(Modifier::BOLD),
        "neighbour"
    );
}

#[test]
fn picker_grapheme_label_respects_reserved_columns() {
    let mut picker = Picker::new(WidgetId::of("t.picker.cols"), "Choose").width(30);
    picker
        .set_items(vec![
            Item::new(format!("{CJK}{CJK}{CJK}{CJK}{CJK}{CJK}{CJK}{CJK}"))
                .detail("DETAIL-COLUMN")
                .tag("TAG"),
        ])
        .expect("valid items");
    let buf = with_ctx(|buf, ctx| picker.draw(SCREEN, buf, ctx));
    let (dx, dy) = find(&buf, "D");
    let before: String = (0..dx).map(|x| buf[(x, dy)].symbol().to_owned()).collect();
    let (lx, _) = find(&buf, CJK);
    let painted = before.matches(CJK).count() as u16;
    assert!(
        lx + painted * 2 + 2 <= dx,
        "label {painted} wide clusters from {lx} reaches detail at {dx}"
    );
    assert!(before.contains('…'), "truncated label ends in an ellipsis");
}

#[test]
fn completion_label_keeps_graphemes_and_detail_columns() {
    for (cluster, cells) in clusters() {
        let mut completion = Completion::new(WidgetId::of("t.completion"));
        completion.open(
            vec![
                rows_item(cluster, 1),
                rows_item(cluster, 2).matched(vec![0]),
            ],
            Rect::new(2, 2, 10, 1),
            0,
        );
        let buf = with_ctx(|buf, ctx| completion.render(SCREEN, buf, ctx));
        assert_rows(&buf, cluster, cells, &["1", "2"]);
    }
}

#[test]
fn completion_matched_byte_offset_styles_the_whole_cluster() {
    let mut completion = Completion::new(WidgetId::of("t.completion.match"));
    completion.open(
        vec![
            Item::new("first"),
            Item::new(format!("{ACCENT}Z")).matched(vec![1]),
        ],
        Rect::new(2, 2, 10, 1),
        0,
    );
    let buf = with_ctx(|buf, ctx| completion.render(SCREEN, buf, ctx));
    let (x, y) = find(&buf, ACCENT);
    assert!(buf[(x, y)].modifier.contains(Modifier::BOLD));
    assert!(!buf[(x + 1, y)].modifier.contains(Modifier::BOLD));
}

fn meter_buf(value: &str, pct: u8, width: u16) -> Buffer {
    let mut buf = with_ctx(|_, _| {});
    for cell in buf.content.iter_mut() {
        cell.set_symbol("#");
    }
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    Meter::new(Some(pct))
        .value(value)
        .visual(MeterVisual::Block)
        .render(Rect::new(2, 1, width, 1), &mut buf, &mut ctx, theme.canvas);
    buf
}

#[test]
fn meter_block_value_preserves_wide_cluster() {
    let buf = meter_buf("界A", 50, 14);
    assert_eq!(buf[(3, 1)].symbol(), CJK);
    assert_eq!(
        buf[(5, 1)].symbol(),
        "A",
        "A follows the two cells of the wide cluster"
    );

    for (cluster, cells) in clusters() {
        let buf = meter_buf(&format!("{cluster}A"), 50, 14);
        let (x, y) = find(&buf, cluster);
        assert_eq!(buf[(x + cells as u16, y)].symbol(), "A", "{cluster:?}");
    }
}

#[test]
fn meter_block_wide_cluster_on_fill_boundary_uses_leading_cell_style() {
    let buf = meter_buf("abcd界A", 50, 14);
    let (x, y) = find(&buf, CJK);
    assert_eq!(x, 2 + 5);
    assert_eq!(
        buf[(x, y)].bg,
        buf[(x + 1, y)].bg,
        "whole cluster shares one background"
    );
    assert!(buf[(x, y)].modifier.contains(Modifier::BOLD));
    assert_ne!(
        buf[(x + 1, y)].bg,
        buf[(x + 2, y)].bg,
        "fill seam follows the next cell"
    );
    assert_eq!(buf[(x + 2, y)].symbol(), "A");
}

#[test]
fn meter_block_long_value_stays_inside_the_bar() {
    let buf = meter_buf("界界界界界界界界界界", 50, 14);
    assert_eq!(
        buf[(16, 1)].symbol(),
        "#",
        "right of the meter area is untouched"
    );
    assert_eq!(
        buf[(1, 1)].symbol(),
        "#",
        "left of the meter area is untouched"
    );
    let ellipsis: Vec<_> = (2..16).filter(|x| buf[(*x, 1)].symbol() == "…").collect();
    assert_eq!(ellipsis.len(), 1);
}

const OUTER: Rect = Rect::new(0, 0, 40, 12);

#[test]
fn legacy_button_rows_stay_inside_allocation() {
    let many = vec![1000u16; 70_000];
    let sets: Vec<Vec<u16>> = vec![
        vec![],
        vec![6, 6],
        vec![1, 1, 1, 1],
        vec![40, 3],
        vec![u16::MAX, u16::MAX, 1],
        many,
    ];
    let areas = |h: u16| {
        [
            Rect::new(2, 3, 8, h),
            Rect::new(5, 5, 0, h),
            Rect::new(65_533, 65_533, u16::MAX, h + 2),
        ]
    };
    for widths in &sets {
        for height in 0..=2 {
            for area in areas(height) {
                for gap in [0, 1, u16::MAX] {
                    for (name, rects) in [
                        ("start", row_layout(area, widths, gap)),
                        ("end", row_layout_right(area, widths, gap)),
                    ] {
                        assert_eq!(rects.len(), widths.len(), "{name}: one rect per button");
                        for r in rects {
                            assert!(
                                contains(area, r),
                                "{name}: {r:?} escapes {area:?} (h={height} gap={gap})"
                            );
                        }
                    }
                }
            }
        }
    }
    let two = row_layout(Rect::new(2, 3, 8, 1), &[6, 6], 1);
    assert_eq!(two[1], Rect::ZERO, "second button does not fit the one row");
}

fn narrow_button(variant: usize) -> Button {
    let id = WidgetId::of("t.narrow");
    let mut b = match variant {
        0 => Button::primary(id, "Save"),
        1 => Button::toggle(id, "Wide", true),
        2 => Button::toggle(id, "Wide", false),
        3 => Button::primary(id, "Wait"),
        4 => Button::primary(id, "Dis"),
        _ => Button::primary(id, "界界界"),
    };
    b.busy = variant == 3;
    b.disabled = variant == 4;
    b
}

#[test]
fn button_narrow_segments_preserve_gutter_and_neighbors() {
    let id = WidgetId::of("t.narrow");
    for variant in 0..6 {
        for w in 0..=8u16 {
            for focused in [false, true] {
                let owner = Rect::new(5, 3, w, 1);
                let mut b = narrow_button(variant);
                let p = paint_in(OUTER, focused.then_some(id), |buf, ctx| {
                    b.render(owner, buf, ctx, Color::Reset);
                });
                let what = format!("variant {variant} width {w} focused {focused}");
                assert_outside_untouched(&p.buf, owner, &what);
                assert_hits_inside(&p, owner, &what);
                assert!(contains(owner, b.area), "{what}: area {:?}", b.area);
                if w == 0 {
                    assert!(p.hits.regions().is_empty() && p.ring.is_empty(), "{what}");
                }
                if w == 1 && focused && variant != 4 {
                    assert_ne!(
                        p.buf[(5, 3)].symbol(),
                        " ",
                        "{what}: the focus gutter survives in a one-cell button"
                    );
                }
            }
        }
    }
}

#[test]
fn spinner_label_and_wide_glyph_are_bounded() {
    let mut theme = Theme::for_level(ColorLevel::TrueColor);
    for glyphs in [&["⠋"][..], &["界"][..]] {
        theme.glyphs.spinner = glyphs;
        for rows in 0..=1u16 {
            for w in [0u16, 1, 2, 5] {
                let owner = Rect::new(5, 3, w, rows);
                let p = paint_with(&theme, OUTER, None, |buf, ctx| {
                    render_spinner(owner, buf, ctx, "WORKING hard", Color::Reset);
                });
                assert_outside_untouched(&p.buf, owner, &format!("{glyphs:?} {w}x{rows}"));
                if w == 1 && glyphs[0] == "界" {
                    assert_ne!(p.buf[(5, 3)].symbol(), "界", "no partial wide glyph");
                }
            }
        }
    }
}

#[test]
fn status_and_icon_helpers_respect_owner() {
    for w in [0u16, 1, 5] {
        let owner = Rect::new(5, 3, w, 6);
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let p = paint_in(OUTER, None, |buf, _| {
            TooSmall::new("A very long brand name")
                .quit_hint("q Quit the whole application now")
                .render(owner, buf, &theme);
        });
        assert_outside_untouched(&p.buf, owner, &format!("TooSmall width {w}"));
    }

    let mut theme = Theme::for_level(ColorLevel::TrueColor);
    theme.glyphs.info = "界";
    let owner = Rect::new(5, 3, 1, 3);
    let p = paint_with(&theme, OUTER, None, |buf, ctx| {
        Notice::new(NoticeKind::Info, "message").render(owner, buf, ctx, Color::Reset);
    });
    assert_outside_untouched(&p.buf, owner, "Notice custom glyph in width 1");

    let id = WidgetId::of("t.icon");
    let plate = Rect::new(5, 3, 3, 1);
    for glyph_x in 2..=10u16 {
        let mut icon = IconButton::close(id);
        icon.glyph = Some("界".to_owned());
        let p = paint_in(OUTER, None, |buf, ctx| {
            icon.render_in(plate, glyph_x, buf, ctx, Style::new(), None);
        });
        let what = format!("icon glyph_x {glyph_x}");
        assert_outside_untouched(&p.buf, plate, &what);
        assert_hits_inside(&p, plate, &what);
        assert_eq!(
            p.hits.regions().len(),
            1,
            "{what}: the visible plate stays clickable"
        );
    }
}

fn rail(count: usize, state: StepState, numbered: bool) -> StepRail {
    let steps = (0..count)
        .map(|i| {
            let mut s = Step::new(&format!("Deploy {i}"));
            s.state = state;
            s
        })
        .collect();
    let mut rail = StepRail::new(WidgetId::of("t.rail"), steps);
    rail.numbered = numbered;
    rail
}

#[test]
fn step_rail_narrow_numbers_and_states_are_bounded() {
    for state in [
        StepState::Queued,
        StepState::Running,
        StepState::Done,
        StepState::Failed,
    ] {
        for numbered in [false, true] {
            for w in 0..=8u16 {
                let owner = Rect::new(5, 3, w, 2);
                let mut r = rail(3, state, numbered);
                let p = paint_in(OUTER, None, |buf, ctx| {
                    r.render(owner, buf, ctx, Color::Reset);
                });
                let what = format!("{state:?} numbered {numbered} width {w}");
                assert_outside_untouched(&p.buf, owner, &what);
                assert_hits_inside(&p, owner, &what);
            }
        }
    }

    let owner = Rect::new(2, 3, 30, 2);
    let mut r = rail(1002, StepState::Queued, true);
    r.core.scroll.scroll_to(98);
    let p = paint_in(OUTER, None, |buf, ctx| {
        r.render(owner, buf, ctx, Color::Reset)
    });
    let top = (0..owner.width)
        .map(|dx| p.buf[(owner.x + dx, 3)].symbol().to_owned())
        .collect::<String>();
    assert!(
        top.contains("99 Deploy 98"),
        "a two digit number keeps its three cell advance: {top:?}"
    );
    let next = (0..owner.width)
        .map(|dx| p.buf[(owner.x + dx, 4)].symbol().to_owned())
        .collect::<String>();
    assert!(
        next.contains("100 Deploy 99"),
        "a three digit number reserves its real width: {next:?}"
    );
}

fn long_labels() -> Vec<Button> {
    (0..4)
        .map(|i| {
            Button::secondary(
                WidgetId::of("t.foot").sub(&format!("b{i}")),
                &format!("Action number {i} label"),
            )
        })
        .collect()
}

fn clicked(p: &Painted, area: Rect, id: WidgetId, what: &str) {
    let hit = p
        .hits
        .resolve((area.x + area.width / 2, area.y).into())
        .unwrap_or_else(|| panic!("{what}: nothing hit at {area:?}"));
    assert_eq!(hit.owner, id, "{what}: the footer button is reachable");
}

#[test]
fn dialog_wrapped_footer_reserves_body_space() {
    let screen = Rect::new(0, 0, 60, 30);
    let id = WidgetId::of("t.choice");
    let mut choice = ChoiceDialog::new(id, "Pick", "Which", &["One", "Two"], 0)
        .buttons(long_labels(), 0)
        .width(44);
    let p = paint_in(screen, None, |buf, ctx| choice.render(screen, buf, ctx));
    let inner = choice.area.inner(ratatui::layout::Margin::new(3, 1));
    let rows: std::collections::BTreeSet<u16> = choice.buttons.iter().map(|b| b.area.y).collect();
    assert!(
        rows.len() >= 2,
        "the footer wraps to several rows: {rows:?}"
    );
    for b in &choice.buttons {
        assert!(!b.area.is_empty(), "every action is visible");
        assert!(contains(inner, b.area), "{:?} leaves {inner:?}", b.area);
        clicked(&p, b.area, b.id, "choice");
        for r in &choice.radio.areas {
            assert!(r.intersection(b.area).is_empty(), "footer over the body");
        }
    }

    let mut info = InfoDialog::new(
        WidgetId::of("t.info2"),
        "Info",
        vec![Prop::new("name", "value")],
    )
    .width(44);
    for b in long_labels() {
        info = info.action(b);
    }
    let p = paint_in(screen, None, |buf, ctx| info.render(screen, buf, ctx));
    let inner = info.area.inner(ratatui::layout::Margin::new(3, 1));
    let rows: std::collections::BTreeSet<u16> = info.actions.iter().map(|b| b.area.y).collect();
    assert!(rows.len() >= 2, "info footer wraps: {rows:?}");
    for b in info.actions.iter().chain([&info.close]) {
        assert!(!b.area.is_empty(), "every info action is visible");
        assert!(contains(inner, b.area), "{:?} leaves {inner:?}", b.area);
        clicked(&p, b.area, b.id, "info");
    }

    let fid = WidgetId::of("t.form");
    let mut form = FormDialog::new(
        fid,
        "Form",
        vec![FormField::input(
            "name",
            TextInput::new(fid.sub("name"), "Name"),
        )],
    )
    .width(44);
    for (i, b) in long_labels().into_iter().enumerate() {
        form = form.action(&format!("a{i}"), b);
    }
    let p = paint_in(screen, None, |buf, ctx| form.render(screen, buf, ctx));
    let inner = form.area.inner(ratatui::layout::Margin::new(3, 1));
    let all: Vec<&Button> = form
        .actions
        .iter()
        .map(|(_, b)| b)
        .chain([&form.cancel, &form.save])
        .collect();
    let rows: std::collections::BTreeSet<u16> = all.iter().map(|b| b.area.y).collect();
    assert!(rows.len() >= 2, "form footer wraps: {rows:?}");
    for b in all {
        assert!(!b.area.is_empty(), "every form action is visible");
        assert!(contains(inner, b.area), "{:?} leaves {inner:?}", b.area);
        clicked(&p, b.area, b.id, "form");
    }
}

#[test]
fn dialog_impossible_footer_has_no_invisible_focus_targets() {
    for height in [5u16, 6, 7, 9] {
        let screen = Rect::new(0, 0, 60, height);
        let mut info = InfoDialog::new(
            WidgetId::of("t.info3"),
            "Info",
            vec![Prop::new("name", "value")],
        )
        .width(44);
        for b in long_labels() {
            info = info.action(b);
        }
        let p = paint_in(screen, None, |buf, ctx| info.render(screen, buf, ctx));
        let inner = info.area.inner(ratatui::layout::Margin::new(3, 1));
        for b in info.actions.iter().chain([&info.close]) {
            assert!(
                contains(inner, b.area),
                "h={height}: {:?} leaves {inner:?}",
                b.area
            );
            let registered = p.hits.regions().iter().any(|r| r.id == b.id);
            let focusable = p.ring.contains(b.id);
            assert_eq!(registered, !b.area.is_empty(), "h={height}: hit vs area");
            assert_eq!(focusable, !b.area.is_empty(), "h={height}: focus vs area");
        }
        if height == 5 {
            assert!(
                info.actions.iter().any(|b| b.area.is_empty()),
                "the smallest surface really cannot fit every row"
            );
        }
    }
    let screen = Rect::new(0, 0, 60, 30);
    let mut info = InfoDialog::new(WidgetId::of("t.info3"), "Info", vec![]).width(44);
    for b in long_labels() {
        info = info.action(b);
    }
    paint_in(screen, None, |buf, ctx| info.render(screen, buf, ctx));
    assert!(
        info.actions.iter().all(|b| !b.area.is_empty()),
        "growing the screen restores every action"
    );
}

#[test]
fn control_geometry_near_coordinate_max() {
    let screen = Rect::new(65_533, 65_533, 2, 2);
    let big = Rect::new(65_533, 65_533, u16::MAX, u16::MAX);
    let widths = [u16::MAX, 3, 9];
    for r in row_layout(big, &widths, 1)
        .into_iter()
        .chain(row_layout_right(big, &widths, 1))
    {
        assert!(contains(screen, r), "{r:?}");
    }
    let id = WidgetId::of("t.max");
    let mut b = Button::primary(id, "Save");
    let p = paint_in(screen, Some(id), |buf, ctx| {
        b.render(big, buf, ctx, Color::Reset)
    });
    assert_outside_untouched(&p.buf, screen, "button near max");
    let mut icon = IconButton::close(id);
    icon.pad = 3;
    let p = paint_in(screen, None, |buf, ctx| {
        icon.render(65_534, 65_534, buf, ctx, Style::new(), None);
    });
    assert_outside_untouched(&p.buf, screen, "icon near max");
}

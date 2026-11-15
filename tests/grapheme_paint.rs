use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::widgets::completion::Completion;
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::picker::Picker;
use junie_tui::widgets::progress::{Meter, MeterVisual};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

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

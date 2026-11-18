use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use termrock_core::focus::FocusRing;
use termrock_core::hit::HitRegistry;
use termrock_core::id::WidgetId;
use termrock_overlays::option_list::{Item, OptionList, PopupRow, PopupSnapshot};
use termrock_overlays::picker::Picker;
use termrock_runtime::ctx::{Interaction, RenderCtx};
use termrock_theme::{ColorLevel, Theme};

fn painted_item_rows(count: usize, max_rows: u16, searchable: bool, hints: &str) -> usize {
    let mut picker = Picker::new(WidgetId::of("picker-rows"), "Rows")
        .max_rows(max_rows)
        .hints(hints)
        .searchable(searchable)
        .expect("toggle search");
    picker
        .set_items(
            (0..count)
                .map(|index| Item::new(format!("Item {index}")))
                .collect(),
        )
        .expect("valid items");
    let screen = Rect::new(0, 0, 80, 40);
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(screen);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    picker.draw(screen, &mut buf, &mut ctx);
    ctx.flush_overlays(&mut buf);
    (0..screen.height)
        .filter(|&y| {
            let line: String = (0..screen.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect();
            line.contains("Item ")
        })
        .count()
}

#[test]
fn painted_item_rows_never_exceed_max_rows() {
    let mut failures = Vec::new();
    for searchable in [false, true] {
        for hints in ["", "Enter choose"] {
            for max_rows in [1u16, 2, 3, 8] {
                for count in [1usize, 2, 3, 7, 8, 9, 20] {
                    let expected = count.min(usize::from(max_rows));
                    let painted = painted_item_rows(count, max_rows, searchable, hints);
                    if painted != expected {
                        failures.push(format!(
                            "searchable={searchable} hints={hints:?} max_rows={max_rows} \
                             count={count}: painted {painted}, expected {expected}"
                        ));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn an_option_list_snapshot_carries_the_visible_rows_for_a_deferred_paint() {
    let items: Vec<Item> = (0..20).map(|i| Item::new(format!("Item {i}"))).collect();
    let mut list = OptionList::new(WidgetId::of("snap"));
    let inner = Rect::new(2, 1, 30, 5);
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 8));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);

    list.layout(inner, &items[..], &ctx);
    let snapshot: PopupSnapshot<String> =
        list.snapshot(inner, &items[..], |i| format!("payload {i}"));
    let rows: &[PopupRow<String>] = snapshot.rows();
    assert_eq!(rows.len(), 5, "only the rows inside the popup are copied");
    assert_eq!((rows[0].index, rows[0].line), (0, 0));
    assert_eq!(rows[0].payload, "payload 0");
    assert_eq!((rows[4].index, rows[4].line), (4, 4));
    assert!(rows.iter().all(|row| row.enabled && !row.checked));
    assert!(!snapshot.more_above() && snapshot.more_below());
    assert_eq!(
        snapshot.row_id(3),
        list.row_id(3),
        "the snapshot names rows as the list does"
    );

    let mut painted = Vec::new();
    snapshot.paint_rows(
        inner,
        theme.canvas,
        &mut buf,
        &mut ctx,
        |row, _paint, buf, _ctx| {
            painted.push(row.index);
            buf.set_string(
                inner.x,
                inner.y + row.line,
                &row.payload,
                ratatui::style::Style::new(),
            );
        },
    );
    assert_eq!(painted, [0, 1, 2, 3, 4]);
    assert!(
        common_row(&buf, inner.y + 1).starts_with("payload 1"),
        "the content closure drew inside each row"
    );

    let _ = list.select(15, &items[..]);
    list.layout(inner, &items[..], &ctx);
    let scrolled = list.snapshot(inner, &items[..], |i| i);
    assert!(scrolled.more_above(), "rows are scrolled off above");
    assert!(scrolled.rows().iter().any(|row| row.index == 15));
}

fn common_row(buf: &Buffer, y: u16) -> String {
    (2..32)
        .map(|x| buf[(x, y)].symbol())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

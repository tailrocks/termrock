use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::picker::Picker;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

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

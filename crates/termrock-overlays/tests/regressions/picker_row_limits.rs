use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use termrock_core::focus::FocusRing;
use termrock_core::hit::HitRegistry;
use termrock_core::id::{ItemKey, WidgetId};
use termrock_overlays::option_list::Item as ListItem;
use termrock_overlays::picker::{Picker, PickerStatus};
use termrock_runtime::ctx::{Interaction, RenderCtx};
use termrock_runtime::popup::backdrop_id;
use termrock_theme::{ColorLevel, Theme};

fn picker_with_items(id: &str, count: usize) -> Picker {
    let mut picker = Picker::new(WidgetId::of(id), "Rows");
    picker
        .set_items(
            (0..count)
                .map(|index| ListItem::new(format!("Item {index}")).key(ItemKey::Index(index)))
                .collect(),
        )
        .expect("valid picker update");
    picker
}

fn draw(picker: &mut Picker, area: Rect) -> (Buffer, HitRegistry) {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buffer = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    picker.draw(area, &mut buffer, &mut ctx);
    ctx.flush_overlays(&mut buffer);
    drop(ctx);
    (buffer, hits)
}

fn popup_rows(buffer: &Buffer, popup: Rect) -> Vec<String> {
    (popup.y..popup.bottom())
        .map(|y| {
            (popup.x..popup.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect()
        })
        .collect()
}

fn assert_hits_inside_popup(picker: &Picker, hits: &HitRegistry, context: &str) {
    let popup = picker.area;
    assert!(!popup.is_empty(), "{context}: popup was not drawn");
    let backdrop = backdrop_id(picker.id);
    let mut inside = 0;
    for region in hits.regions() {
        if region.id == backdrop || region.area.is_empty() {
            continue;
        }
        let a = region.area;
        assert!(
            a.x >= popup.x
                && a.y >= popup.y
                && a.right() <= popup.right()
                && a.bottom() <= popup.bottom(),
            "{context}: region {a:?} escapes popup {popup:?}"
        );
        inside += 1;
    }
    assert!(inside > 0, "{context}: popup registered no hit regions");
}

#[test]
fn picker_one_row_provider_status_transition() {
    let area = Rect::new(7, 3, 60, 14);
    for searchable in [false, true] {
        let mut picker = picker_with_items("one-row-transition", 5).max_rows(1);
        picker
            .set_searchable(searchable)
            .expect("indexed items are searchable");
        let transition = [
            (PickerStatus::Ready, "Item 0"),
            (PickerStatus::Loading("Fetching".into()), "Fetching"),
            (
                PickerStatus::Error {
                    message: "Offline".into(),
                    detail: Some("Retry later".into()),
                },
                "Offline",
            ),
            (PickerStatus::Ready, "Item 0"),
        ];
        let mut heights = Vec::new();
        for (step, (status, label)) in transition.into_iter().enumerate() {
            let context = format!("searchable={searchable} step={step} label={label}");
            picker.status = status;
            let (buffer, hits) = draw(&mut picker, area);
            let rows = popup_rows(&buffer, picker.area);
            assert_eq!(
                rows.iter().filter(|row| row.contains(label)).count(),
                1,
                "{context}: expected exactly one row with the label in {rows:?}"
            );
            assert!(
                !rows.iter().any(|row| row.contains("Retry later")),
                "{context}: the status detail needs a second row and must be omitted: {rows:?}"
            );
            if step % 3 != 0 {
                assert!(
                    !rows.iter().any(|row| row.contains("Item ")),
                    "{context}: a non-ready status must replace the items: {rows:?}"
                );
            }
            assert_hits_inside_popup(&picker, &hits, &context);
            assert!(
                picker.area.right() <= area.right() && picker.area.bottom() <= area.bottom(),
                "{context}: popup {:?} leaves the buffer {area:?}",
                picker.area
            );
            heights.push(picker.height());
        }
        assert_eq!(
            heights.first(),
            heights.last(),
            "returning to Ready restores the Ready height"
        );
    }
}

#[test]
fn picker_large_count_uses_bounded_height() {
    let statuses = || {
        [
            PickerStatus::Ready,
            PickerStatus::Loading("Fetching".into()),
            PickerStatus::Error {
                message: "Offline".into(),
                detail: Some("Retry later".into()),
            },
        ]
    };
    let mut large = picker_with_items("large-count", 65_536).max_rows(8);
    let mut eight = picker_with_items("eight-count", 8).max_rows(8);
    for status in statuses() {
        large.status = status.clone();
        eight.status = status.clone();
        assert_eq!(
            large.height(),
            eight.height(),
            "{status:?}: 65,536 items must size like eight"
        );
    }
    large.status = PickerStatus::Ready;
    eight.status = PickerStatus::Ready;
    let area = Rect::new(2, 1, 60, 24);
    let (buffer, hits) = draw(&mut large, area);
    let (_, eight_hits) = draw(&mut eight, area);
    let rows = popup_rows(&buffer, large.area);
    assert!(rows.iter().any(|row| row.contains("Item 0")), "{rows:?}");
    assert!(
        !rows.iter().any(|row| row.contains("Item 8")),
        "a 65,536-item picker stops at its eight configured rows: {rows:?}"
    );
    assert_eq!(
        large.area, eight.area,
        "popup geometry matches the eight-item picker"
    );
    assert_hits_inside_popup(&large, &hits, "large count");
    assert_hits_inside_popup(&eight, &eight_hits, "eight count");
}

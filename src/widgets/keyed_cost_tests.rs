//! Deterministic work counts for collection projections and overlay paint.
//!
//! Counters are work units from `core::work` (unit tests only), never timings.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use crate::core::event::Key;
use crate::core::focus::FocusRing;
use crate::core::hit::HitRegistry;
use crate::core::id::{ItemKey, WidgetId};
use crate::core::keyed::{KeyedRecord, KeyedState, KeyedVec};
use crate::core::work::Work;
use crate::theme::{ColorLevel, Theme};
use crate::ui::ctx::{Interaction, RenderCtx};
use crate::widgets::completion::Completion;
use crate::widgets::option_editor::{
    ChoiceEditor, ColumnEditor, EditorConfig, OptionItem, Options,
};
use crate::widgets::option_list::Item;
use crate::widgets::picker::Picker;
use crate::widgets::select::Select;

const SCREEN: Rect = Rect::new(0, 0, 80, 30);

fn overlay_frame(paint: impl FnOnce(&mut Buffer, &mut RenderCtx)) -> Buffer {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(SCREEN);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    paint(&mut buf, &mut ctx);
    ctx.flush_overlays(&mut buf);
    buf
}

fn keyed_items(n: usize) -> Vec<Item> {
    (0..n)
        .map(|i| Item::new(format!("item {i}")).key(ItemKey::num(i as u64)))
        .collect()
}

#[test]
fn picker_projection_join_is_linear() {
    const N: usize = 3_000;
    const M: usize = 1_500;
    let mut picker = Picker::new(WidgetId::of("pk"), "Projection");
    let (set, work) = Work::measure(|| picker.set_items(keyed_items(N)));
    set.expect("unique keys");
    assert_eq!(work.index_builds, N as u64, "{work:?}");

    let visible: Vec<Item> = (0..M)
        .map(|i| {
            Item::new("")
                .key(ItemKey::num((N - 1 - i * 2) as u64))
                .matched(vec![0])
        })
        .collect();
    let (result, work) = Work::measure(|| picker.set_visible_items(visible));
    result.expect("every visible key is authoritative");
    assert_eq!(
        work.join_probes, M as u64,
        "one lookup per visible key: {work:?}"
    );
    assert_eq!(work.index_builds, 0, "{work:?}");
    assert_eq!(
        picker.item_for_key(ItemKey::num(7)).expect("known").label,
        "item 7"
    );
}

#[test]
fn overlay_snapshot_copies_visible_rows_only() {
    let options: Vec<String> = (0..100_000)
        .map(|i| format!("option number {i:06}"))
        .collect();
    let refs: Vec<&str> = options.iter().map(String::as_str).collect();
    let mut select = Select::new(WidgetId::of("sel"), "Pick", &refs, 0);
    select.on_key(&Key {
        code: KeyCode::Enter,
        mods: KeyModifiers::NONE,
    });
    assert!(select.open);
    let ((), work) = Work::measure(|| {
        overlay_frame(|buf, ctx| select.render(Rect::new(0, 0, 40, 3), buf, ctx, ctx.theme.canvas));
    });
    assert!((1..=8).contains(&work.snapshot_rows), "{work:?}");
    assert_eq!(work.width_rows, 0, "{work:?}");

    let mut completion = Completion::new(WidgetId::of("completion"));
    completion.open(keyed_items(100_000), Rect::new(2, 2, 1, 1), 0);
    let ((), first) = Work::measure(|| {
        overlay_frame(|buf, ctx| completion.render(SCREEN, buf, ctx));
    });
    assert!((1..=8).contains(&first.snapshot_rows), "{first:?}");
    assert_eq!(
        first.width_rows, 100_000,
        "the first frame measures once: {first:?}"
    );
    let ((), again) = Work::measure(|| {
        overlay_frame(|buf, ctx| completion.render(SCREEN, buf, ctx));
    });
    assert_eq!(
        again.width_rows, 0,
        "unchanged items are not re-measured: {again:?}"
    );
    assert!((1..=8).contains(&again.snapshot_rows), "{again:?}");
    completion.set_items(keyed_items(10));
    let ((), changed) = Work::measure(|| {
        overlay_frame(|buf, ctx| completion.render(SCREEN, buf, ctx));
    });
    assert_eq!(changed.width_rows, 10, "{changed:?}");
}

#[test]
fn overlay_snapshot_choice_editor_copies_visible_rows_only() {
    let cfg = EditorConfig::default();
    let options = || {
        Options::new((0..50_000).map(|i| {
            let item = OptionItem::new(&format!("v{i}")).label(&format!("label {i}"));
            if i == 2 {
                item.disabled("locked")
            } else {
                item
            }
        }))
    };
    for editor in [
        ColumnEditor::Select(options()),
        ColumnEditor::Tags(options()),
        ColumnEditor::Segmented(options()),
    ] {
        let mut ed = ChoiceEditor::open(WidgetId::of("ed"), &editor, "v0", &cfg).expect("popup");
        let rect = Rect::new(1, 1, 20, 1);
        let ((), first) = Work::measure(|| {
            overlay_frame(|buf, ctx| ed.render(rect, buf, ctx, &cfg));
        });
        assert!(
            (1..=16).contains(&first.snapshot_rows),
            "{editor:?} {first:?}"
        );
        let ((), again) = Work::measure(|| {
            overlay_frame(|buf, ctx| ed.render(rect, buf, ctx, &cfg));
        });
        assert_eq!(
            (again.width_rows, again.source_rows),
            (0, 0),
            "{editor:?} {again:?}"
        );
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Row {
    off: bool,
}

impl KeyedRecord for Row {
    fn label(&self) -> &str {
        "row"
    }
    fn enabled(&self) -> bool {
        !self.off
    }
}

#[test]
fn keyed_unchanged_stamp_does_not_enumerate_the_source() {
    let mut store =
        KeyedVec::try_from_rows((1..=500).map(|n| (ItemKey::num(n), Row { off: false })))
            .expect("valid");
    let mut state = KeyedState::new();
    let (_, first) = Work::measure(|| state.reconcile(&store).expect("valid"));
    assert_eq!(
        first.source_rows, 1_000,
        "authoritative plus visible, once: {first:?}"
    );
    let (out, again) = Work::measure(|| state.reconcile(&store).expect("valid"));
    assert_eq!((out, again.source_rows), (Default::default(), 0));

    store.update(ItemKey::num(7), |row| row.off = true);
    let (out, work) = Work::measure(|| state.reconcile(&store).expect("valid"));
    assert!(out.data_changed);
    assert_eq!(work.source_rows, 1_000, "{work:?}");
    assert!(!state.is_enabled(ItemKey::num(7)));
}

#[test]
fn keyed_resize_cancels_capture_without_source_scan() {
    let store = KeyedVec::try_from_rows((1..=3).map(|n| (ItemKey::num(n), Row { off: false })))
        .expect("valid");
    let mut state = KeyedState::new();
    state.reconcile(&store).expect("valid");
    assert!(state.capture_press(ItemKey::num(2), 5));
    let (flags, work) = Work::measure(|| {
        (
            state.invalidate_layout(5),
            state.invalidate_layout(6),
            state.reconcile(&store).expect("valid"),
        )
    });
    assert_eq!((flags.0, flags.1), (false, true));
    assert_eq!(flags.2, Default::default());
    assert_eq!(work.source_rows, 0, "{work:?}");
    assert!(!state.release_press(ItemKey::num(2), 5));
}

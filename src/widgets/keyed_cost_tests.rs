//! Deterministic work counts for collection projections and overlay paint.
//!
//! Counters are the #56 work metrics (`work-metrics` feature), never timings.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use crate::core::event::{Key, Outcome};
use crate::core::focus::FocusRing;
use crate::core::hit::HitRegistry;
use crate::core::id::{ItemKey, WidgetId};
use crate::core::keyed::{KeyedRecord, KeyedState, KeyedVec};
use crate::core::refusal::{Reason, take_refusal};
use crate::metrics::{Metric, WorkDelta, measure};
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

/// The metrics a delta recorded, for assertion messages.
pub(crate) fn nz(delta: &WorkDelta) -> Vec<(Metric, u64)> {
    delta.nonzero().collect()
}

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
    let (set, work) = measure(|| picker.set_items(keyed_items(N)));
    set.expect("unique keys");
    assert_eq!(
        work.get(Metric::KeyIndexInserts),
        N as u64,
        "{:?}",
        nz(&work)
    );

    let visible: Vec<Item> = (0..M)
        .map(|i| {
            Item::new("")
                .key(ItemKey::num((N - 1 - i * 2) as u64))
                .matched(vec![0])
        })
        .collect();
    let (result, work) = measure(|| picker.set_visible_items(visible));
    result.expect("every visible key is authoritative");
    assert_eq!(
        work.get(Metric::KeyLookups),
        M as u64,
        "one lookup per visible key: {:?}",
        nz(&work)
    );
    assert_eq!(work.get(Metric::KeyIndexInserts), 0, "{:?}", nz(&work));
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
    let ((), work) = measure(|| {
        overlay_frame(|buf, ctx| select.render(Rect::new(0, 0, 40, 3), buf, ctx, ctx.theme.canvas));
    });
    assert!(
        (1..=8).contains(&work.get(Metric::OverlayRowsSnapshotted)),
        "{:?}",
        nz(&work)
    );
    assert_eq!(work.get(Metric::WidthRowsMeasured), 0, "{:?}", nz(&work));

    let mut completion = Completion::new(WidgetId::of("completion"));
    completion.open(keyed_items(100_000), Rect::new(2, 2, 1, 1), 0);
    let ((), first) = measure(|| {
        overlay_frame(|buf, ctx| completion.render(SCREEN, buf, ctx));
    });
    assert!(
        (1..=8).contains(&first.get(Metric::OverlayRowsSnapshotted)),
        "{:?}",
        nz(&first)
    );
    assert_eq!(
        first.get(Metric::WidthRowsMeasured),
        100_000,
        "the first frame measures once: {:?}",
        nz(&first)
    );
    let ((), again) = measure(|| {
        overlay_frame(|buf, ctx| completion.render(SCREEN, buf, ctx));
    });
    assert_eq!(
        again.get(Metric::WidthRowsMeasured),
        0,
        "unchanged items are not re-measured: {:?}",
        nz(&again)
    );
    assert!(
        (1..=8).contains(&again.get(Metric::OverlayRowsSnapshotted)),
        "{:?}",
        nz(&again)
    );
    completion.set_items(keyed_items(10));
    let ((), changed) = measure(|| {
        overlay_frame(|buf, ctx| completion.render(SCREEN, buf, ctx));
    });
    assert_eq!(
        changed.get(Metric::WidthRowsMeasured),
        10,
        "{:?}",
        nz(&changed)
    );
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
        let ((), first) = measure(|| {
            overlay_frame(|buf, ctx| ed.render(rect, buf, ctx, &cfg));
        });
        assert!(
            (1..=16).contains(&first.get(Metric::OverlayRowsSnapshotted)),
            "{editor:?} {:?}",
            nz(&first)
        );
        let ((), again) = measure(|| {
            overlay_frame(|buf, ctx| ed.render(rect, buf, ctx, &cfg));
        });
        assert_eq!(
            (
                again.get(Metric::WidthRowsMeasured),
                again.get(Metric::SourceRowsEnumerated)
            ),
            (0, 0),
            "{editor:?} {:?}",
            nz(&again)
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
    let (_, first) = measure(|| state.reconcile(&store).expect("valid"));
    assert_eq!(
        first.get(Metric::SourceRowsEnumerated),
        1_000,
        "authoritative plus visible, once: {:?}",
        nz(&first)
    );
    let (out, again) = measure(|| state.reconcile(&store).expect("valid"));
    assert_eq!(
        (out, again.get(Metric::SourceRowsEnumerated)),
        (Default::default(), 0)
    );

    store.update(ItemKey::num(7), |row| row.off = true);
    let (out, work) = measure(|| state.reconcile(&store).expect("valid"));
    assert!(out.data_changed);
    assert_eq!(
        work.get(Metric::SourceRowsEnumerated),
        1_000,
        "{:?}",
        nz(&work)
    );
    assert!(!state.is_enabled(ItemKey::num(7)));
}

#[test]
fn keyed_resize_cancels_capture_without_source_scan() {
    let store = KeyedVec::try_from_rows((1..=3).map(|n| (ItemKey::num(n), Row { off: false })))
        .expect("valid");
    let mut state = KeyedState::new();
    state.reconcile(&store).expect("valid");
    assert!(state.capture_press(ItemKey::num(2), 5));
    let (flags, work) = measure(|| {
        (
            state.invalidate_layout(5),
            state.invalidate_layout(6),
            state.reconcile(&store).expect("valid"),
        )
    });
    assert_eq!((flags.0, flags.1), (false, true));
    assert_eq!(flags.2, Default::default());
    assert_eq!(work.get(Metric::SourceRowsEnumerated), 0, "{:?}", nz(&work));
    assert!(!state.release_press(ItemKey::num(2), 5));
}

#[test]
fn list_detail_unchanged_widths_are_cached() {
    use crate::ui::layout::Split;
    use crate::widgets::list_detail::{ListDetail, ListRow, SourceFrame};

    let mut store = KeyedVec::try_from_rows((0..5_000).map(|n| {
        (
            ItemKey::num(n),
            ListRow::new(format!("row {n}")).detail("detail"),
        )
    }))
    .expect("valid");
    let mut detail = ListDetail::new(WidgetId::of("m"), WidgetId::of("d"), Split::new(50, 36, 40));
    let frame = SourceFrame {
        title: "Rows",
        meta: "",
        head: &[],
        under: None,
        detail_title: "Detail",
        detail_meta: "",
        lines: &[],
        actions: &[],
    };
    let paint = |detail: &mut ListDetail, store: &KeyedVec<ListRow>| {
        overlay_frame(|buf, ctx| {
            detail
                .render_source(SCREEN, store, &frame, buf, ctx)
                .expect("valid source");
        });
    };
    let ((), first) = measure(|| paint(&mut detail, &store));
    assert_eq!(
        first.get(Metric::WidthRowsMeasured),
        5_000,
        "{:?}",
        nz(&first)
    );
    let ((), again) = measure(|| paint(&mut detail, &store));
    assert_eq!(
        again.get(Metric::WidthRowsMeasured),
        0,
        "unchanged stamp measures no row: {:?}",
        nz(&again)
    );
    assert_eq!(
        again.get(Metric::SourceRowsEnumerated),
        0,
        "{:?}",
        nz(&again)
    );

    store.update(ItemKey::num(3), |row| {
        row.label = "row three, renamed".into()
    });
    let ((), changed) = measure(|| paint(&mut detail, &store));
    assert_eq!(
        changed.get(Metric::WidthRowsMeasured),
        5_000,
        "a data revision re-measures once: {:?}",
        nz(&changed)
    );
}

fn select_key(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

#[test]
fn select_fixed_probe_never_clones_options() {
    for n in [1_000, 100_000] {
        let labels: Vec<String> = (0..n).map(|i| format!("option number {i:06}")).collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();

        let probe = Select::new(WidgetId::of("fixed"), "Pick", &refs, 0);
        let (copy, work) = measure(|| probe.clone());
        assert_eq!(
            work.get(Metric::PayloadBytesCloned),
            labels.iter().map(String::len).sum::<usize>() as u64,
            "counter sees a clone: {:?}",
            nz(&work)
        );
        drop(copy);

        let mut live = Select::new(WidgetId::of("live"), "Pick", &refs, 0);
        let (_, work) = measure(|| live.on_key(&select_key(KeyCode::Char('o'))));
        assert!(
            work.get(Metric::LabelsRead) > 0,
            "counter sees a label search: {:?}",
            nz(&work)
        );

        let mut select = Select::new(WidgetId::of("fixed"), "Pick", &refs, 0)
            .fixed(Reason::new("Always off").short("managed"));
        let keys = [
            (KeyCode::Char('o'), true),
            (KeyCode::Enter, true),
            (KeyCode::Down, true),
            (KeyCode::Right, false),
            (KeyCode::F(5), false),
            (KeyCode::Char('?'), false),
        ];
        for (code, refused) in keys {
            let before = (select.selected, select.open, select.list.cursor.index);
            let (out, work) = measure(|| select.on_key(&select_key(code)));
            assert_eq!(
                (
                    work.get(Metric::PayloadBytesCloned),
                    work.get(Metric::LabelsRead)
                ),
                (0, 0),
                "{n} options, {code:?}: {:?}",
                nz(&work)
            );
            assert_eq!(
                (select.selected, select.open, select.list.cursor.index),
                before,
                "{code:?} changed state"
            );
            assert_eq!(out.0 == Outcome::Consumed, refused, "{code:?}");
            assert_eq!(take_refusal().is_some(), refused, "{code:?}");
        }
    }
}

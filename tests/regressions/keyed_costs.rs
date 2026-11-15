//! Deterministic cost tests for collection projections and overlay paint.
//!
//! Counters are work units from `junie_tui::core::work`, never timings.

use crate::common::*;
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::core::work::Work;
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::widgets::completion::Completion;
use junie_tui::widgets::option_editor::{
    ChoiceEditor, ColumnEditor, EditorConfig, OptionItem, Options,
};
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::picker::Picker;
use junie_tui::widgets::select::Select;
use junie_tui::{core::focus::FocusRing, core::hit::HitRegistry, theme::ColorLevel, theme::Theme};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

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

fn screen_text(buf: &Buffer) -> String {
    (0..buf.area.height)
        .map(|y| row(buf, y))
        .collect::<Vec<_>>()
        .join("\n")
}

fn many_options(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("option number {i:06}")).collect()
}

fn select_with(options: &[String]) -> Select {
    let refs: Vec<&str> = options.iter().map(String::as_str).collect();
    Select::new(WidgetId::of("sel"), "Pick", &refs, 0)
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
    assert!(
        work.join_probes + work.index_builds <= (N + N) as u64,
        "building the authoritative set costs O(N): {work:?}"
    );

    let visible: Vec<Item> = (0..M)
        .map(|i| {
            Item::new("")
                .key(ItemKey::num((N - 1 - i * 2) as u64))
                .matched(vec![0])
        })
        .collect();
    let (result, work) = Work::measure(|| picker.set_visible_items(visible));
    result.expect("every visible key is authoritative");
    assert!(
        work.join_probes + work.index_builds <= (N + M) as u64,
        "joining M visible keys costs N index entries plus M lookups, not N*M: {work:?}"
    );
    assert_eq!(
        picker.item_for_key(ItemKey::num(7)).expect("known").label,
        "item 7"
    );
}

#[test]
fn overlay_snapshot_copies_visible_rows_only() {
    let options = many_options(100_000);
    let mut select = select_with(&options);
    select.on_key(&key(KeyCode::Enter));
    assert!(select.open);
    let (buf, work) = {
        let mut buf = None;
        let ((), work) = Work::measure(|| {
            buf = Some(overlay_frame(|buf, ctx| {
                select.render(Rect::new(0, 0, 40, 3), buf, ctx, ctx.theme.canvas)
            }));
        });
        (buf.expect("painted"), work)
    };
    assert_eq!(
        work.payload_rows_copied, 0,
        "an open Select must not copy its options: {work:?}"
    );
    assert!(
        (1..=8).contains(&work.snapshot_rows),
        "only the visible rows are prepared: {work:?}"
    );
    let text = screen_text(&buf);
    assert!(text.contains("option number 000000"), "{text}");
    assert!(text.contains("option number 000007"), "{text}");
    assert!(!text.contains("option number 000008"), "{text}");
}

#[test]
fn overlay_snapshot_completion_copies_visible_rows_only() {
    let mut completion = Completion::new(WidgetId::of("completion"));
    completion.open(keyed_items(100_000), Rect::new(2, 2, 1, 1), 0);
    let mut buf = None;
    let ((), work) = Work::measure(|| {
        buf = Some(overlay_frame(|buf, ctx| {
            completion.render(SCREEN, buf, ctx)
        }));
    });
    assert_eq!(work.payload_rows_copied, 0, "{work:?}");
    assert!((1..=8).contains(&work.snapshot_rows), "{work:?}");
    let text = screen_text(&buf.expect("painted"));
    assert!(text.contains("item 0"), "{text}");
}

fn choice_options(n: usize) -> Options {
    Options::new((0..n).map(|i| {
        let item = OptionItem::new(&format!("v{i}")).label(&format!("label {i}"));
        if i == 2 {
            item.disabled("locked")
        } else {
            item
        }
    }))
}

#[test]
fn overlay_snapshot_choice_editor_copies_visible_rows_only() {
    let cfg = EditorConfig::default();
    for editor in [
        ColumnEditor::Select(choice_options(50_000)),
        ColumnEditor::Tags(choice_options(50_000)),
        ColumnEditor::Segmented(choice_options(50_000)),
    ] {
        let mut ed = ChoiceEditor::open(WidgetId::of("ed"), &editor, "v0", &cfg).expect("popup");
        let mut buf = None;
        let ((), work) = Work::measure(|| {
            buf = Some(overlay_frame(|buf, ctx| {
                ed.render(Rect::new(1, 1, 20, 1), buf, ctx, &cfg)
            }));
        });
        assert_eq!(work.payload_rows_copied, 0, "{editor:?} {work:?}");
        assert!(work.snapshot_rows <= 16, "{work:?}");
        let text = screen_text(&buf.expect("painted"));
        assert!(text.contains("label 0"), "{text}");
    }
}

#[test]
fn select_fixed_probe_never_clones_options() {
    for n in [1_000, 100_000] {
        let options = many_options(n);
        let mut select = select_with(&options).fixed(reason());
        for code in [
            KeyCode::Char('o'),
            KeyCode::Enter,
            KeyCode::Down,
            KeyCode::F(5),
        ] {
            let before = (select.selected, select.open);
            let (out, work) = Work::measure(|| select.on_key(&key(code)));
            assert_eq!(
                work.payload_rows_copied, 0,
                "{code:?} with {n} options copied the model"
            );
            assert_eq!(
                (select.selected, select.open),
                before,
                "{code:?} changed state"
            );
            let _ = out;
        }
    }
}

#[test]
fn select_fixed_probe_matches_live_outcome() {
    use junie_tui::core::event::Outcome;
    use ratatui::crossterm::event::KeyModifiers;
    let keys = [
        key(KeyCode::Enter),
        key(KeyCode::Char(' ')),
        key(KeyCode::Up),
        key(KeyCode::Down),
        key(KeyCode::Left),
        key(KeyCode::Right),
        key(KeyCode::Tab),
        key(KeyCode::BackTab),
        key(KeyCode::Esc),
        key(KeyCode::Char('?')),
        key(KeyCode::F(2)),
        key(KeyCode::Char('x')),
        key(KeyCode::Char('o')),
        key(KeyCode::Home),
        key(KeyCode::End),
        chord(KeyCode::Down, KeyModifiers::SHIFT),
        chord(KeyCode::Char('x'), KeyModifiers::CONTROL),
    ];
    let words = ["alpha", "bravo", "charlie"];
    let make = |n: usize, open: bool, arrows: junie_tui::widgets::select::SelectArrows| {
        let mut s = Select::new(WidgetId::of("s"), "S", &words[..n], 0);
        s.arrows = arrows;
        s.open = open;
        s
    };
    use junie_tui::widgets::select::{SelectArrows, TabInOpen};
    let mut checked = 0;
    for n in [0usize, 3] {
        for open in [false, true] {
            for arrows in [SelectArrows::Open, SelectArrows::Step] {
                for tab in [TabInOpen::Choose, TabInOpen::Close, TabInOpen::Swallow] {
                    for escape in [true, false] {
                        for k in &keys {
                            let configure = |mut s: Select| {
                                s.tab_in_open = tab;
                                s.list.dismiss.escape = escape;
                                s
                            };
                            let mut live = configure(make(n, open, arrows));
                            let (outcome, action) = live.on_key(k);
                            let handled = outcome != Outcome::Ignored || action.is_some();

                            let mut fixed = configure(make(n, open, arrows)).fixed(reason());
                            let (probe, probe_action) = fixed.on_key(k);
                            assert_eq!(probe_action, None);
                            assert_eq!(
                                probe == Outcome::Consumed,
                                handled,
                                "n={n} open={open} {arrows:?} {tab:?} esc={escape} {k:?}"
                            );
                            assert_eq!(taken().is_some(), handled);
                            checked += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(checked > 600);
}

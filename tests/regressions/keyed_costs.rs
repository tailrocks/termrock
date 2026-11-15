//! Deterministic cost tests for collection projections and overlay paint.
//!
//! Work-count assertions live in `src/widgets/keyed_cost_tests.rs`.

use crate::common::*;
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::widgets::completion::Completion;
use junie_tui::widgets::option_editor::{
    ChoiceEditor, ColumnEditor, EditorConfig, OptionItem, Options,
};
use junie_tui::widgets::option_list::Item;
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
fn overlay_snapshot_paints_only_the_window_with_correct_rows() {
    let options = many_options(100_000);
    let mut select = select_with(&options);
    select.on_key(&key(KeyCode::Enter));
    assert!(select.open);
    let buf =
        overlay_frame(|buf, ctx| select.render(Rect::new(0, 0, 40, 3), buf, ctx, ctx.theme.canvas));
    let text = screen_text(&buf);
    assert!(text.contains("option number 000000"), "{text}");
    assert!(text.contains("option number 000007"), "{text}");
    assert!(!text.contains("option number 000008"), "{text}");

    let mut completion = Completion::new(WidgetId::of("completion"));
    completion.open(keyed_items(1_000), Rect::new(2, 2, 1, 1), 0);
    let buf = overlay_frame(|buf, ctx| completion.render(SCREEN, buf, ctx));
    assert!(screen_text(&buf).contains("item 0"));

    let cfg = EditorConfig::default();
    for editor in [
        ColumnEditor::Select(choice_options(500)),
        ColumnEditor::Tags(choice_options(500)),
        ColumnEditor::Segmented(choice_options(500)),
    ] {
        let mut ed = ChoiceEditor::open(WidgetId::of("ed"), &editor, "v0", &cfg).expect("popup");
        let buf = overlay_frame(|buf, ctx| ed.render(Rect::new(1, 1, 20, 1), buf, ctx, &cfg));
        assert!(screen_text(&buf).contains("label 0"));
    }
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
            let out = select.on_key(&key(code));
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

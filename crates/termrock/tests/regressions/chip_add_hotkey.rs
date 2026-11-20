use crate::common::*;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use termrock::core::event::Outcome;
use termrock::core::id::WidgetId;
use termrock::core::keymap::{KeyBinding, KeyMap};
use termrock::widgets::chips::{Chip, ChipAction, ChipBar, ChipCmd};

const CHIPS: WidgetId = WidgetId::of("add_hotkey.chips");
const WIDTH: u16 = 60;

fn render(cb: &mut ChipBar) {
    draw(None, |buf, ctx| {
        cb.render(Rect::new(0, 0, WIDTH, 1), buf, ctx, ctx.theme.canvas);
    });
}

#[test]
fn plus_adds_when_the_bar_has_an_add_action() {
    let mut cb = ChipBar::new(CHIPS);
    cb.set_chips(vec![Chip::new("a")]);
    render(&mut cb);

    let (outcome, action) = cb.on_key(&key(KeyCode::Char('+')));

    assert_eq!(action, Some(ChipAction::Add), "+ adds a filter");
    assert!(cb.add_label().is_some(), "the bar offers an add label");
    assert_eq!(outcome, Outcome::Changed);
}

#[test]
fn plus_is_ignored_when_the_bar_has_no_add_action() {
    let mut cb = ChipBar::new(CHIPS);
    cb.set_chips(vec![Chip::new("a")]);
    cb.set_add_label(None);
    assert_eq!(cb.add_label(), None);
    render(&mut cb);

    let (outcome, action) = cb.on_key(&key(KeyCode::Char('+')));

    assert_eq!(action, None, "+ must not emit Add without an add action");
    assert_eq!(outcome, Outcome::Ignored, "+ is not consumed");
}

#[test]
fn the_add_key_hint_follows_the_add_label() {
    let mut cb = ChipBar::new(CHIPS);
    assert_eq!(cb.key_hint(ChipCmd::Add), Some("+"), "an add bar hints +");

    assert!(cb.set_add_label(None));
    assert_eq!(cb.key_hint(ChipCmd::Add), None, "no add label, no + hint");
    assert_eq!(
        cb.key_hint(ChipCmd::Remove),
        Some("Delete"),
        "the other commands keep their hint"
    );

    assert!(cb.set_add_label(Some("+ New".into())));
    assert_eq!(cb.key_hint(ChipCmd::Add), Some("+"), "the hint comes back");
}

#[test]
fn the_add_key_hint_names_a_custom_binding_and_never_a_dead_one() {
    let keymap: KeyMap<ChipCmd> = vec![(KeyBinding::char('n'), ChipCmd::Add)];
    let mut cb = ChipBar::new(CHIPS).keymap(keymap);
    assert_eq!(cb.key_hint(ChipCmd::Add), Some("n"));
    assert_eq!(cb.key_hint(ChipCmd::Remove), None, "an unbound command");
    cb.set_chips(vec![Chip::new("a")]);
    render(&mut cb);
    let (_, action) = cb.on_key(&key(KeyCode::Char('n')));
    assert_eq!(action, Some(ChipAction::Add), "the hinted key does add");

    assert!(cb.set_add_label(None));
    assert_eq!(cb.key_hint(ChipCmd::Add), None);
    let (outcome, action) = cb.on_key(&key(KeyCode::Char('n')));
    assert_eq!((outcome, action), (Outcome::Ignored, None), "and is dead");
}

#[test]
fn the_lead_key_hint_needs_a_lead_label() {
    let mut cb = ChipBar::new(CHIPS);
    assert_eq!(cb.key_hint(ChipCmd::Lead), None, "no lead, no hint");
    assert!(cb.set_lead(Some("match all".into())));
    assert_eq!(cb.key_hint(ChipCmd::Lead), Some("m"));
}

#[test]
fn the_lead_label_is_read_back_and_set_lead_reports_a_change() {
    let mut cb = ChipBar::new(CHIPS);
    assert_eq!(cb.lead(), None, "a fresh bar has no lead");
    assert!(cb.set_lead(Some("Filters".to_owned())));
    assert_eq!(cb.lead(), Some("Filters"));
    assert!(
        !cb.set_lead(Some("Filters".to_owned())),
        "an equal lead is no change"
    );
    assert!(cb.set_lead(None));
    assert_eq!(cb.lead(), None);
}

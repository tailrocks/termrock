use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use termrock::core::event::Key;
use termrock::core::focus::{Focus, FocusRing};
use termrock::core::id::WidgetId;
use termrock::widgets::button::{Button, ButtonRow, RowOverflow};
use termrock::widgets::choice_dialog::ChoiceDialog;
use termrock::widgets::dialog::Dialog;
use termrock::widgets::form_dialog::{FormDialog, FormField};
use termrock::widgets::input::TextInput;

use crate::common::{key, paint_in};

const NAMES: [&str; 4] = ["alpha", "bravo", "charlie", "delta"];

/// Presses Right and Left from `start` and checks focus only ever lands on a
/// widget the last frame registered. Returns whether some press left a button
/// unplaced, so a caller can insist the screen size exercised the defect.
fn arrows_stay_on_placed_buttons(
    mut press: impl FnMut(&Key, &mut Focus),
    start: WidgetId,
    ring: &FocusRing,
    what: &str,
) {
    for code in [KeyCode::Right, KeyCode::Left] {
        let mut focus = Focus::default();
        focus.focus(start);
        for n in 0..12 {
            press(&key(code), &mut focus);
            let at = focus.current().expect("focus stays on a widget");
            assert!(
                ring.contains(at),
                "{what}: {code:?} press {n} focused an unplaced button"
            );
        }
    }
}

#[test]
fn arrows_in_a_form_dialog_skip_actions_cut_off_by_a_short_screen() {
    let mut cut = 0;
    for h in 3..=14 {
        let id = WidgetId::of("t.form");
        let mut form = FormDialog::new(
            id,
            "Account",
            vec![FormField::input(
                "name",
                TextInput::new(id.sub("name"), "Name"),
            )],
        )
        .width(40);
        for name in NAMES {
            form = form.action(name, Button::subtle(id.sub(name), name));
        }
        let screen = Rect::new(0, 0, 60, h);
        let p = paint_in(screen, None, |buf, ctx| form.render(screen, buf, ctx));
        let buttons = [form.cancel.id, form.save.id]
            .into_iter()
            .chain(NAMES.iter().map(|n| id.sub(n)))
            .collect::<Vec<_>>();
        let placed = buttons.iter().filter(|b| p.ring.contains(**b)).count();
        if placed == 0 || placed == buttons.len() {
            continue;
        }
        cut += 1;
        let start = *buttons.iter().find(|b| p.ring.contains(**b)).expect("one");
        arrows_stay_on_placed_buttons(
            |k, f| {
                form.on_key(k, f, &p.ring);
            },
            start,
            &p.ring,
            &format!("form dialog, height {h}"),
        );
    }
    assert!(cut > 0, "no screen height cut the footer");
}

#[test]
fn arrows_in_a_dialog_skip_actions_behind_the_more_button() {
    let mut cut = 0;
    for w in 12..=60 {
        let id = WidgetId::of("t.dialog");
        let actions = NAMES.iter().map(|n| Button::subtle(id.sub(n), n)).collect();
        let mut dialog = Dialog::confirm(id, "Title", "Body", "ok")
            .actions(actions, None)
            .button_row(ButtonRow::new().overflow(RowOverflow::More));
        let screen = Rect::new(0, 0, w, 24);
        let p = paint_in(screen, None, |buf, ctx| dialog.render(screen, buf, ctx));
        let placed: Vec<WidgetId> = NAMES
            .iter()
            .map(|n| id.sub(n))
            .filter(|b| p.ring.contains(*b))
            .collect();
        if placed.is_empty() || placed.len() == NAMES.len() {
            continue;
        }
        cut += 1;
        arrows_stay_on_placed_buttons(
            |k, f| {
                dialog.on_key(k, f, &p.ring);
            },
            placed[0],
            &p.ring,
            &format!("dialog, width {w}"),
        );
    }
    assert!(cut > 0, "no width hid an action");
}

#[test]
fn arrows_in_a_choice_dialog_skip_buttons_cut_off_by_a_short_screen() {
    let mut cut = 0;
    for h in 4..=14 {
        let id = WidgetId::of("t.choice");
        let mut dialog = ChoiceDialog::new(id, "Pick", "Kind", &["one", "two"], 0);
        dialog.width = 30;
        dialog.buttons = NAMES.iter().map(|n| Button::subtle(id.sub(n), n)).collect();
        let screen = Rect::new(0, 0, 40, h);
        let p = paint_in(screen, None, |buf, ctx| dialog.render(screen, buf, ctx));
        let placed: Vec<WidgetId> = dialog
            .buttons
            .iter()
            .map(|b| b.id)
            .filter(|b| p.ring.contains(*b))
            .collect();
        if placed.is_empty() || placed.len() == NAMES.len() {
            continue;
        }
        cut += 1;
        arrows_stay_on_placed_buttons(
            |k, f| {
                dialog.on_key(k, f, &p.ring);
            },
            placed[0],
            &p.ring,
            &format!("choice dialog, height {h}"),
        );
    }
    assert!(cut > 0, "no screen height cut the footer");
}

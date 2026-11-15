mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::{chord, key, row};
use junie_tui::core::edit::{EditEntry, FieldKeys};
use junie_tui::core::focus::{BlurAction, Focus, FocusRing};
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::widgets::edit::ValidateWhen;
use junie_tui::widgets::form_dialog::{FormAction, FormDialog, FormField, FormFieldKind};
use junie_tui::widgets::input::{TextInput, ValidateOn};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

const FORM: WidgetId = WidgetId::of("t.form");
const SECRET: WidgetId = WidgetId::of("t.form.secret");
const SCREEN: Rect = Rect::new(0, 0, 100, 30);

fn draw_form(form: &mut FormDialog, focus: Option<WidgetId>) -> Buffer {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(SCREEN);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let interaction = Interaction {
        focus,
        ..Default::default()
    };
    let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
    form.render(SCREEN, &mut buf, &mut ctx);
    buf
}

fn dump(buf: &Buffer) -> String {
    (0..buf.area.height)
        .map(|y| row(buf, y))
        .collect::<Vec<_>>()
        .join("\n")
}

fn input<'a>(form: &'a FormDialog, name: &str) -> &'a TextInput {
    match &form.field(name).expect("field").kind {
        FormFieldKind::Input(i) => i,
        _ => panic!("not an input"),
    }
}

fn input_mut<'a>(form: &'a mut FormDialog, name: &str) -> &'a mut TextInput {
    match &mut form.field_mut(name).expect("field").kind {
        FormFieldKind::Input(i) => i,
        _ => panic!("not an input"),
    }
}

fn type_str(i: &mut TextInput, s: &str) {
    for c in s.chars() {
        i.on_key(&key(KeyCode::Char(c)));
    }
}

fn configured(calls: &Rc<Cell<u32>>) -> TextInput {
    let calls = Rc::clone(calls);
    let mut i = TextInput::new(SECRET, "Token")
        .placeholder("paste a token")
        .required(true)
        .required_message("A token is required")
        .masked()
        .reveal_tail(2)
        .help("Stored in the keychain")
        .plain_label()
        .wrap_label(true)
        .blur_action(BlurAction::Keep)
        .edit_entry(EditEntry::Explicit)
        .field_keys(FieldKeys::Desktop)
        .validate_on(ValidateOn::EveryCommit)
        .validate_when(ValidateWhen::Live)
        .context_menu(false)
        .readout("sk-...")
        .validator(move |s| {
            calls.set(calls.get() + 1);
            s.contains("bad").then(|| "No bad tokens".to_owned())
        });
    i.disabled = false;
    i.field.read_only = false;
    i
}

fn token_form(calls: &Rc<Cell<u32>>) -> FormDialog {
    FormDialog::new(
        FORM,
        "Account",
        vec![FormField::input("token", configured(calls))],
    )
}

#[test]
fn form_value_preserves_policy() {
    let calls = Rc::new(Cell::new(0));
    let mut form = token_form(&calls);
    {
        let i = input_mut(&mut form, "token");
        i.disabled = true;
        i.field.read_only = true;
    }
    form.set_text("token", "credential");

    let i = input(&form, "token");
    assert_eq!(i.text(), "credential");
    assert_eq!(i.id, SECRET);
    assert_eq!(i.label, "Token");
    assert_eq!(i.placeholder, "paste a token");
    assert!(i.masked && i.field.secret, "masking survives");
    assert_eq!(i.reveal_tail, 2);
    assert!(i.required);
    assert_eq!(i.required_message.as_deref(), Some("A token is required"));
    assert!(i.disabled && i.field.read_only);
    assert_eq!(i.help, "Stored in the keychain");
    assert!(i.plain_label && i.wrap_label);
    assert_eq!(i.blur_action, BlurAction::Keep);
    assert_eq!(i.edit_entry, Some(EditEntry::Explicit));
    assert_eq!(i.field_keys, Some(FieldKeys::Desktop));
    assert_eq!(i.validate_on, ValidateOn::EveryCommit);
    assert_eq!(i.field.validate_when, ValidateWhen::Live);
    assert!(!i.edit_menu.enabled);
    assert_eq!(i.readout, "sk-...");
    assert_eq!(calls.get(), 0, "replacing a value never runs the validator");

    let i = input_mut(&mut form, "token");
    i.disabled = false;
    i.field.read_only = false;
    i.set_error(None);
    form.set_text("token", "a bad token");
    let i = input_mut(&mut form, "token");
    assert!(!i.validate(), "the same validator still rejects");
    assert_eq!(calls.get(), 1, "the original validator closure is retained");
    assert_eq!(i.error(), Some("No bad tokens"));
}

#[test]
fn form_value_masked_render_never_reveals_secret() {
    let calls = Rc::new(Cell::new(0));
    let mut form = token_form(&calls);
    form.set_text("token", "credential");

    let screen = dump(&draw_form(&mut form, None));
    assert!(
        !screen.contains("credential") && !screen.contains("redential"),
        "no run of the secret is painted:\n{screen}"
    );
    assert!(screen.contains('\u{2022}'), "the value renders as a mask");
    assert!(
        screen.contains("al"),
        "the configured reveal tail still shows"
    );

    let i = input_mut(&mut form, "token");
    i.begin_edit();
    let (outcome, _) = i.on_key(&chord(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(
        !outcome.consumed(),
        "copy of a masked value is still refused"
    );
}

#[test]
fn form_value_required_custom_still_blocks_save() {
    let ring = FocusRing::default();

    // Required only: an emptied field must not save. (A custom validator
    // currently replaces the required check; composing them is #35.)
    let mut form = FormDialog::new(
        FORM,
        "Account",
        vec![FormField::input(
            "token",
            TextInput::new(SECRET, "Token")
                .required(true)
                .required_message("A token is required")
                .masked(),
        )],
    );
    let mut focus = Focus::default();
    form.set_text("token", "");
    form.on_key(&key(KeyCode::Enter), &mut focus, &ring);
    assert!(
        !form.events.contains(&FormAction::Save),
        "an emptied required field does not save"
    );
    assert_eq!(focus.current(), Some(SECRET), "focus lands on the field");
    assert_eq!(input(&form, "token").error(), Some("A token is required"));
    form.set_text("token", "fine");
    assert_eq!(input(&form, "token").error(), None, "stale error cleared");
    focus.focus(form.save.id);
    form.on_key(&key(KeyCode::Enter), &mut focus, &ring);
    assert!(form.events.contains(&FormAction::Save));

    // Custom validator: a rejected value must not save.
    let calls = Rc::new(Cell::new(0));
    let mut form = token_form(&calls);
    let mut focus = Focus::default();
    form.set_text("token", "a bad token");
    form.on_key(&key(KeyCode::Enter), &mut focus, &ring);
    assert!(!form.events.contains(&FormAction::Save));
    assert_eq!(focus.current(), Some(SECRET));
    assert_eq!(input(&form, "token").error(), Some("No bad tokens"));
    form.set_text("token", "fine");
    focus.focus(form.save.id);
    form.on_key(&key(KeyCode::Enter), &mut focus, &ring);
    assert!(form.events.contains(&FormAction::Save));
}

#[test]
fn form_value_new_cancel_baseline() {
    let calls = Rc::new(Cell::new(0));
    let mut form = token_form(&calls);
    {
        let i = input_mut(&mut form, "token");
        i.begin_edit();
        type_str(i, "old");
        i.field.press(0, 1, false);
        i.field.drag(2);
        assert!(i.field.pressed());
    }

    form.set_text("token", "new");
    let i = input_mut(&mut form, "token");
    assert!(!i.editing, "replacement ends the edit session");
    assert!(!i.field.pressed(), "pointer capture is reset");
    assert!(!i.can_undo(), "history from the old value is gone");
    assert!(!i.is_dirty());
    assert_eq!(i.field.buffer.cursor_offset(), 3, "caret sits at the end");
    assert!(!i.field.buffer.has_selection());

    i.begin_edit();
    type_str(i, "x");
    assert_eq!(i.text(), "newx");
    i.cancel();
    assert_eq!(i.text(), "new", "cancel restores the replacement");
    i.begin_edit();
    i.on_key(&chord(KeyCode::Char('z'), KeyModifiers::CONTROL));
    assert!(!i.text().contains("old"), "undo cannot reach the old value");
    assert!(
        !form.dirty && form.events.is_empty(),
        "no user-change event"
    );
}

#[test]
fn form_value_retains_focus_tracking() {
    let mut form = FormDialog::new(
        FORM,
        "Account",
        vec![FormField::input(
            "token",
            TextInput::new(SECRET, "Token").edit_entry(EditEntry::Immediate),
        )],
    );
    draw_form(&mut form, Some(SECRET));
    assert!(input(&form, "token").editing, "Immediate entry on focus");

    form.set_text("token", "replaced");
    assert!(!input(&form, "token").editing);
    draw_form(&mut form, Some(SECRET));
    assert!(
        !input(&form, "token").editing,
        "same focus is not a new focus edge"
    );
    assert!(form.events.is_empty());

    draw_form(&mut form, None);
    draw_form(&mut form, Some(SECRET));
    assert!(
        input(&form, "token").editing,
        "a real focus transition re-enters editing"
    );
}

fn draw_input(i: &mut TextInput, focus: Option<WidgetId>) {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 4));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let interaction = Interaction {
        focus,
        ..Default::default()
    };
    let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
    i.render(Rect::new(0, 0, 60, 4), &mut buf, &mut ctx, theme.canvas);
}

#[test]
fn form_value_clear_preserves_active_session() {
    let id = WidgetId::of("t.search");
    let mut search = TextInput::new(id, "").edit_entry(EditEntry::Immediate);
    draw_input(&mut search, Some(id));
    assert!(search.editing, "focused Immediate field is editing");
    type_str(&mut search, "abc");

    search.clear();
    assert!(search.editing, "clear keeps the active edit session");
    assert_eq!(search.text(), "");
    assert!(!search.can_undo());
    draw_input(&mut search, Some(id));
    assert!(search.editing, "and the next render keeps it");

    type_str(&mut search, "d");
    assert_eq!(
        search.text(),
        "d",
        "the next character lands without begin_edit"
    );
    search.cancel();
    assert_eq!(search.text(), "", "cancel returns the cleared baseline");
}

#[test]
fn form_value_set_value_builder_and_setter_share_one_path() {
    let built = TextInput::new(SECRET, "Token").masked().value("one\ntwo");
    assert!(built.masked && built.field.secret);
    assert_eq!(built.text(), "onetwo", "single-line text is normalised");

    let mut i = TextInput::new(SECRET, "Token").masked().required(true);
    i.set_error(Some("stale".into()));
    i.set_value("x");
    assert_eq!(i.error(), None);
    assert!(i.masked && i.required);
    assert!(!i.editing && !i.is_dirty());
}

use crate::common::*;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};
use termrock::core::event::Outcome;
use termrock::core::focus::{Focus, FocusRing};
use termrock::core::hints::{HasHints, HintCx};
use termrock::core::hit::HitRegistry;
use termrock::core::id::WidgetId;
use termrock::core::refusal::{Control, Reason, clear_refusal};
use termrock::shell::StatusLine;
use termrock::theme::{ColorLevel, Theme, Tone};
use termrock::ui::ctx::{Interaction, RenderCtx};
use termrock::widgets::button::Button;
use termrock::widgets::choice::{Checkbox, RadioGroup};
use termrock::widgets::choice_dialog::ChoiceDialog;
use termrock::widgets::dialog::{Dialog, DialogResult};
use termrock::widgets::file_browser::{FileBrowser, FileEntry, MemorySource};
use termrock::widgets::form_dialog::{FormDialog, FormField};
use termrock::widgets::input::TextInput;
use termrock::widgets::list::{ListBox, ListItem, SelectMode};
use termrock::widgets::props::Prop;
use termrock::widgets::select::{Select, SelectArrows};

#[test]
fn a_fixed_checkbox_refuses_every_edit() {
    clear_refusal();
    let mut fixed = Checkbox::new(WidgetId::of("t.fixed"), "Keep awake", false).fixed(reason());
    for k in [KeyCode::Char(' '), KeyCode::Enter] {
        assert_eq!(fixed.on_key(&key(k)), Outcome::Consumed);
        assert_eq!(taken().as_deref(), Some(WHY));
    }
    assert_eq!(fixed.on_click(), Outcome::Consumed);
    assert_eq!(taken().as_deref(), Some(WHY));
    assert!(!fixed.checked);
    assert_eq!(fixed.on_key(&key(KeyCode::Tab)), Outcome::Ignored);
    assert_eq!(taken(), None);

    let mut live = Checkbox::new(WidgetId::of("t.live"), "Keep awake", false);
    assert_eq!(live.on_key(&key(KeyCode::Char(' '))), Outcome::Changed);
    assert!(live.checked);
    assert_eq!(live.on_click(), Outcome::Changed);
    assert!(!live.checked);
    assert_eq!(taken(), None);
}

#[test]
fn a_fixed_checkbox_draws_disabled_with_its_mark_and_stays_focusable() {
    let id = WidgetId::of("t.fixed");
    let render = |mut cb: Checkbox, focus: bool| {
        let (buf, ring) = draw(focus.then_some(id), |buf, ctx| {
            cb.render(Rect::new(0, 0, 40, 1), buf, ctx, ctx.theme.canvas)
        });
        (row(&buf, 0), fg_at(&buf, 6, 0), ring.contains(id))
    };
    let base = Checkbox::new(id, "Keep awake", true);
    let (live_row, live_ink, live_ring) = render(base.clone(), false);
    let (off_row, off_ink, off_ring) = render(
        Checkbox {
            disabled: true,
            ..base.clone()
        },
        false,
    );
    let (fixed_row, fixed_ink, fixed_ring) = render(base.clone().fixed(reason()), false);
    let (focused_row, _, _) = render(base.clone().fixed(reason()), true);
    let (off_focused, _, off_focused_ring) = render(
        Checkbox {
            disabled: true,
            ..base
        },
        true,
    );

    assert!(live_row.contains("[✓] Keep awake") && live_ring);
    assert!(fixed_row.contains("[✓] Keep awake"));
    assert_eq!(fixed_ink, off_ink);
    assert_ne!(fixed_ink, live_ink);
    assert!(fixed_ring && !off_ring && !off_focused_ring);
    assert!(focused_row.starts_with('▎'), "{focused_row:?}");
    assert!(!off_focused.starts_with('▎'), "{off_row:?} {off_focused:?}");
}

#[test]
fn a_fixed_select_refuses_open_arrows_typing_and_clicks() {
    clear_refusal();
    let id = WidgetId::of("t.select");
    let mut fixed = Select::new(id, "Dirty exit", &["ask", "stop", "keep"], 0).fixed(reason());
    for k in [
        KeyCode::Enter,
        KeyCode::Char(' '),
        KeyCode::Down,
        KeyCode::Up,
        KeyCode::Char('s'),
    ] {
        let (out, action) = fixed.on_key(&key(k));
        assert_eq!((out, action), (Outcome::Consumed, None), "{k:?}");
        assert_eq!(taken().as_deref(), Some(WHY), "{k:?}");
    }
    assert!(!fixed.open);
    assert_eq!(fixed.value(), "ask");
    assert_eq!(fixed.on_click(id), (Outcome::Consumed, None));
    assert_eq!(taken().as_deref(), Some(WHY));
    assert_eq!(
        fixed.on_press(id, ratatui::layout::Position::new(1, 1)),
        Outcome::Consumed
    );
    assert_eq!(taken().as_deref(), Some(WHY));
    assert!(!fixed.open && fixed.value() == "ask");
    for k in [KeyCode::Tab, KeyCode::Esc, KeyCode::Left, KeyCode::Right] {
        assert_eq!(fixed.on_key(&key(k)).0, Outcome::Ignored, "{k:?}");
        assert_eq!(taken(), None, "{k:?}");
    }

    let mut stepping = Select::new(id, "Dirty exit", &["ask", "stop", "keep"], 0)
        .arrows(SelectArrows::Step)
        .fixed(reason());
    for k in [KeyCode::Left, KeyCode::Right] {
        assert_eq!(stepping.on_key(&key(k)), (Outcome::Consumed, None), "{k:?}");
        assert_eq!(taken().as_deref(), Some(WHY), "{k:?}");
        assert_eq!(stepping.value(), "ask", "{k:?}");
    }

    let mut live = Select::new(id, "Dirty exit", &["ask", "stop", "keep"], 0);
    assert_eq!(live.on_key(&key(KeyCode::Enter)).0, Outcome::Changed);
    assert!(live.open);
    assert_eq!(taken(), None);
}

#[test]
fn a_fixed_select_keeps_its_value_and_shows_the_focus_gutter() {
    let id = WidgetId::of("t.select");
    let render = |mut sel: Select, focus: bool| {
        let (buf, ring) = draw(focus.then_some(id), |buf, ctx| {
            sel.render(Rect::new(0, 0, 40, 3), buf, ctx, ctx.theme.canvas)
        });
        (row(&buf, 1), ring.contains(id))
    };
    let base = Select::new(id, "Dirty exit", &["ask", "stop"], 0);
    let (fixed, in_ring) = render(base.clone().fixed(reason()), true);
    let (off, off_ring) = render(base.disabled(true), true);
    assert!(fixed.starts_with('▎') && fixed.contains("ask"), "{fixed:?}");
    assert!(in_ring && !off_ring);
    assert!(!off.starts_with('▎'), "{off:?}");
}

#[test]
fn a_fixed_radio_option_takes_the_cursor_but_is_never_chosen() {
    clear_refusal();
    let id = WidgetId::of("t.radio");
    let mut group =
        RadioGroup::new(id, "Source", &["Local", "Manager", "Typed"], 0).fixed_option(2, reason());
    assert_eq!(group.on_key(&key(KeyCode::Down)), Outcome::Changed);
    assert_eq!((group.cursor, group.selected), (1, 1));
    assert_eq!(group.on_key(&key(KeyCode::Down)), Outcome::Changed);
    assert_eq!((group.cursor, group.selected), (2, 1));
    assert_eq!(taken(), None);
    assert_eq!(group.on_key(&key(KeyCode::Char(' '))), Outcome::Consumed);
    assert_eq!(taken().as_deref(), Some(WHY));
    assert_eq!(group.on_key(&key(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(taken().as_deref(), Some(WHY));
    group.on_click(0);
    assert_eq!((group.cursor, group.selected), (0, 0));
    assert_eq!(taken(), None);
    group.on_click(2);
    assert_eq!((group.cursor, group.selected), (2, 0));
    assert_eq!(taken().as_deref(), Some(WHY));

    let (buf, ring) = draw(Some(id), |buf, ctx| {
        group.render(Rect::new(0, 0, 40, 4), buf, ctx, ctx.theme.canvas)
    });
    assert!(ring.contains(id));
    assert!(row(&buf, 3).starts_with('▎'), "{:?}", row(&buf, 3));
    assert!(row(&buf, 3).contains("( ) Typed") && row(&buf, 1).contains("(●) Local"));
    assert_ne!(fg_at(&buf, 6, 3), fg_at(&buf, 6, 2));
}

#[test]
fn a_fixed_button_refuses_and_does_not_activate() {
    clear_refusal();
    let mut fixed = Button::secondary(WidgetId::of("t.btn"), "Disable").fixed(reason());
    assert!(!fixed.can_activate());
    assert_eq!(
        fixed.on_key(&key(KeyCode::Enter)),
        (Outcome::Consumed, false)
    );
    assert_eq!(taken().as_deref(), Some(WHY));
    assert_eq!(
        fixed.on_key(&key(KeyCode::Char(' '))),
        (Outcome::Consumed, false)
    );
    assert_eq!(taken().as_deref(), Some(WHY));
    assert!(!fixed.on_click());
    assert_eq!(taken().as_deref(), Some(WHY));

    let mut live = Button::secondary(WidgetId::of("t.btn"), "Disable");
    assert_eq!(live.on_key(&key(KeyCode::Enter)), (Outcome::Changed, true));
    assert!(live.on_click());
    assert_eq!(taken(), None);

    let id = WidgetId::of("t.btn");
    let (buf, ring) = draw(Some(id), |buf, ctx| {
        let mut b = Button::secondary(id, "Disable").fixed(reason());
        b.render(Rect::new(0, 0, 12, 1), buf, ctx, ctx.theme.canvas);
    });
    assert!(ring.contains(id) && row(&buf, 0).starts_with("▎Disable"));
}

#[test]
fn a_fixed_list_row_takes_the_cursor_and_refuses_to_be_ticked() {
    clear_refusal();
    let items = vec![
        ListItem::new("Team policy"),
        ListItem::new("Managed policy").fixed(reason()),
        ListItem::new("Personal policy"),
    ];
    let mut list = ListBox::new(WidgetId::of("t.list"), items, SelectMode::Multi);
    assert_eq!(list.on_key(&key(KeyCode::Down)), Outcome::Changed);
    assert_eq!(list.core.cursor, 1);
    assert_eq!(list.on_key(&key(KeyCode::Char(' '))), Outcome::Consumed);
    assert_eq!(taken().as_deref(), Some(WHY));
    assert!(!list.is_checked(1));
    list.on_click(1);
    assert_eq!(taken().as_deref(), Some(WHY));
    assert!(!list.is_checked(1));
    list.on_key(&key(KeyCode::Char('a')));
    assert!(list.is_checked(0) && !list.is_checked(1) && list.is_checked(2));
    assert_eq!(taken(), None);
    list.on_key(&key(KeyCode::Up));
    assert_eq!(list.on_key(&key(KeyCode::Char(' '))), Outcome::Changed);
    assert!(!list.is_checked(0));
    assert_eq!(taken(), None);
}

#[test]
fn taking_a_refusal_clears_the_slot() {
    clear_refusal();
    assert!(Control::fixed(reason()).refuse());
    assert_eq!(taken().as_deref(), Some(WHY));
    assert_eq!(taken(), None);
    assert!(!Control::Live.refuse());
    assert_eq!(taken(), None);
}

#[test]
fn a_refusal_posts_the_reason_alone_in_a_neutral_tone() {
    clear_refusal();
    let mut host = Host::new(Act::Refuse(reason()));
    host.press(KeyCode::Char('x'));
    assert_eq!(host.status(), Some((WHY.to_owned(), Tone::Secondary)));
}

#[test]
fn a_refusal_status_lasts_five_seconds() {
    use termrock::clock::Moment;
    let mut line = StatusLine::<()>::new();
    line.set_at(WHY, Tone::Secondary, Moment::ZERO);
    assert!(!line.expire(Moment::from_millis(4_999)));
    assert_eq!(line.text(), Some(WHY));
    assert!(line.expire(Moment::from_millis(5_000)));
    assert_eq!(line.text(), None);
}

#[test]
fn a_fixed_row_shows_the_focus_through_the_shared_row_painter() {
    use termrock::theme::CursorStyle;
    use termrock::ui::ctx::VisualState;
    use termrock::ui::row::{RowLook, RowUi};

    let theme = Theme::for_level(ColorLevel::TrueColor);
    let paint = |look: RowLook, state: VisualState| {
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        RowUi::paint(
            &ctx,
            &mut buf,
            Rect::new(0, 0, 20, 1),
            state,
            look,
            theme.canvas,
        );
        (
            buf[(0, 0)].symbol().to_owned(),
            buf[(5, 0)].bg,
            buf[(5, 0)].fg,
        )
    };
    let off = VisualState {
        disabled: true,
        ..Default::default()
    };
    let fixed = VisualState {
        disabled: true,
        fixed: true,
        focused: true,
        ..Default::default()
    };
    let fixed_rest = VisualState {
        focused: false,
        ..fixed
    };
    let fill = RowLook::Fill(CursorStyle::Fill);

    let (gutter, _, _) = paint(RowLook::Bar, fixed);
    assert_eq!(gutter, "▎");
    assert_eq!(paint(RowLook::Bar, off).0, " ");
    assert_eq!(paint(RowLook::Bar, fixed_rest).0, " ");

    let (_, fixed_bg, fixed_ink) = paint(fill, fixed);
    let (_, off_bg, off_ink) = paint(fill, off);
    assert_eq!(fixed_bg, theme.ladders.highlight.rest);
    assert_ne!(off_bg, theme.ladders.highlight.rest);
    assert_eq!(fixed_ink, off_ink);
}

fn confirm_dialog(fixed: bool) -> Dialog {
    let mut dialog = Dialog::confirm(
        WidgetId::of("t.dialog"),
        "Disable all",
        "Every item stays on.",
        "Disable",
    )
    .accelerators(Some(('y', 'n')));
    if fixed {
        dialog.actions[1] = dialog.actions[1].clone().fixed(reason());
    }
    dialog
}

fn route_keys(dialog: &mut Dialog, focused: Option<WidgetId>, k: KeyCode) -> Outcome {
    let mut focus = Focus::default();
    focus.set(focused);
    dialog.on_key(&key(k), &mut focus, &FocusRing::default())
}

#[test]
fn a_dialog_refuses_a_fixed_action_by_every_route() {
    clear_refusal();
    type Route = fn(&mut Dialog) -> Outcome;
    let routes: [(&str, Route); 4] = [
        ("mnemonic d", |d| route_keys(d, None, KeyCode::Char('d'))),
        ("accelerator y", |d| route_keys(d, None, KeyCode::Char('y'))),
        ("Enter on the focused button", |d| {
            route_keys(d, Some(WidgetId::of("t.dialog").sub("ok")), KeyCode::Enter)
        }),
        ("click", |d| {
            d.on_click(
                WidgetId::of("t.dialog").sub("ok"),
                Position::new(0, 0),
                &mut Focus::default(),
            )
        }),
    ];
    for (name, route) in routes {
        let mut live = confirm_dialog(false);
        route(&mut live);
        assert_eq!(live.result, Some(DialogResult::Action(1)), "live {name}");
        assert_eq!(taken(), None, "live {name}");

        let mut fixed = confirm_dialog(true);
        let out = route(&mut fixed);
        assert!(out.consumed(), "{name}");
        assert_eq!(fixed.result, None, "{name}");
        assert_eq!(taken().as_deref(), Some(WHY), "{name}");
    }
}

#[test]
fn a_prompt_refuses_a_fixed_primary_when_the_field_commits() {
    clear_refusal();
    let field = WidgetId::of("t.prompt").sub("name");
    let build = |fixed: bool| {
        let mut dialog = Dialog::prompt(
            WidgetId::of("t.prompt"),
            "Rename",
            TextInput::new(field, "Name").value("abc"),
            "Rename",
        );
        if fixed {
            dialog.actions[1] = dialog.actions[1].clone().fixed(reason());
        }
        dialog
    };
    for (fixed, want) in [(false, Some(DialogResult::Action(1))), (true, None)] {
        let mut dialog = build(fixed);
        route_keys(&mut dialog, Some(field), KeyCode::Char('x'));
        let out = route_keys(&mut dialog, Some(field), KeyCode::Enter);
        assert_eq!(
            out,
            if fixed {
                Outcome::Consumed
            } else {
                Outcome::Changed
            },
            "fixed {fixed}"
        );
        assert_eq!(dialog.result, want, "fixed {fixed}");
        assert_eq!(taken().as_deref(), fixed.then_some(WHY), "fixed {fixed}");
    }
}

#[test]
fn a_typed_acknowledgement_refuses_a_fixed_gated_action() {
    clear_refusal();
    let id = WidgetId::of("t.facts");
    let ack = id.sub("ack");
    let build = |fixed: bool| {
        let mut go = Button::danger(id.sub("go"), "Delete");
        if fixed {
            go = go.fixed(reason());
        }
        Dialog::facts(
            id,
            "Delete",
            vec![Prop::new("Target", "box")],
            vec![],
            Some("DEL"),
            go,
        )
    };
    for (fixed, want) in [(false, Some(DialogResult::Action(1))), (true, None)] {
        let mut dialog = build(fixed);
        for c in ['D', 'E', 'L'] {
            route_keys(&mut dialog, Some(ack), KeyCode::Char(c));
        }
        let out = route_keys(&mut dialog, Some(ack), KeyCode::Enter);
        assert_eq!(
            out,
            if fixed {
                Outcome::Consumed
            } else {
                Outcome::Changed
            },
            "fixed {fixed}"
        );
        assert_eq!(dialog.result, want, "fixed {fixed}");
        assert_eq!(taken().as_deref(), fixed.then_some(WHY), "fixed {fixed}");
    }
}

#[test]
fn a_dialog_does_not_offer_a_fixed_action() {
    let offers = |dialog: &Dialog| {
        let ok = dialog.actions[1].id;
        dialog
            .hints(Some(ok))
            .iter()
            .filter(|h| h.action == "Disable")
            .map(|h| h.key.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(offers(&confirm_dialog(false)), ["Enter", "y"]);
    assert_eq!(offers(&confirm_dialog(true)), Vec::<&str>::new());
}

fn trio(middle: ListItem) -> ListBox {
    ListBox::new(
        WidgetId::of("t.trio"),
        vec![ListItem::new("A"), middle, ListItem::new("C")],
        SelectMode::Multi,
    )
}

fn fixed_middle() -> ListItem {
    ListItem::new("B").fixed(reason())
}

fn ticks(list: &ListBox) -> [bool; 3] {
    [0, 1, 2].map(|i| list.is_checked(i))
}

fn shift(code: KeyCode) -> termrock::core::event::Key {
    chord(code, KeyModifiers::SHIFT)
}

#[test]
fn a_checked_fixed_row_keeps_its_tick_when_another_row_is_clicked() {
    clear_refusal();
    for target in [0, 2] {
        let mut list = trio(fixed_middle());
        list.set_checked(1, true);
        list.on_click(target);
        let mut want = [false, true, false];
        want[target] = true;
        assert_eq!(ticks(&list), want, "click {target}");
    }
    assert_eq!(taken(), None);
}

#[test]
fn a_checked_fixed_row_keeps_its_tick_through_toggle_all() {
    clear_refusal();
    let mut list = trio(fixed_middle());
    list.set_checked(1, true);
    list.on_key(&key(KeyCode::Char('a')));
    assert_eq!(ticks(&list), [true, true, true]);
    list.on_key(&key(KeyCode::Char('a')));
    assert_eq!(ticks(&list), [false, true, false]);
    assert_eq!(taken(), None);
}

#[test]
fn an_unchecked_fixed_row_does_not_block_deselecting_all() {
    clear_refusal();
    let mut list = trio(fixed_middle());
    list.on_key(&key(KeyCode::Char('a')));
    assert_eq!(ticks(&list), [true, false, true]);
    list.on_key(&key(KeyCode::Char('a')));
    assert_eq!(ticks(&list), [false, false, false]);
    assert_eq!(taken(), None);
}

#[test]
fn a_checked_fixed_row_keeps_its_tick_when_a_range_starts_on_it() {
    clear_refusal();
    let mut list = trio(fixed_middle());
    list.on_key(&key(KeyCode::Down));
    list.set_checked(1, true);
    list.on_key(&shift(KeyCode::Down));
    assert_eq!(ticks(&list), [false, true, true]);
    assert_eq!(taken(), None);
}

#[test]
fn an_unchecked_fixed_row_is_not_ticked_by_a_range() {
    clear_refusal();
    let mut list = trio(fixed_middle());
    list.on_key(&shift(KeyCode::Down));
    list.on_key(&shift(KeyCode::Down));
    assert_eq!(ticks(&list), [true, false, true]);
    list.on_key(&shift(KeyCode::Up));
    assert_eq!(ticks(&list), [true, false, false]);
    assert_eq!(taken(), None);
}

#[test]
fn an_unchecked_fixed_row_is_not_ticked_by_select_all() {
    clear_refusal();
    for select_all in [
        key(KeyCode::Char('a')),
        chord(KeyCode::Char('a'), KeyModifiers::CONTROL),
    ] {
        let mut list = trio(fixed_middle());
        list.on_key(&select_all);
        assert_eq!(ticks(&list), [true, false, true], "{select_all:?}");
    }
    assert_eq!(taken(), None);
}

#[test]
fn a_checked_disabled_row_keeps_its_tick_when_another_row_is_clicked() {
    clear_refusal();
    let mut list = trio(ListItem::new("B").disabled(true));
    list.set_checked(1, true);
    list.on_click(0);
    assert_eq!(ticks(&list), [true, true, false]);
}

#[test]
fn a_later_refusal_replaces_an_earlier_one_in_the_slot() {
    clear_refusal();
    Control::fixed(Reason::new("first")).refuse();
    Control::fixed(Reason::new("second")).refuse();
    assert_eq!(taken().as_deref(), Some("second"));
    assert_eq!(taken(), None);
}

#[test]
fn the_slot_belongs_to_its_thread() {
    clear_refusal();
    Control::fixed(reason()).refuse();
    let elsewhere = std::thread::spawn(|| {
        let seen = taken();
        Control::fixed(Reason::new("elsewhere")).refuse();
        seen
    })
    .join()
    .expect("thread");
    assert_eq!(elsewhere, None);
    assert_eq!(taken().as_deref(), Some(WHY));
}

fn browser(fixed: bool) -> FileBrowser {
    let mut browser = FileBrowser::new(
        WidgetId::of("t.browser"),
        "Choose",
        MemorySource::new(vec![FileEntry::dir("/src/app")]).home("/src"),
        "/src",
    )
    .checkbox("Read-only", false);
    if fixed {
        browser.checkbox = browser.checkbox.take().map(|c| c.fixed(reason()));
    }
    browser
}

fn browser_rows(browser: &mut FileBrowser) -> Vec<String> {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let area = Rect::new(0, 0, 100, 30);
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    browser.render(area, &mut buf, &mut ctx);
    (0..area.height).map(|y| row(&buf, y)).collect()
}

#[test]
fn a_fixed_checkbox_in_the_file_browser_shows_its_reason_and_refuses() {
    clear_refusal();
    let line = |rows: &[String]| {
        rows.iter()
            .find(|r| r.contains("Read-only"))
            .cloned()
            .unwrap_or_default()
    };
    let live = line(&browser_rows(&mut browser(false)));
    assert!(
        live.contains("[ ] Read-only") && !live.contains(WHY),
        "{live}"
    );

    let mut fixed = browser(true);
    let shown = line(&browser_rows(&mut fixed));
    assert!(
        shown.contains("[ ] Read-only") && shown.contains(WHY),
        "{shown}"
    );

    let id = fixed.checkbox.as_ref().expect("a checkbox").id;
    let mut focus = Focus::default();
    focus.set(Some(id));
    for code in [KeyCode::Char(' '), KeyCode::Enter] {
        let out = fixed.on_key(&key(code), &mut focus, &FocusRing::default());
        assert!(out.consumed(), "{code:?}");
        assert_eq!(taken().as_deref(), Some(WHY), "{code:?}");
        assert!(!fixed.checkbox.as_ref().expect("a checkbox").checked);
    }
}

fn choice(disabled: bool) -> ChoiceDialog {
    let mut dialog = ChoiceDialog::new(
        WidgetId::of("t.choice"),
        "Destination",
        "Where",
        &["Project folder", "Edit destination…"],
        0,
    );
    dialog.radio.disabled = disabled;
    dialog
}

#[test]
fn a_choice_dialog_with_a_disabled_radio_starts_on_its_primary_button() {
    let live = choice(false);
    assert_eq!(live.initial_focus(), live.radio.id);
    let disabled = choice(true);
    assert_eq!(disabled.initial_focus(), disabled.buttons[1].id);
    assert_ne!(disabled.initial_focus(), disabled.buttons[0].id);
}

#[test]
fn a_click_on_a_disabled_choice_radio_keeps_the_focus_where_it_is() {
    let mut dialog = choice(true);
    let next = dialog.buttons[1].id;
    let mut focus = Focus::default();
    focus.set(Some(next));
    dialog.on_click(dialog.radio.option_id(1), &mut focus);
    assert_eq!(focus.current(), Some(next));
    assert_eq!(dialog.radio.selected, 0);
    assert_eq!(dialog.result, None);
}

#[test]
fn a_form_does_not_offer_a_fixed_action() {
    let id = WidgetId::of("t.form");
    let form = FormDialog::new(
        id,
        "Account",
        vec![FormField::input(
            "name",
            TextInput::new(id.sub("name"), "Name"),
        )],
    )
    .action(
        "plain",
        Button::subtle(id.sub("plain"), "Plain text").fixed(reason()),
    )
    .action("check", Button::subtle(id.sub("check"), "Check"));
    let offers = |button: &str| {
        form.hints(&HintCx::new(Some(id.sub(button))))
            .iter()
            .any(|h| {
                h.action
                    == if button == "plain" {
                        "Plain text"
                    } else {
                        "Check"
                    }
            })
    };
    assert!(!offers("plain"));
    assert!(offers("check"));
}

#[test]
fn a_focused_menu_item_with_a_reason_keeps_the_cursor_fill() {
    use termrock::ui::popup::Placement;
    use termrock::widgets::menu::{ContextMenu, MenuItem};

    let theme = Theme::for_level(ColorLevel::TrueColor);
    let cell = |items: Vec<MenuItem>, focus: usize, label: &str| {
        let items_len = items.len();
        let mut menu = ContextMenu::new(WidgetId::of("t.menu"), items)
            .skip_disabled(false)
            .anchor(Rect::new(2, 1, 1, 1), Placement::Below);
        for _ in 0..items_len {
            match menu.highlighted() {
                Some(at) if at > focus => menu.on_key(&key(KeyCode::Up)),
                Some(at) if at < focus => menu.on_key(&key(KeyCode::Down)),
                _ => break,
            };
        }
        assert_eq!(menu.highlighted(), Some(focus));
        let area = Rect::new(0, 0, 60, 12);
        let mut buf = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        {
            let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
            menu.render(area, &mut buf, &mut ctx);
            ctx.flush_overlays(&mut buf);
        }
        (0..area.height)
            .find_map(|y| {
                let cells: Vec<char> = row(&buf, y).chars().collect();
                let at = cells
                    .windows(label.chars().count())
                    .position(|w| w.iter().collect::<String>() == label)?;
                Some((buf[(at as u16, y)].bg, buf[(at as u16, y)].fg))
            })
            .unwrap_or_else(|| panic!("no `{label}` row"))
    };
    let items = || {
        vec![
            MenuItem::new("Reasoned").reason("no disabled state"),
            MenuItem::new("Plain").disabled(true),
            MenuItem::new("Live"),
        ]
    };
    let fill = theme.ladders.highlight.rest;

    let (bg, fg) = cell(items(), 0, "Reasoned");
    assert_eq!(bg, fill, "the focused reasoned item keeps the cursor fill");
    assert_eq!(fg, theme.legible(theme.text_muted, fill));
    assert_ne!(fg, theme.disabled);

    let (bg, fg) = cell(items(), 1, "Reasoned");
    assert_ne!(bg, fill, "an unfocused reasoned item has no fill");
    assert_eq!(fg, theme.disabled);

    let (bg, fg) = cell(items(), 1, "Plain");
    assert_ne!(bg, fill, "an unreasoned disabled item is unchanged");
    assert_eq!(fg, theme.disabled);

    let (bg, _) = cell(items(), 2, "Live");
    assert_eq!(bg, fill, "a live item keeps its fill");
}

#[test]
fn a_notice_with_a_payload_waits_for_the_clock_and_yields_to_a_message() {
    use termrock::clock::Moment;
    let mut line = StatusLine::<u8>::new();
    line.notice_with_at("refresh failed", Tone::Secondary, 3, Moment::ZERO);
    assert_eq!(line.payload(), Some(&3));
    assert!(!line.expire(Moment::from_millis(1)));
    line.set_with("saved", Tone::Secondary, 5);
    line.notice_with("refresh failed again", Tone::Secondary, 9);
    assert_eq!(line.text(), Some("saved"), "a notice never hides a message");
    assert_eq!(line.payload(), Some(&5));
}

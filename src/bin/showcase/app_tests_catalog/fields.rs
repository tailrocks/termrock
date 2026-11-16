//! Lane `fields` of the #14 burn-down: this lane's callable scenarios.
//!
//! Each scenario builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. Register one `ScenarioCase` per
//! `ScenarioId` variant from the `lane:fields` block in `../coverage.rs`. Edit only
//! this file, `../coverage/lane_fields.rs` and `../coverage_debt/fields.txt`.

use super::*;
use junie_tui::core::edit::{EditEntry, FocusSelect, UndoStack};
use junie_tui::core::edit::{EditKind, clipboard};
use junie_tui::core::focus::BlurAction;
use junie_tui::core::keymap::KeyBinding;
use junie_tui::core::keymap::NavKeys;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::core::refusal::Control;
use junie_tui::core::scroll::HScroll;
use junie_tui::core::text::TextBuffer;
use junie_tui::theme::ButtonKind;
use junie_tui::ui::ctx::FocusVisible;
use junie_tui::ui::tooltip::Tip;
use junie_tui::widgets::action_row::ActionRow;
use junie_tui::widgets::button::{
    Button, ButtonRow, MORE_LABEL, RowAlign, RowOverflow, row_layout, row_layout_right, row_lines,
};
use junie_tui::widgets::choice::{CheckHit, RadioGroup, Toggle};
use junie_tui::widgets::code::{CodeCmd, CodeEditor as Editor, EditorAction};
use junie_tui::widgets::edit::{
    EditExit, EditResult, Editable, FieldError, LineLook, Validate, ValidateFn, ValidateWhen,
};
use junie_tui::widgets::field_common::{
    Applied, EditAction, PointerField, PointerSelect, apply_edit, edit_key, edit_key_in,
    edit_key_with, paste_clipboard, route_click, route_drag, route_press, text_cmd_action,
    unbound_key,
};
use junie_tui::widgets::icon_button::{IconButton, IconLook};
use junie_tui::widgets::input::{InputAction, ValidateOn};
use junie_tui::widgets::keyhint::EDIT_FIELD;
use junie_tui::widgets::textarea::TextArea;
use junie_tui::widgets::widget_hints::EDIT as EDIT_HINT;
use ratatui::style::Style;
use std::rc::Rc;

#[rustfmt::skip]
pub const CASES: &[ScenarioCase] = &[
    ScenarioCase { id: ScenarioId::FieldsButtonControls, run: fields_button_controls },
    ScenarioCase { id: ScenarioId::FieldsButtonRowLayout, run: fields_button_row_layout },
    ScenarioCase { id: ScenarioId::FieldsActionRow, run: fields_action_row },
    ScenarioCase { id: ScenarioId::FieldsChoiceControls, run: fields_choice_controls },
    ScenarioCase { id: ScenarioId::FieldsIconButton, run: fields_icon_button },
    ScenarioCase { id: ScenarioId::FieldsEditFieldCore, run: fields_edit_field_core },
    ScenarioCase { id: ScenarioId::FieldsValidators, run: fields_validators },
    ScenarioCase { id: ScenarioId::FieldsFieldCommon, run: fields_field_common },
    ScenarioCase { id: ScenarioId::FieldsPointerRouting, run: fields_pointer_routing },
    ScenarioCase { id: ScenarioId::FieldsTextInputBuilders, run: fields_text_input_builders },
    ScenarioCase { id: ScenarioId::FieldsTextInputEditing, run: fields_text_input_editing },
    ScenarioCase { id: ScenarioId::FieldsTextInputPointer, run: fields_text_input_pointer },
    ScenarioCase { id: ScenarioId::FieldsTextAreaBuilders, run: fields_text_area_builders },
    ScenarioCase { id: ScenarioId::FieldsTextAreaEditing, run: fields_text_area_editing },
    ScenarioCase { id: ScenarioId::FieldsCodeEditorKeys, run: fields_code_editor_keys },
    ScenarioCase { id: ScenarioId::FieldsCodeEditorFind, run: fields_code_editor_find },
    ScenarioCase { id: ScenarioId::FieldsCodeEditorConfig, run: fields_code_editor_config },
];

/// One painted frame with the hit regions and the focus ring it filled.
struct Ringed {
    buf: Buffer,
    hits: HitRegistry,
    ring: FocusRing,
}

fn paint_ring(
    area: Rect,
    interaction: Interaction,
    draw: impl FnOnce(Rect, &mut Buffer, &mut RenderCtx),
) -> Ringed {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        draw(area, &mut buf, &mut ctx);
        ctx.flush_overlays(&mut buf);
    }
    Ringed { buf, hits, ring }
}

fn id(name: &str) -> WidgetId {
    WidgetId::of(name)
}

fn row_text(buf: &Buffer, y: u16) -> String {
    rows(buf)[usize::from(y)].clone()
}

fn fields_button_controls() {
    let save = Button::primary(id("fields.save"), "Save");
    let cancel = Button::secondary(id("fields.cancel"), "Cancel");
    let more = Button::subtle(id("fields.more"), "More");
    let delete = Button::danger(id("fields.delete"), "Delete");
    let custom = Button::new(id("fields.custom"), "Custom", ButtonKind::Secondary);
    let bold = Button::toggle(id("fields.bold"), "Bold", false);
    assert_eq!(save.kind, ButtonKind::Primary);
    assert_eq!(cancel.kind, ButtonKind::Secondary);
    assert_eq!(more.kind, ButtonKind::Subtle);
    assert_eq!(delete.kind, ButtonKind::Danger);
    assert_eq!(custom.kind, ButtonKind::Secondary);
    assert_eq!(bold.kind, ButtonKind::Toggle);
    assert_eq!(bold.on, Some(false));
    assert_eq!(
        save.width(),
        6,
        "label width plus one cell of padding a side"
    );
    assert_eq!(bold.width(), 8, "a toggle adds its marker and a space");

    // Each kind paints its own fill behind the label.
    let mut kinds = [save.clone(), cancel.clone(), more.clone(), delete.clone()];
    let painted = paint_ring(
        Rect::new(0, 0, 40, 4),
        Interaction::default(),
        |_, buf, ctx| {
            for (y, button) in kinds.iter_mut().enumerate() {
                button.render(Rect::new(0, y as u16, 12, 1), buf, ctx, ctx.theme.canvas);
            }
        },
    );
    for (y, label) in ["Save", "Cancel", "More", "Delete"].iter().enumerate() {
        assert!(row_text(&painted.buf, y as u16).contains(label));
    }
    let fill = |y: u16| painted.buf[(2, y)].style();
    assert_ne!(fill(0), fill(1), "primary and secondary differ");
    assert_ne!(fill(0), fill(3), "primary and danger differ");
    assert_ne!(fill(1), fill(3), "secondary and danger differ");
    assert_eq!(
        kinds[0].area,
        Rect::new(0, 0, 6, 1),
        "render records the area"
    );

    // Enter and Space activate, other keys fall through.
    let mut live = save.clone();
    assert!(live.can_activate());
    assert_eq!(live.on_key(&key(KeyCode::Enter)), (Outcome::Changed, true));
    assert_eq!(
        live.on_key(&key(KeyCode::Char(' '))),
        (Outcome::Changed, true)
    );
    assert_eq!(
        live.on_key(&key(KeyCode::Char('x'))),
        (Outcome::Ignored, false)
    );
    assert!(live.on_click());
    assert_eq!(live.activate(), (Outcome::Changed, true));
    assert_eq!(live.hint().key, "Enter");
    assert_eq!(live.hint().action, "Save");
    assert!(!live.refuse(), "a live button does not refuse");

    // A toggle flips on each activation.
    let mut flag = bold.clone();
    assert_eq!(flag.activate(), (Outcome::Changed, true));
    assert_eq!(flag.on, Some(true));
    assert!(flag.on_click());
    assert_eq!(flag.on, Some(false));

    // Disabled buttons consume input and do nothing.
    let mut off = Button::secondary(id("fields.off"), "Off").disabled(true);
    assert!(!off.can_activate());
    assert_eq!(off.on_key(&key(KeyCode::Enter)), (Outcome::Consumed, false));
    assert!(!off.on_click());
    assert!(!off.refuse(), "a disabled button is not a refusal");

    // A fixed control refuses with its reason and stays unactivated.
    clear_refusal();
    let mut fixed = Button::secondary(id("fields.fixed"), "Deploy")
        .control(Control::fixed(Reason::new("Needs a signed build")));
    assert!(!fixed.can_activate());
    assert!(fixed.refuse());
    assert_eq!(
        take_refusal().map(|r| r.text),
        Some("Needs a signed build".to_owned())
    );
    assert_eq!(fixed.activate(), (Outcome::Consumed, false));
    assert!(take_refusal().is_some(), "activating raises the refusal");
    assert!(!fixed.on_click());

    // Tooltip text and shortcut survive each other in either order.
    let tipped = Button::primary(id("fields.tip"), "Run")
        .tooltip("Run the task")
        .shortcut("Ctrl+R");
    assert_eq!(
        tipped.tip,
        Some(Tip::new("Run the task").shortcut("Ctrl+R"))
    );
    let retipped = tipped.clone().tooltip("Run it");
    assert_eq!(retipped.tip, Some(Tip::new("Run it").shortcut("Ctrl+R")));
    let only_key = Button::primary(id("fields.key"), "Run").shortcut("Ctrl+R");
    assert_eq!(only_key.tip, Some(Tip::new("").shortcut("Ctrl+R")));

    // Focus visibility and focusability change what the frame registers.
    let hidden =
        Button::primary(id("fields.focus"), "Focus").focus_visible(FocusVisible::KeyboardOnly);
    assert_eq!(hidden.focus_visible, FocusVisible::KeyboardOnly);
    assert_eq!(save.focus_visible, FocusVisible::Always);
    let mut stop = Button::primary(id("fields.stop"), "Stop");
    let mut skip = Button::primary(id("fields.skip"), "Skip").focusable(false);
    assert!(stop.focusable && !skip.focusable);
    let frame = paint_ring(
        Rect::new(0, 0, 20, 2),
        Interaction::default(),
        |_, buf, ctx| {
            stop.render(Rect::new(0, 0, 10, 1), buf, ctx, ctx.theme.canvas);
            skip.render(Rect::new(0, 1, 10, 1), buf, ctx, ctx.theme.canvas);
        },
    );
    assert!(frame.ring.contains(id("fields.stop")));
    assert!(
        !frame.ring.contains(id("fields.skip")),
        "a non-focusable button stays out of the ring"
    );
    assert!(
        frame
            .hits
            .regions()
            .iter()
            .any(|r| r.id == id("fields.skip")),
        "but it is still clickable"
    );

    // The focused button shows the gutter, a busy-less disabled one does not.
    let mut focused = Button::primary(id("fields.focused"), "Go");
    let lit = paint_ring(
        Rect::new(0, 0, 10, 1),
        Interaction {
            focus: Some(id("fields.focused")),
            ..Interaction::default()
        },
        |_, buf, ctx| focused.render(Rect::new(0, 0, 6, 1), buf, ctx, ctx.theme.canvas),
    );
    assert!(row_text(&lit.buf, 0).starts_with('▎'));
}

fn fields_button_row_layout() {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let buttons = vec![
        Button::secondary(id("row.a"), "Alpha"),
        Button::secondary(id("row.b"), "Beta"),
        Button::primary(id("row.c"), "Gamma"),
    ];
    let widths: Vec<u16> = buttons.iter().map(Button::width).collect();
    assert_eq!(widths, [7, 6, 7]);

    // The free row helpers lay widths out left, right and wrapped.
    let area = Rect::new(0, 0, 30, 3);
    let left = row_layout(area, &widths, 2);
    assert_eq!(left[0], Rect::new(0, 0, 7, 1));
    assert_eq!(left[1], Rect::new(9, 0, 6, 1));
    let right = row_layout_right(area, &widths, 2);
    assert_eq!(right[2].right(), 30, "end alignment hugs the right edge");
    assert!(right[0].x > left[0].x);
    assert_eq!(row_lines(30, &widths, 2), 1);
    assert_eq!(
        row_lines(10, &widths, 2),
        3,
        "a narrow width wraps every button"
    );
    assert_eq!(row_lines(30, &[], 2), 1, "an empty row still takes a line");
    let wrapped = row_layout(Rect::new(0, 0, 10, 3), &widths, 2);
    assert_eq!(wrapped[1].y, 1);
    assert_eq!(wrapped[2].y, 2);

    // ButtonRow: alignment, gap and the overflow policy.
    let start = ButtonRow::new();
    assert_eq!(start.align, RowAlign::Start);
    assert_eq!(start.overflow, RowOverflow::More);
    assert_eq!(start.gap, None);
    let natural = start.layout(area, &buttons, &theme);
    assert!(natural.more.is_none() && natural.hidden.is_empty());
    let gap = theme.spacing.button_gap;
    assert_eq!(natural.rects[1].unwrap().x, 7 + gap);
    assert_eq!(start.width(&buttons, &theme), 20 + 2 * gap);

    let end = ButtonRow::new().align(RowAlign::End).gap(1);
    assert_eq!(end.align, RowAlign::End);
    assert_eq!(end.gap, Some(1));
    let at_end = end.layout(area, &buttons, &theme);
    assert_eq!(at_end.rects[2].unwrap().right(), 30);
    assert_eq!(
        at_end.rects[1].unwrap().right() + 1,
        at_end.rects[2].unwrap().x
    );
    assert_eq!(end.width(&buttons, &theme), 22);

    // Too narrow for all buttons: the primary stays and the rest go behind "…".
    let narrow = Rect::new(0, 0, 14, 3);
    let more = ButtonRow::new().gap(1).layout(narrow, &buttons, &theme);
    assert!(!more.hidden.is_empty(), "something is hidden");
    assert!(more.rects[2].is_some(), "the primary button stays visible");
    assert!(more.more.is_some(), "a more button takes the freed slot");
    assert_eq!(MORE_LABEL, "…");
    assert_eq!(
        more.hidden,
        [0, 1]
            .iter()
            .copied()
            .filter(|i| more.rects[*i].is_none())
            .collect::<Vec<_>>()
    );
    assert_eq!(ButtonRow::new().rows(14, &buttons, &theme), 1);

    // The wrap overflow keeps every button and grows rows instead.
    let wrap = ButtonRow::new().gap(1).overflow(RowOverflow::Wrap);
    assert_eq!(wrap.overflow, RowOverflow::Wrap);
    let wrapped = wrap.layout(narrow, &buttons, &theme);
    assert!(wrapped.hidden.is_empty() && wrapped.more.is_none());
    assert!(wrapped.rects.iter().all(Option::is_some));
    assert_eq!(wrap.rows(14, &buttons, &theme), 2);
    assert_eq!(wrap.rows_at_gap(14, &buttons, 5), 2);
    assert_eq!(ButtonRow::new().rows_at_gap(14, &buttons, 5), 1);
    assert_eq!(
        ButtonRow::new()
            .overflow(RowOverflow::Wrap)
            .rows_at_gap(14, &buttons, 5),
        3,
        "a wider fallback gap needs more rows"
    );

    // Focus stepping skips disabled buttons and only wraps when asked.
    let mut stepped = buttons.clone();
    stepped[1] = stepped[1].clone().disabled(true);
    let flat = ButtonRow::new();
    assert_eq!(
        flat.step(&stepped, Some(id("row.a")), true),
        Some(id("row.c"))
    );
    assert_eq!(
        flat.step(&stepped, Some(id("row.c")), false),
        Some(id("row.a"))
    );
    assert_eq!(flat.step(&stepped, Some(id("row.c")), true), None);
    assert_eq!(flat.step(&stepped, Some(id("missing")), true), None);
    let looping = ButtonRow::new().wrap(true);
    assert!(looping.wrap);
    assert_eq!(
        looping.step(&stepped, Some(id("row.c")), true),
        Some(id("row.a"))
    );

    // Rendering paints the buttons and registers the overflow button.
    let owner = id("row.owner");
    assert_eq!(ButtonRow::more_id(owner), owner.sub("more"));
    let mut shown = buttons.clone();
    let mut layout = None;
    let frame = paint_ring(
        Rect::new(0, 0, 14, 1),
        Interaction::default(),
        |a, buf, ctx| {
            layout = Some(ButtonRow::new().gap(1).render(
                owner,
                a,
                &mut shown,
                buf,
                ctx,
                ctx.theme.canvas,
            ));
        },
    );
    let layout = layout.unwrap();
    assert!(layout.more.is_some());
    assert!(row_text(&frame.buf, 0).contains("Gamma"));
    assert!(row_text(&frame.buf, 0).contains(MORE_LABEL));
    assert!(
        frame
            .hits
            .regions()
            .iter()
            .any(|r| r.id == ButtonRow::more_id(owner))
    );
    assert_eq!(shown[0].area, Rect::ZERO, "a hidden button has no area");
    assert_eq!(
        layout.hidden.len(),
        2 - usize::from(layout.rects[1].is_some())
    );
}

fn fields_action_row() {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let ok = Button::primary(id("action.ok"), "OK");
    let cancel = Button::secondary(id("action.cancel"), "Cancel");
    let buttons = [&ok, &cancel];

    let plain = ActionRow::new();
    assert_eq!(plain.align, RowAlign::Start);
    assert_eq!(plain.gap, 2);
    assert!(plain.message.is_none() && !plain.wrap);
    assert_eq!(plain.width(&buttons), 4 + 8 + 2);

    let with_message = ActionRow::new().message("Saved", Tone::Success).gap(1);
    assert_eq!(with_message.gap, 1);
    assert_eq!(
        with_message.message,
        Some(("Saved".to_owned(), Tone::Success))
    );
    assert_eq!(with_message.width(&buttons), 4 + 8 + 1 + 1 + 5);

    let area = Rect::new(0, 0, 40, 1);
    assert_eq!(plain.layout(area, &buttons)[1], Rect::new(6, 0, 8, 1));
    let end = ActionRow::new().align(RowAlign::End);
    assert_eq!(end.align, RowAlign::End);
    assert_eq!(end.layout(area, &buttons)[1].right(), 40);

    // Rendering draws the buttons and puts the message after them.
    let mut ok_m = ok.clone();
    let mut cancel_m = cancel.clone();
    let mut rects = vec![];
    let frame = paint_ring(area, Interaction::default(), |a, buf, ctx| {
        rects = with_message.render(
            a,
            buf,
            ctx,
            ctx.theme.canvas,
            &mut [&mut ok_m, &mut cancel_m],
        );
    });
    let line = row_text(&frame.buf, 0);
    assert!(line.contains("OK") && line.contains("Cancel"));
    assert!(line.find("Saved").unwrap() > line.find("Cancel").unwrap());
    assert_eq!(rects.len(), 2);
    assert!(frame.ring.contains(id("action.ok")));
    let tone_fg = frame.buf[(u16::try_from(line.find("Saved").unwrap()).unwrap(), 0)]
        .style()
        .fg;
    assert_eq!(tone_fg, Some(theme.tone_on(Tone::Success, theme.canvas)));

    // End alignment puts the message on the left of the buttons.
    let mut ok_e = ok.clone();
    let mut cancel_e = cancel.clone();
    let end_msg = ActionRow::new()
        .align(RowAlign::End)
        .message("Saved", Tone::Success);
    let frame = paint_ring(area, Interaction::default(), |a, buf, ctx| {
        end_msg.render(
            a,
            buf,
            ctx,
            ctx.theme.canvas,
            &mut [&mut ok_e, &mut cancel_e],
        );
    });
    let line = row_text(&frame.buf, 0);
    assert!(line.find("Saved").unwrap() < line.find("OK").unwrap());

    // Stepping follows the buttons and honours `wrap`.
    assert_eq!(
        plain.step(&buttons, Some(id("action.ok")), true),
        Some(id("action.cancel"))
    );
    assert_eq!(plain.step(&buttons, Some(id("action.cancel")), true), None);
    let wrapping = ActionRow::new().wrap(true);
    assert!(wrapping.wrap);
    assert_eq!(
        wrapping.step(&buttons, Some(id("action.cancel")), true),
        Some(id("action.ok"))
    );
}

fn fields_choice_controls() {
    // Checkbox: three states, the control policy, the hit target and width.
    let mut check = Checkbox::new(id("choice.check"), "Remember me", false);
    assert_eq!(check.width(), 17);
    assert_eq!(check.on_key(&key(KeyCode::Char(' '))), Outcome::Changed);
    assert!(check.checked);
    let mut mixed = Checkbox::new(id("choice.mixed"), "Select all", false).mixed(true);
    assert!(mixed.mixed);
    assert_eq!(mixed.on_click(), Outcome::Changed);
    assert!(
        mixed.checked && !mixed.mixed,
        "a mixed box resolves to checked"
    );
    clear_refusal();
    let mut locked = Checkbox::new(id("choice.locked"), "Locked", true)
        .control(Control::fixed(Reason::new("Managed by policy")));
    assert_eq!(locked.on_key(&key(KeyCode::Enter)), Outcome::Consumed);
    assert!(locked.checked, "a fixed checkbox keeps its value");
    assert_eq!(
        take_refusal().map(|r| r.text),
        Some("Managed by policy".to_owned())
    );

    let mut label_hit = Checkbox::new(id("choice.label"), "Wide", false);
    let mut row_hit = Checkbox::new(id("choice.row"), "Wide", false).hit(CheckHit::Row);
    assert_eq!(label_hit.hit, CheckHit::Label);
    assert_eq!(row_hit.hit, CheckHit::Row);
    let frame = paint_ring(
        Rect::new(0, 0, 30, 2),
        Interaction::default(),
        |_, buf, ctx| {
            label_hit.render(Rect::new(0, 0, 30, 1), buf, ctx, ctx.theme.canvas);
            row_hit.render(Rect::new(0, 1, 30, 1), buf, ctx, ctx.theme.canvas);
        },
    );
    let region = |name: &str| {
        frame
            .hits
            .regions()
            .iter()
            .find(|r| r.id == id(name))
            .map(|r| r.area)
            .unwrap()
    };
    assert_eq!(
        region("choice.label").width,
        10,
        "label hits cover the box and the text"
    );
    assert_eq!(
        region("choice.row").width,
        30,
        "row hits cover the whole row"
    );
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut mixed_paint = Checkbox::new(id("choice.paint"), "Some", false).mixed(true);
    let frame = paint_ring(
        Rect::new(0, 0, 20, 1),
        Interaction::default(),
        |_, buf, ctx| {
            mixed_paint.render(Rect::new(0, 0, 20, 1), buf, ctx, ctx.theme.canvas);
        },
    );
    assert!(row_text(&frame.buf, 0).contains(theme.glyphs.check_mixed));
    assert_eq!(mixed_paint.area, Rect::new(0, 0, 20, 1));

    // RadioGroup: arrows move and select, fixed options are skipped over.
    clear_refusal();
    let mut radio = RadioGroup::new(id("choice.radio"), "Plan", &["Free", "Team", "Pro"], 0)
        .fixed_option(2, Reason::new("Contact sales"));
    assert_eq!(radio.height(), 4);
    assert_eq!(radio.width(), 10, "the widest option plus its marker");
    assert_eq!(radio.on_key(&key(KeyCode::Down)), Outcome::Changed);
    assert_eq!((radio.cursor, radio.selected), (1, 1));
    assert_eq!(radio.on_key(&key(KeyCode::Down)), Outcome::Changed);
    assert_eq!(
        (radio.cursor, radio.selected),
        (2, 1),
        "the cursor lands on a fixed option without choosing it"
    );
    assert_eq!(radio.on_key(&key(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(radio.selected, 1);
    assert_eq!(
        take_refusal().map(|r| r.text),
        Some("Contact sales".to_owned())
    );
    assert_eq!(radio.on_click(0), Outcome::Changed);
    assert_eq!(radio.selected, 0);
    assert_eq!(radio.on_click(2), Outcome::Changed);
    assert_eq!(
        radio.selected, 0,
        "a fixed option cannot be chosen by click"
    );
    assert_eq!(radio.on_click(9), Outcome::Consumed);
    assert_eq!(radio.on_key(&key(KeyCode::Char('x'))), Outcome::Ignored);
    let part = RadioGroup::option_part(1);
    assert_eq!(radio.option_id(1), radio.id.part(part));
    let custom_control = RadioGroup::new(id("choice.radio2"), "Mode", &["A", "B"], 0)
        .option_control(1, Control::fixed(Reason::new("Locked")));
    assert!(custom_control.controls[1].is_fixed());
    assert!(!custom_control.controls[0].is_fixed());

    let mut vim = RadioGroup::new(id("choice.vim"), "Pick", &["One", "Two"], 0).nav(NavKeys::VIM);
    assert_eq!(vim.on_key(&key(KeyCode::Char('j'))), Outcome::Changed);
    assert_eq!(vim.selected, 1);
    let mut desktop = RadioGroup::new(id("choice.desk"), "Pick", &["One", "Two"], 0);
    assert_eq!(desktop.on_key(&key(KeyCode::Char('j'))), Outcome::Ignored);

    let frame = paint_ring(
        Rect::new(0, 0, 20, 4),
        Interaction::default(),
        |_, buf, ctx| {
            radio.render(Rect::new(0, 0, 20, 4), buf, ctx, ctx.theme.canvas);
        },
    );
    let text = text_of(&frame.buf);
    assert!(text.contains("Plan") && text.contains("Free") && text.contains("Team"));
    assert!(text.contains(theme.glyphs.radio_on));
    assert_eq!(radio.areas.len(), 3);
    assert!(
        frame
            .hits
            .regions()
            .iter()
            .any(|r| r.id == radio.option_id(1))
    );

    // Toggle: keys and clicks flip it, disabled ignores keys.
    let mut toggle = Toggle::new(id("choice.toggle"), "Dark mode", false);
    assert_eq!(toggle.width(), 19);
    assert_eq!(toggle.on_key(&key(KeyCode::Enter)), Outcome::Changed);
    assert!(toggle.on);
    assert_eq!(toggle.on_click(), Outcome::Changed);
    assert!(!toggle.on);
    assert_eq!(toggle.on_key(&key(KeyCode::Char('x'))), Outcome::Ignored);
    let mut disabled = Toggle::new(id("choice.off"), "Off", true).disabled(true);
    assert_eq!(disabled.on_key(&key(KeyCode::Char(' '))), Outcome::Ignored);
    assert_eq!(disabled.on_click(), Outcome::Consumed);
    assert!(disabled.on);
    let frame = paint_ring(
        Rect::new(0, 0, 30, 1),
        Interaction::default(),
        |_, buf, ctx| {
            toggle.render(Rect::new(0, 0, 30, 1), buf, ctx, ctx.theme.canvas);
        },
    );
    assert!(row_text(&frame.buf, 0).contains("Dark mode"));
}

fn fields_icon_button() {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let close = IconButton::close(id("icon.close"));
    assert_eq!(close.look, IconLook::Close);
    assert_eq!(close.pad, IconButton::DEFAULT_PAD);
    assert_eq!(close.glyph(&theme), theme.glyphs.close);
    assert_eq!(
        close.width(&theme),
        3,
        "the glyph plus one pad cell each side"
    );

    let gear = IconButton::new(id("icon.gear"), "⚙");
    assert_eq!(gear.look, IconLook::Title);
    assert_eq!(gear.glyph(&theme), "⚙");
    let padded = gear.clone().pad(2);
    assert_eq!(padded.width(&theme), 5);
    assert_eq!(gear.clone().plate(5).pad, 2);
    assert_eq!(gear.clone().plate(1).pad, 0);
    let swapped = gear.with_glyph("✎");
    assert_eq!(swapped.glyph(&theme), "✎");
    let renamed = swapped.with_id(id("icon.edit"));
    assert_eq!(renamed.id, id("icon.edit"));
    assert!(renamed.part.is_none());
    let owner = id("icon.owner");
    let owned = close.part_of(owner, junie_tui::core::id::PartRef::of(Part::ROW));
    assert_eq!(owned.part.map(|(o, _)| o), Some(owner));
    assert!(!close.danger_hover);
    let danger = close.clone().danger_hover(true);
    assert!(danger.danger_hover);
    let tipped = danger.clone().tip(Tip::new("Close").shortcut("Esc"));
    assert_eq!(tipped.tip, Some(Tip::new("Close").shortcut("Esc")));

    // Rest and hover paint differently; the plate is clickable.
    let host = ratatui::style::Style::default()
        .fg(theme.text_secondary)
        .bg(theme.canvas);
    let rest = paint_ring(
        Rect::new(0, 0, 10, 1),
        Interaction::default(),
        |_, buf, ctx| {
            assert_eq!(
                tipped.render(2, 0, buf, ctx, host, None),
                Rect::new(2, 0, 3, 1)
            );
        },
    );
    assert_eq!(rest.buf[(3, 0)].symbol(), theme.glyphs.close);
    assert!(rest.hits.regions().iter().any(|r| r.id == id("icon.close")));
    let hover = paint_ring(
        Rect::new(0, 0, 10, 1),
        Interaction {
            hover: Some(id("icon.close")),
            ..Interaction::default()
        },
        |_, buf, ctx| {
            tipped.render(2, 0, buf, ctx, host, None);
        },
    );
    assert_ne!(
        rest.buf[(3, 0)].style(),
        hover.buf[(3, 0)].style(),
        "hover restyles the plate"
    );
    paint_ring(
        Rect::new(0, 0, 10, 1),
        Interaction::default(),
        |_, _, ctx| {
            assert_ne!(
                close.style(ctx, host),
                IconButton::new(id("icon.title"), "x").style(ctx, host),
                "the close and title looks style the plate differently"
            );
        },
    );

    // `danger_hover` recolours a hovered plate in both looks.
    paint_ring(
        Rect::new(0, 0, 10, 1),
        Interaction {
            hover: Some(id("icon.close")),
            ..Interaction::default()
        },
        |_, _, ctx| {
            for calm in [
                IconButton::close(id("icon.close")),
                IconButton::new(id("icon.close"), "x"),
            ] {
                assert_ne!(
                    calm.style(ctx, host),
                    calm.clone().danger_hover(true).style(ctx, host),
                    "a hovered plate turns to its danger colour"
                );
            }
        },
    );

    // `render_in` draws inside a caller-owned plate.
    let mut registered = None;
    let in_plate = paint_ring(
        Rect::new(0, 0, 10, 1),
        Interaction::default(),
        |_, buf, ctx| {
            registered = Some(padded.render_in(Rect::new(0, 0, 5, 1), 2, buf, ctx, host, None));
        },
    );
    assert_eq!(registered, Some(Rect::new(0, 0, 5, 1)));
    assert_eq!(in_plate.buf[(2, 0)].symbol(), "⚙");
    let part_frame = paint_ring(
        Rect::new(0, 0, 10, 1),
        Interaction::default(),
        |_, buf, ctx| {
            owned.render(1, 0, buf, ctx, host, None);
        },
    );
    assert!(part_frame.hits.regions().iter().any(|r| r.owner == owner));
}

fn ctrl(c: char) -> Key {
    Key {
        code: KeyCode::Char(c),
        mods: KeyModifiers::CONTROL,
    }
}

fn fields_validators() {
    let mut err = FieldError::new("too short");
    assert_eq!(err.message(), "too short");
    err.message_mut().push('!');
    assert_eq!(&*err, "too short!");
    assert_eq!(err.to_string(), "too short!");
    assert_eq!(FieldError::from("a"), FieldError::new("a"));
    assert_eq!(FieldError::from("owned".to_owned()).into_message(), "owned");
    assert_eq!(err.into_message(), "too short!");

    // A validator may parse: the Ok value is the parsed type.
    let parse = Validator::new(|t| {
        t.parse::<u32>()
            .map_err(|_| FieldError::new("not a number"))
    });
    assert_eq!(parse.validate("42").unwrap(), 42);
    assert_eq!(parse.validate("x").unwrap_err().message(), "not a number");

    // A contextual validator reads outside state, and `bind` fixes it.
    let ranged = Validator::with_context(|max: &u32, t: &str| {
        let n: u32 = t.parse().map_err(|_| FieldError::new("not a number"))?;
        if n > *max {
            Err(FieldError::new(format!("over {max}")))
        } else {
            Ok(n)
        }
    });
    assert_eq!(ranged.validate_in(&10, "7").unwrap(), 7);
    assert_eq!(ranged.validate_in(&5, "7").unwrap_err().message(), "over 5");
    let bound = ranged.bind(9);
    assert_eq!(bound.validate("9").unwrap(), 9);
    assert_eq!(bound.validate("10").unwrap_err().message(), "over 9");

    // The boxed function shape behind `Validator` is nameable.
    let boxed: Box<ValidateFn<usize, ()>> = Box::new(|_, t| Ok(t.len()));
    assert_eq!(boxed(&(), "four").unwrap(), 4);

    // Each ValidateWhen decides when typing validates.
    let reject = || Validator::check(|t| t.contains('!').then(|| "no bang".to_owned()));
    for (when, expect_after_bad_edit, expect_after_error) in [
        (ValidateWhen::Commit, false, false),
        (ValidateWhen::AfterError, false, true),
        (ValidateWhen::Live, true, true),
    ] {
        let mut field = EditField::single("ok")
            .validator(reject())
            .validate_when(when);
        assert_eq!(field.validate_when, when);
        field.buffer.move_doc_end(false);
        field.on_key(&key(KeyCode::Char('!')));
        assert_eq!(
            field.error_message().is_some(),
            expect_after_bad_edit,
            "{when:?} first bad edit"
        );
        field.set_error(Some(FieldError::new("stale")));
        field.on_key(&key(KeyCode::Char('?')));
        let revalidated = field.error_message() == Some("no bang");
        assert_eq!(
            revalidated, expect_after_error,
            "{when:?} with an error showing"
        );
        // Fixing the text clears the error again when it revalidates.
        field.set_text("fine");
        field.set_error(Some(FieldError::new("stale")));
        field.after_edit();
        assert_eq!(
            field.error_message().is_none(),
            expect_after_error,
            "{when:?} after_edit clears a stale error only when validating"
        );
    }
    assert_eq!(ValidateWhen::default(), ValidateWhen::AfterError);
}

fn fields_edit_field_core() {
    let mut field = EditField::single("hello").keys(FieldKeys::Desktop);
    field.buffer.move_doc_end(false);
    assert_eq!(field.keys, FieldKeys::Desktop);
    assert!(!field.multiline);
    assert!(EditField::multi("a\nb").multiline);

    // The key table, as `action` reads it.
    assert!(matches!(
        field.action(&key(KeyCode::Enter)),
        EditAction::Commit
    ));
    assert!(matches!(
        field.action(&key(KeyCode::Esc)),
        EditAction::Cancel
    ));
    assert!(matches!(
        field.action(&key(KeyCode::Tab)),
        EditAction::Tab { backward: false }
    ));
    assert!(matches!(
        field.action(&key(KeyCode::BackTab)),
        EditAction::Tab { backward: true }
    ));
    assert!(matches!(
        field.action(&key(KeyCode::Char('x'))),
        EditAction::Insert('x')
    ));
    assert!(matches!(
        field.action(&key(KeyCode::Left)),
        EditAction::Apply(_)
    ));
    assert!(matches!(field.action(&ctrl('z')), EditAction::Undo));
    assert!(matches!(field.action(&ctrl('y')), EditAction::Redo));
    assert!(matches!(field.action(&ctrl('a')), EditAction::SelectAll));
    assert!(matches!(field.action(&ctrl('v')), EditAction::Paste));
    assert!(matches!(field.action(&ctrl('x')), EditAction::Cut));
    assert!(matches!(
        field.action(&key(KeyCode::F(5))),
        EditAction::None
    ));
    assert!(matches!(
        EditField::multi("a").action(&key(KeyCode::Enter)),
        EditAction::Apply(_)
    ));
    assert_eq!(field.text_cmd(&ctrl('z')), Some(TextCmd::Undo));
    assert_eq!(field.text_cmd(&key(KeyCode::Char('q'))), None);

    // Typing edits and records undo history; an exit key only reports.
    let typed = field.on_key(&key(KeyCode::Char('!')));
    assert_eq!(
        typed,
        EditResult {
            outcome: Outcome::Changed,
            edited: true,
            exit: None,
            unused: false
        }
    );
    assert_eq!(field.text(), "hello!");
    assert!(field.can_undo() && !field.can_redo());
    field.on_key(&ctrl('z'));
    assert_eq!(field.text(), "hello");
    assert!(field.can_redo());
    field.on_key(&ctrl('y'));
    assert_eq!(field.text(), "hello!");
    assert_eq!(
        field.on_key(&key(KeyCode::Enter)).exit,
        Some(EditExit::Commit)
    );
    assert_eq!(
        field.on_key(&key(KeyCode::Esc)).exit,
        Some(EditExit::Cancel)
    );
    assert_eq!(
        field.on_key(&key(KeyCode::BackTab)).exit,
        Some(EditExit::Tab { backward: true })
    );
    let unbound = field.on_key(&key(KeyCode::F(5)));
    assert!(unbound.unused && !unbound.edited);
    assert_eq!(
        unbound.outcome,
        Outcome::Consumed,
        "a plain unbound key is eaten"
    );
    let chord = field.on_key(&Key {
        code: KeyCode::F(5),
        mods: KeyModifiers::CONTROL,
    });
    assert_eq!(
        chord.outcome,
        Outcome::Ignored,
        "a chord is left to the host"
    );
    assert_eq!(unbound_key(&key(KeyCode::F(5))), Outcome::Consumed);
    assert_eq!(field.text(), "hello!");

    // `apply` obeys read_only, `apply_as` overrides it for one call.
    field.read_only = true;
    let ignored = field.apply(EditAction::Insert('?'), &key(KeyCode::Char('?')));
    assert!(!ignored.edited);
    assert_eq!(field.text(), "hello!");
    let forced = field.apply_as(EditAction::Insert('?'), &key(KeyCode::Char('?')), false);
    assert!(forced.edited);
    assert_eq!(field.text(), "hello!?");
    assert!(!field.paste("more"), "paste is refused while read-only");
    field.read_only = false;

    // Commands: copy, cut, paste and the line commands.
    assert!(field.run(TextCmd::SelectAll).outcome == Outcome::Changed);
    field.run(TextCmd::Copy);
    assert_eq!(clipboard::text().as_deref(), Some("hello!?"));
    field.run(TextCmd::Cut);
    assert_eq!(field.text(), "");
    let pasted = field.run(TextCmd::Paste);
    assert!(pasted.edited);
    assert_eq!(field.text(), "hello!?");
    field.read_only = true;
    assert!(!field.run(TextCmd::Paste).edited);
    assert!(!field.run_as(TextCmd::Paste, true).edited);
    assert_eq!(field.text(), "hello!?");
    // The override rules paste as it rules every other command: a read-only
    // field takes a paste the caller allows, and a writable one refuses a
    // paste the caller forbids.
    let clip = clipboard::text().expect("the cut above left text on the clipboard");
    let mut locked = EditField::single("ab");
    locked.read_only = true;
    locked.buffer.move_doc_end(false);
    assert!(locked.run_as(TextCmd::Paste, false).edited);
    assert_eq!(locked.text(), format!("ab{clip}"));
    locked.read_only = false;
    assert!(!locked.run_as(TextCmd::Paste, true).edited);
    assert_eq!(locked.text(), format!("ab{clip}"));
    field.read_only = false;
    assert_eq!(
        field.run_as(TextCmd::LineStart, true).outcome,
        Outcome::Changed
    );
    assert_eq!(field.buffer.cursor_offset(), 0);

    // Shortcuts follow the key profile and a custom map.
    let desktop = EditField::single("").keys(FieldKeys::Desktop);
    let emacs = EditField::single("").keys(FieldKeys::Emacs);
    assert!(desktop.shortcut(TextCmd::Copy).is_some());
    assert_ne!(
        desktop.shortcut(TextCmd::Copy),
        emacs.shortcut(TextCmd::Copy)
    );
    let mut remapped = EditField::single("x").keys(FieldKeys::Desktop);
    remapped.text_keymap = Some(vec![(
        junie_tui::core::keymap::KeyBinding::ctrl('q'),
        TextCmd::Copy,
    )]);
    assert_eq!(remapped.text_cmd(&ctrl('q')), Some(TextCmd::Copy));
    assert_eq!(
        remapped.text_cmd(&ctrl('z')),
        None,
        "the custom map replaces the defaults"
    );
    assert!(matches!(remapped.action(&ctrl('q')), EditAction::Copy));
    assert!(remapped.shortcut(TextCmd::Copy).unwrap().contains('Q'));
    assert_eq!(remapped.shortcut(TextCmd::Undo), None);

    // Pasting normalises line endings per field kind.
    let mut single = EditField::single("");
    assert!(single.paste("a\r\nb\n"));
    assert_eq!(single.text(), "a b");
    assert!(!single.paste(""));
    let mut multi = EditField::multi("");
    assert!(multi.paste("a\r\nb\rc"));
    assert_eq!(multi.text(), "a\nb\nc");

    // History: record/seal/clear_history.
    let mut history = EditField::single("abc");
    assert!(!history.can_undo());
    history.record(EditKind::Typing);
    assert!(history.can_undo());
    history.seal();
    history.clear_history();
    assert!(!history.can_undo() && !history.can_redo());
    history.set_text("fresh");
    assert_eq!(history.text(), "fresh");

    // Required comes before the validator; error getters read the result.
    let mut required = EditField::single("");
    required.required = Some("Fill this in".to_owned());
    required.validator = Some(Validator::check(|_| Some("never reached".to_owned())));
    assert!(!required.validate());
    assert_eq!(required.error_message(), Some("Fill this in"));
    required.set_error(None);
    assert_eq!(required.error_message(), None);

    // Pointer selection: press, drag and click are one gesture.
    let mut pointer = EditField::single("hello big world");
    assert!(!pointer.pressed());
    assert!(!pointer.press(2, 1, false));
    assert!(pointer.pressed());
    assert!(pointer.drag(7));
    assert_eq!(pointer.buffer.selection(), Some(2..7));
    pointer.click(7, 1, false);
    assert!(!pointer.pressed());
    assert_eq!(
        pointer.buffer.selection(),
        Some(2..7),
        "a drag keeps its selection on release"
    );
    assert!(
        pointer.press(7, 2, false),
        "a double press selects the word"
    );
    assert_eq!(pointer.buffer.selection(), Some(6..9));
    assert!(pointer.drag(13));
    assert_eq!(
        pointer.buffer.selection(),
        Some(6..15),
        "dragging after a double press grows by words"
    );
    pointer.click(13, 1, false);
    pointer.click(7, 2, false);
    assert_eq!(pointer.buffer.selection(), Some(6..9));
    pointer.click(7, 2, true);
    assert_eq!(
        pointer.buffer.selection(),
        Some(0..15),
        "whole-word fields select everything"
    );
    pointer.click(7, 3, false);
    assert_eq!(pointer.buffer.selection(), Some(0..15));
    pointer.click(4, 1, false);
    assert_eq!(pointer.buffer.selection(), None);
    assert_eq!(pointer.buffer.cursor_offset(), 4);
    let mut locked = EditField::single("abc");
    locked.sel = PointerSelect::OFF;
    locked.press(1, 2, false);
    assert!(!locked.drag(2), "drag selection can be switched off");
    assert_eq!(locked.buffer.selection(), None);

    // Drawing: the viewport follows the caret and reports the cursor cell.
    let look = LineLook {
        text: Style::default(),
        selection: Style::default().bg(ratatui::style::Color::Blue),
        ellipsis: Style::default(),
        ellipsis_glyph: "…",
        underline: None,
        follow_caret: true,
    };
    let mut line = EditField::single("hello wide world");
    line.buffer.move_doc_end(false);
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
    let area = Rect::new(0, 0, 8, 1);
    let cursor = line.draw_line(&mut buf, area, &look);
    assert!(line.line_scroll() > 0, "the caret scrolled the line");
    assert_eq!(line.line_area(), area);
    assert_eq!(
        buf[(0, 0)].symbol(),
        "…",
        "clipped text is marked at the left"
    );
    assert!(cursor.x < area.right());
    assert_eq!(
        line.line_offset_at(Position::new(5, 0)),
        line.buffer.offset_at(0, line.line_scroll() + 5)
    );
    let glyphs = EditField::graphemes("aé日");
    assert_eq!(
        glyphs,
        vec![
            (0, "a".to_owned(), 1),
            (1, "é".to_owned(), 1),
            (3, "日".to_owned(), 2)
        ]
    );
    let mut cjk = EditField::single("aé日b");
    cjk.buffer.move_doc_start(false);
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
    cjk.draw_glyphs(
        &mut buf,
        Rect::new(0, 0, 10, 1),
        &LineLook {
            follow_caret: false,
            ..look
        },
        &glyphs,
    );
    assert_eq!(buf[(2, 0)].symbol(), "日");
    assert_eq!(cjk.glyph_offset_at(Position::new(2, 0), &glyphs), 3);
    assert_eq!(
        cjk.glyph_offset_at(Position::new(9, 0), &glyphs),
        cjk.text().len()
    );
    let mut selecting = EditField::single("abcdef");
    selecting.buffer.select_range(1, 3);
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
    selecting.draw_line(&mut buf, Rect::new(0, 0, 10, 1), &look);
    assert_eq!(buf[(1, 0)].style().bg, look.selection.bg);
    assert_ne!(buf[(4, 0)].style().bg, look.selection.bg);
}

fn fields_field_common() {
    // PointerSelect presets and the builder chain.
    assert_eq!(PointerSelect::default(), PointerSelect::DEFAULT);
    let all = PointerSelect::DEFAULT;
    assert!(all.drag && all.word && all.line && all.drag_by_word);
    let none = PointerSelect::OFF;
    assert!(!none.drag && !none.word && !none.line && !none.drag_by_word);
    let custom = PointerSelect::OFF
        .drag(true)
        .word(true)
        .line(true)
        .drag_by_word(true);
    assert_eq!(custom, PointerSelect::DEFAULT);
    assert_eq!(
        PointerSelect::DEFAULT
            .line(false)
            .drag_by_word(false)
            .word(false)
            .drag(false),
        PointerSelect::OFF
    );

    // The key tables.
    for (code, mods, check) in [
        (KeyCode::Enter, KeyModifiers::NONE, "commit"),
        (KeyCode::Esc, KeyModifiers::NONE, "cancel"),
        (KeyCode::Tab, KeyModifiers::NONE, "tab"),
        (KeyCode::Char('q'), KeyModifiers::NONE, "insert"),
        (KeyCode::Char('q'), KeyModifiers::ALT, "none"),
        (KeyCode::Backspace, KeyModifiers::NONE, "apply"),
        (KeyCode::Char('h'), KeyModifiers::CONTROL, "apply"),
        (KeyCode::F(3), KeyModifiers::NONE, "none"),
    ] {
        let k = Key { code, mods };
        let action = edit_key(&k, false);
        let ok = match check {
            "commit" => matches!(action, EditAction::Commit),
            "cancel" => matches!(action, EditAction::Cancel),
            "tab" => matches!(action, EditAction::Tab { backward: false }),
            "insert" => matches!(action, EditAction::Insert('q')),
            "apply" => matches!(action, EditAction::Apply(_)),
            _ => matches!(action, EditAction::None),
        };
        assert!(ok, "{code:?} {mods:?} should be {check}");
    }
    assert!(matches!(
        edit_key(&key(KeyCode::Enter), true),
        EditAction::Apply(_)
    ));
    assert!(matches!(
        edit_key(&key(KeyCode::Up), true),
        EditAction::Apply(_)
    ));
    assert!(matches!(
        edit_key(&key(KeyCode::Up), false),
        EditAction::None
    ));
    assert!(matches!(
        edit_key_with(&ctrl('a'), false, FieldKeys::Desktop),
        EditAction::SelectAll
    ));
    assert!(matches!(
        edit_key_with(&ctrl('a'), false, FieldKeys::Emacs),
        EditAction::Apply(_)
    ));
    let only_q = vec![(junie_tui::core::keymap::KeyBinding::ctrl('q'), TextCmd::Cut)];
    assert!(matches!(
        edit_key_in(&ctrl('q'), false, FieldKeys::Desktop, &only_q),
        EditAction::Cut
    ));
    assert!(matches!(
        edit_key_in(&ctrl('z'), false, FieldKeys::Desktop, &only_q),
        EditAction::None
    ));

    // Each text command maps to its action; document commands need multiline.
    for (cmd, expect) in [
        (TextCmd::SelectAll, "select_all"),
        (TextCmd::Copy, "copy"),
        (TextCmd::Cut, "cut"),
        (TextCmd::Paste, "paste"),
        (TextCmd::Undo, "undo"),
        (TextCmd::Redo, "redo"),
        (TextCmd::LineStart, "apply"),
        (TextCmd::SelectToLineEnd, "apply"),
    ] {
        let got = match text_cmd_action(cmd, false) {
            EditAction::SelectAll => "select_all",
            EditAction::Copy => "copy",
            EditAction::Cut => "cut",
            EditAction::Paste => "paste",
            EditAction::Undo => "undo",
            EditAction::Redo => "redo",
            EditAction::Apply(_) => "apply",
            _ => "other",
        };
        assert_eq!(got, expect, "{cmd:?}");
    }
    assert!(matches!(
        text_cmd_action(TextCmd::DocStart, false),
        EditAction::None
    ));
    assert!(matches!(
        text_cmd_action(TextCmd::DocStart, true),
        EditAction::Apply(_)
    ));

    // apply_edit reports Edited when text changes and Moved when only the caret does.
    let mut buffer = TextBuffer::single("abc");
    buffer.move_doc_end(false);
    let mut undo = UndoStack::new();
    let backspace = edit_key(&key(KeyCode::Backspace), false);
    assert_eq!(
        apply_edit(
            &mut buffer,
            &mut undo,
            backspace,
            KeyCode::Backspace,
            false,
            false
        ),
        Some(Applied::Edited)
    );
    assert_eq!(buffer.text(), "ab");
    let left = edit_key(&key(KeyCode::Left), false);
    assert_eq!(
        apply_edit(&mut buffer, &mut undo, left, KeyCode::Left, false, false),
        Some(Applied::Moved)
    );
    assert_eq!(
        apply_edit(
            &mut buffer,
            &mut undo,
            EditAction::Insert('X'),
            KeyCode::Char('X'),
            true,
            false
        ),
        Some(Applied::Moved),
        "read-only blocks inserts"
    );
    assert_eq!(buffer.text(), "ab");
    assert_eq!(
        apply_edit(
            &mut buffer,
            &mut undo,
            EditAction::Undo,
            KeyCode::Null,
            false,
            false
        ),
        Some(Applied::Edited)
    );
    assert_eq!(buffer.text(), "abc");
    assert_eq!(
        apply_edit(
            &mut buffer,
            &mut undo,
            EditAction::Redo,
            KeyCode::Null,
            false,
            false
        ),
        Some(Applied::Edited)
    );
    assert_eq!(
        apply_edit(
            &mut buffer,
            &mut undo,
            EditAction::Commit,
            KeyCode::Enter,
            false,
            false
        ),
        None,
        "exit actions are not edits"
    );
    clipboard::copy("");
    buffer.select_all();
    assert_eq!(
        apply_edit(
            &mut buffer,
            &mut undo,
            EditAction::Copy,
            KeyCode::Null,
            false,
            true
        ),
        Some(Applied::Moved)
    );
    assert_eq!(
        clipboard::text().as_deref(),
        Some(""),
        "a secret field never copies"
    );
    apply_edit(
        &mut buffer,
        &mut undo,
        EditAction::Copy,
        KeyCode::Null,
        false,
        false,
    );
    assert_eq!(clipboard::text().as_deref(), Some("ab"));

    // paste_clipboard hands the clipboard text to the handler.
    clipboard::copy("from clipboard");
    let mut seen = String::new();
    let outcome = paste_clipboard(|t| {
        seen = t.to_owned();
        Outcome::Changed
    });
    assert_eq!(
        (outcome, seen.as_str()),
        (Outcome::Changed, "from clipboard")
    );
}

/// A tiny host of an edit field, to drive the shared pointer routing.
struct Pad {
    id: WidgetId,
    field: EditField,
    begins: u32,
    afters: u32,
    focuses: u32,
    label_clicks: u32,
    whole_word: bool,
    enabled: bool,
}

impl Pad {
    fn new(name: &str, text: &str) -> Self {
        let mut field = EditField::single(text);
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 3));
        let look = LineLook {
            text: Style::default(),
            selection: Style::default(),
            ellipsis: Style::default(),
            ellipsis_glyph: "…",
            underline: None,
            follow_caret: false,
        };
        field.draw_line(&mut buf, Rect::new(0, 1, 30, 1), &look);
        Self {
            id: id(name),
            field,
            begins: 0,
            afters: 0,
            focuses: 0,
            label_clicks: 0,
            whole_word: false,
            enabled: true,
        }
    }
}

impl Editable for Pad {
    fn field(&mut self) -> &mut EditField {
        &mut self.field
    }
    fn edit_id(&self) -> WidgetId {
        self.id
    }
    fn offset_at(&self, pos: Position) -> usize {
        self.field.line_offset_at(pos)
    }
    fn pointer_enabled(&self) -> bool {
        self.enabled
    }
    fn whole_word(&self) -> bool {
        self.whole_word
    }
    fn on_label(&self, pos: Position) -> bool {
        pos.y == 0
    }
    fn label_click(&mut self) {
        self.label_clicks += 1;
    }
    fn pointer_begin(&mut self) {
        self.begins += 1;
    }
    fn pointer_focus(&mut self) {
        self.focuses += 1;
    }
    fn pointer_after(&mut self) {
        self.afters += 1;
    }
}

fn fields_pointer_routing() {
    let mut pad = Pad::new("pad.one", "hello big world");
    assert_eq!(pad.pointer_id(), id("pad.one"));
    // A single press only arms the gesture; a double press selects a word.
    assert_eq!(
        pad.on_press_count(Position::new(1, 1), 1),
        Outcome::Consumed
    );
    assert_eq!(pad.focuses, 1);
    assert_eq!(pad.begins, 0);
    assert_eq!(pad.on_press_count(Position::new(7, 1), 2), Outcome::Changed);
    assert_eq!(pad.field.buffer.selection(), Some(6..9));
    assert_eq!((pad.begins, pad.afters), (1, 1));
    assert_eq!(pad.on_drag(Position::new(13, 1)), Outcome::Changed);
    assert_eq!(pad.field.buffer.selection(), Some(6..15));
    assert_eq!(
        pad.on_click_count(Position::new(13, 1), 1),
        Outcome::Changed
    );
    assert_eq!(pad.on_click_count(Position::new(7, 1), 2), Outcome::Changed);
    assert_eq!(pad.field.buffer.selection(), Some(6..9));
    pad.whole_word = true;
    pad.on_click_count(Position::new(7, 1), 2);
    assert_eq!(pad.field.buffer.selection(), Some(0..15));
    pad.whole_word = false;
    // A click on the label row reaches the label hook instead of the text.
    let before = pad.field.buffer.selection();
    assert_eq!(pad.on_click_count(Position::new(2, 0), 1), Outcome::Changed);
    assert_eq!(pad.label_clicks, 1);
    assert_eq!(pad.field.buffer.selection(), before);
    assert_eq!(
        pad.on_press_count(Position::new(2, 0), 2),
        Outcome::Consumed
    );
    // Drag selection can be switched off; a disabled pad ignores the pointer.
    pad.field.sel = PointerSelect::OFF;
    assert_eq!(pad.on_drag(Position::new(3, 1)), Outcome::Ignored);
    pad.enabled = false;
    assert_eq!(
        pad.on_click_count(Position::new(3, 1), 1),
        Outcome::Consumed
    );
    assert_eq!(
        pad.on_press_count(Position::new(3, 1), 2),
        Outcome::Consumed
    );
    assert_eq!(pad.on_drag(Position::new(3, 1)), Outcome::Ignored);

    // The route helpers find the field by id and forward the gesture.
    let mut a = Pad::new("pad.a", "alpha beta");
    let mut b = Pad::new("pad.b", "gamma delta");
    {
        let mut fields: [&mut dyn PointerField; 2] = [&mut a, &mut b];
        assert_eq!(
            route_press(&mut fields, id("pad.b"), Position::new(7, 1), 2),
            Some(Outcome::Changed)
        );
        assert_eq!(
            route_press(&mut fields, id("pad.none"), Position::new(0, 1), 1),
            None
        );
        assert_eq!(
            route_drag(&mut fields, id("pad.b"), Position::new(10, 1)),
            Some(Outcome::Changed)
        );
        assert_eq!(
            route_drag(&mut fields, id("pad.none"), Position::new(0, 1)),
            None
        );
        assert_eq!(
            route_click(&mut fields, id("pad.a"), Position::new(2, 1), 0),
            Some(Outcome::Changed)
        );
        assert_eq!(
            route_click(&mut fields, id("pad.none"), Position::new(0, 1), 1),
            None
        );
    }
    assert_eq!(b.field.buffer.selection(), Some(6..11));
    assert_eq!(
        a.field.buffer.selection(),
        None,
        "a count of zero acts as a single click"
    );
    assert_eq!(a.field.buffer.cursor_offset(), 2);
}

fn text_input_frame(input: &mut TextInput, width: u16, focus: bool) -> Ringed {
    let interaction = Interaction {
        focus: focus.then_some(input.id),
        ..Interaction::default()
    };
    paint_ring(Rect::new(0, 0, width, 4), interaction, |a, buf, ctx| {
        input.render(a, buf, ctx, ctx.theme.canvas)
    })
}

fn fields_text_input_builders() {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    assert_eq!(TextInput::HEIGHT, 3);
    assert_eq!(TextInput::GUTTER, 2);
    let base = |name: &str| TextInput::new(id(name), "Name").edit_entry(EditEntry::Explicit);

    // Height follows the message row and label wrapping.
    assert_eq!(base("ti.h").height(), 2);
    assert_eq!(base("ti.h").help("Shown under the field").height(), 3);
    let mut errored = base("ti.h");
    errored.set_error(Some("Bad value".to_owned()));
    assert_eq!(errored.height(), 3);
    assert_eq!(errored.error(), Some("Bad value"));
    errored.set_error(None);
    assert_eq!(errored.height(), 2);
    let long = TextInput::new(
        id("ti.long"),
        "A label that is far too long for a narrow column",
    )
    .wrap_label(true);
    assert!(long.wrap_label);
    assert!(
        long.height_for(14) > long.height_for(80),
        "a wrapped label takes more rows"
    );
    let flat = TextInput::new(
        id("ti.long"),
        "A label that is far too long for a narrow column",
    );
    assert_eq!(
        flat.height_for(14),
        flat.height_for(80),
        "an unwrapped label keeps one row"
    );

    // Label decorations: optional marker, required star, plain label.
    let mut optional = base("ti.opt");
    let text = text_of(&text_input_frame(&mut optional, 40, false).buf);
    assert!(text.contains(theme.strings.optional));
    let mut plain = base("ti.opt").plain_label();
    assert!(plain.plain_label);
    assert!(
        !text_of(&text_input_frame(&mut plain, 40, false).buf).contains(theme.strings.optional)
    );
    let mut starred = base("ti.req").required(true);
    let text = text_of(&text_input_frame(&mut starred, 40, false).buf);
    assert!(text.contains("Name *"));
    assert!(!text.contains(theme.strings.optional));

    // The required message replaces the default and wins before a validator.
    let mut required = base("ti.req2")
        .required(true)
        .required_message("Say who you are")
        .validator(|_| Some("never reached".to_owned()));
    assert_eq!(
        required.required_message.as_deref(),
        Some("Say who you are")
    );
    assert!(!required.validate());
    assert_eq!(required.error(), Some("Say who you are"));
    let mut default_message = base("ti.req3").required(true);
    assert!(!default_message.validate());
    assert_eq!(default_message.error(), Some(theme.strings.required));

    // Placeholder shows only while empty.
    let mut hint = base("ti.ph").placeholder("Ada Lovelace");
    assert_eq!(hint.placeholder, "Ada Lovelace");
    assert!(text_of(&text_input_frame(&mut hint, 40, false).buf).contains("Ada Lovelace"));
    let mut filled = base("ti.ph").placeholder("Ada Lovelace").value("Grace");
    let text = text_of(&text_input_frame(&mut filled, 40, false).buf);
    assert!(text.contains("Grace") && !text.contains("Ada Lovelace"));

    // Masking hides the value; reveal_tail leaves the last characters.
    let mut masked = base("ti.mask").masked().value("hunter2!!");
    assert!(masked.masked && masked.field.secret);
    let text = text_of(&text_input_frame(&mut masked, 40, false).buf);
    assert!(text.contains(theme.glyphs.mask) && !text.contains("hunter2"));
    let mut tail = base("ti.mask").masked().reveal_tail(3).value("hunter2!!");
    assert_eq!(tail.reveal_tail, 3);
    let text = text_of(&text_input_frame(&mut tail, 40, false).buf);
    assert!(text.contains("2!!") && !text.contains("hunter"));

    // The readout sits at the right edge and may be a link.
    let mut readout = base("ti.out").readout("12/40").value("abc");
    assert_eq!(readout.readout, "12/40");
    assert!(text_of(&text_input_frame(&mut readout, 40, false).buf).contains("12/40"));
    readout.set_readout("13/40");
    assert!(text_of(&text_input_frame(&mut readout, 40, false).buf).contains("13/40"));
    let link = id("ti.out.link");
    let mut linked = base("ti.out").readout("clear").readout_link(link);
    assert_eq!(linked.readout_link, Some(link));
    let frame = text_input_frame(&mut linked, 40, false);
    assert!(
        frame.hits.regions().iter().any(|r| r.id == link),
        "the readout is clickable"
    );
    assert!(!linked.readout_area.is_empty());

    // label_hit decides whether the label row is a click target.
    let label_regions = |input: &mut TextInput| {
        let frame = text_input_frame(input, 40, false);
        frame
            .hits
            .regions()
            .iter()
            .filter(|r| r.id == input.id && r.area.y == 0)
            .count()
    };
    assert_eq!(label_regions(&mut base("ti.lh")), 1);
    assert_eq!(label_regions(&mut base("ti.lh").label_hit(false)), 0);

    // Entry policy and what the field reports it wants.
    let explicit = base("ti.entry");
    assert_eq!(explicit.entry(), EditEntry::Explicit);
    assert!(!explicit.wants_text_input());
    let mut type_to_edit = base("ti.entry");
    type_to_edit.set_edit_entry(EditEntry::TypeToEdit);
    assert_eq!(type_to_edit.edit_entry, Some(EditEntry::TypeToEdit));
    assert!(type_to_edit.wants_text_input());
    assert_eq!(
        base("ti.entry").edit_entry(EditEntry::Immediate).entry(),
        EditEntry::Immediate
    );
    #[allow(deprecated)]
    {
        assert_eq!(
            base("ti.entry").type_to_edit(true).entry(),
            EditEntry::TypeToEdit
        );
        assert_eq!(
            base("ti.entry").type_to_edit(false).entry(),
            EditEntry::Explicit
        );
    }
    let off = base("ti.entry")
        .disabled(true)
        .edit_entry(EditEntry::TypeToEdit);
    assert!(!off.wants_text_input(), "a disabled field never wants text");

    // Focus selection, key profile, key map and pointer policy land on the field.
    let mut focused = base("ti.sel")
        .value("hello")
        .edit_entry(EditEntry::Immediate)
        .focus_select(FocusSelect::SelectAll);
    assert_eq!(focused.focus_select, FocusSelect::SelectAll);
    text_input_frame(&mut focused, 40, true);
    assert_eq!(
        focused.field.buffer.selection(),
        Some(0..5),
        "focus selects everything"
    );
    let mut caret = base("ti.sel")
        .value("hello")
        .edit_entry(EditEntry::Immediate)
        .focus_select(FocusSelect::CaretStart);
    text_input_frame(&mut caret, 40, true);
    assert_eq!(
        (
            caret.field.buffer.selection(),
            caret.field.buffer.cursor_offset()
        ),
        (None, 0)
    );
    let emacs = base("ti.keys").field_keys(FieldKeys::Emacs);
    assert_eq!(emacs.field_keys, Some(FieldKeys::Emacs));
    assert_eq!(emacs.field.keys, FieldKeys::Emacs);
    let remapped = base("ti.keys").text_keymap(vec![(KeyBinding::ctrl('q'), TextCmd::Undo)]);
    assert_eq!(remapped.field.text_cmd(&ctrl('q')), Some(TextCmd::Undo));
    let frozen = base("ti.ptr").pointer_select(PointerSelect::OFF);
    assert_eq!(frozen.field.sel, PointerSelect::OFF);

    // Validation timing builders.
    assert_eq!(base("ti.v").validate_on, ValidateOn::Change);
    assert_eq!(
        base("ti.v")
            .validate_on(ValidateOn::EveryCommit)
            .validate_on,
        ValidateOn::EveryCommit
    );
    assert_eq!(
        base("ti.v")
            .validate_when(ValidateWhen::Live)
            .field
            .validate_when,
        ValidateWhen::Live
    );
    assert_eq!(
        base("ti.v").blur_action(BlurAction::Keep).blur_action,
        BlurAction::Keep
    );
    assert!(!base("ti.v").context_menu(false).edit_menu.enabled);
    assert!(base("ti.v").context_menu(true).edit_menu.enabled);

    // validate_when decides whether typing validates before a commit.
    for (when, shows) in [(ValidateWhen::Live, true), (ValidateWhen::Commit, false)] {
        let mut field = base("ti.vw")
            .validate_when(when)
            .validator(|t| t.contains('!').then(|| "no bang".to_owned()));
        field.begin_edit();
        field.on_key(&key(KeyCode::Char('!')));
        assert_eq!(field.error().is_some(), shows, "{when:?}");
    }

    // field_keys picks the chord that selects all, once the field is painted.
    for (keys, chord, selects) in [
        (FieldKeys::Emacs, 'l', true),
        (FieldKeys::Emacs, 'a', false),
        (FieldKeys::Desktop, 'a', true),
    ] {
        let mut field = base("ti.keys").field_keys(keys).value("hello");
        text_input_frame(&mut field, 40, false);
        field.begin_edit();
        field.on_key(&ctrl(chord));
        assert_eq!(
            field.field.buffer.selection().is_some(),
            selects,
            "{keys:?} ctrl+{chord}"
        );
    }
}

fn fields_text_input_editing() {
    let base = |name: &str| TextInput::new(id(name), "Name").edit_entry(EditEntry::Explicit);

    // Explicit entry: Enter opens, typing changes, Enter commits.
    let mut input = base("te.one").value("Ada");
    assert_eq!(
        input.on_key(&key(KeyCode::Char('x'))),
        (Outcome::Ignored, None)
    );
    assert_eq!(input.on_key(&key(KeyCode::Enter)), (Outcome::Changed, None));
    assert!(input.editing && input.wants_text_input());
    input.field.buffer.move_doc_end(false);
    assert_eq!(
        input.on_key(&key(KeyCode::Char('!'))),
        (Outcome::Changed, Some(InputAction::Changed))
    );
    assert_eq!(input.text(), "Ada!");
    assert!(input.is_dirty() && input.can_undo());
    assert_eq!(input.hints(), EDIT_FIELD.to_vec());
    assert_eq!(
        input.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(InputAction::Committed))
    );
    assert!(!input.editing && !input.is_dirty());
    assert_eq!(input.hints(), vec![EDIT_HINT]);
    assert!(base("te.dis").disabled(true).hints().is_empty());

    // Escape cancels and restores; the undo stack remembers the discarded text.
    input.begin_edit();
    input.field.buffer.move_doc_end(false);
    input.on_key(&key(KeyCode::Char('?')));
    assert_eq!(
        input.on_key(&key(KeyCode::Esc)),
        (Outcome::Changed, Some(InputAction::Cancelled))
    );
    assert_eq!(input.text(), "Ada!");

    // Tab commits and asks for the next field in either direction.
    input.begin_edit();
    assert_eq!(
        input.on_key(&key(KeyCode::Tab)),
        (
            Outcome::Changed,
            Some(InputAction::CommittedTab { backward: false })
        )
    );
    input.begin_edit();
    assert_eq!(
        input.on_key(&key(KeyCode::BackTab)),
        (
            Outcome::Changed,
            Some(InputAction::CommittedTab { backward: true })
        )
    );

    // commit, cancel and begin_edit can be driven without keys.
    let mut manual = base("te.manual").value("start");
    manual.begin_edit();
    manual.field.set_text("changed");
    assert!(manual.is_dirty());
    manual.cancel();
    assert_eq!(manual.text(), "start");
    assert!(!manual.editing);
    manual.begin_edit();
    manual.field.set_text("kept");
    manual.commit();
    assert_eq!((manual.text(), manual.editing), ("kept", false));
    manual.begin_edit();
    manual.clear();
    assert_eq!(manual.text(), "");
    assert!(manual.editing, "clear keeps the edit session");
    assert!(!manual.is_dirty() || manual.text() != "kept");
    manual.cancel();

    // Type-to-edit starts on a printable key and reports Changed.
    let mut typing = base("te.type")
        .edit_entry(EditEntry::TypeToEdit)
        .value("ab");
    assert_eq!(
        typing.on_key(&key(KeyCode::Char('c'))),
        (Outcome::Changed, Some(InputAction::Changed))
    );
    assert_eq!(typing.text(), "abc");
    assert!(typing.editing);

    // Immediate entry resumes editing after a commit.
    let mut immediate = base("te.imm").edit_entry(EditEntry::Immediate).value("go");
    immediate.on_key(&key(KeyCode::Char('!')));
    assert!(immediate.editing);
    immediate.on_key(&key(KeyCode::Enter));
    assert!(immediate.editing, "resume reopens after commit");
    immediate.commit();
    assert!(!immediate.editing);
    immediate.resume();
    assert!(immediate.editing);
    let mut explicit = base("te.res");
    explicit.resume();
    assert!(
        !explicit.editing,
        "explicit fields never resume on their own"
    );

    // Blur follows the configured action.
    for (action, expect, text, editing) in [
        (
            BlurAction::Commit,
            Some(InputAction::Committed),
            "typed",
            false,
        ),
        (
            BlurAction::Cancel,
            Some(InputAction::Cancelled),
            "orig",
            false,
        ),
        (BlurAction::Keep, None, "typed", true),
    ] {
        let mut field = base("te.blur").value("orig").blur_action(action);
        assert_eq!(field.on_blur(), None, "{action:?} not editing");
        field.begin_edit();
        field.field.set_text("typed");
        assert_eq!(field.on_blur(), expect, "{action:?}");
        assert_eq!((field.text(), field.editing), (text, editing), "{action:?}");
    }
    let mut clean = base("te.clean")
        .edit_entry(EditEntry::Immediate)
        .value("same");
    clean.begin_edit();
    assert_eq!(
        clean.on_blur(),
        None,
        "an untouched immediate field just closes"
    );
    assert!(!clean.editing);

    // Validation timing: Change validates only edited text, EveryCommit always.
    let hits = Rc::new(std::cell::Cell::new(0));
    for (when, expected) in [(ValidateOn::Change, 0), (ValidateOn::EveryCommit, 1)] {
        hits.set(0);
        let counter = hits.clone();
        let mut field = base("te.val")
            .value("fixed")
            .validate_on(when)
            .validator(move |_| {
                counter.set(counter.get() + 1);
                None
            });
        field.begin_edit();
        field.commit();
        assert_eq!(hits.get(), expected, "{when:?}");
    }

    // Programmatic value replacement ends the session and clears the error.
    let mut value = base("te.value").value("one");
    value.begin_edit();
    value.set_error(Some("stale".to_owned()));
    value.set_value("two");
    assert_eq!(
        (value.text(), value.editing, value.error()),
        ("two", false, None)
    );

    // Paste starts an edit unless disabled.
    let mut paste = base("te.paste");
    assert_eq!(paste.on_paste("pasted"), Outcome::Changed);
    assert_eq!(paste.text(), "pasted");
    assert_eq!(
        base("te.paste").disabled(true).on_paste("x"),
        Outcome::Ignored
    );
}

fn fields_text_input_pointer() {
    let mut input = TextInput::new(id("tp.one"), "Name")
        .edit_entry(EditEntry::Explicit)
        .value("hello big world");
    let frame = text_input_frame(&mut input, 40, false);
    let field_row = input.area.y;
    assert_eq!(field_row, 1);
    assert!(
        frame
            .hits
            .regions()
            .iter()
            .any(|r| r.id == input.id && r.area.y == field_row)
    );

    // A press arms the gesture, a drag selects, the click releases it.
    assert_eq!(
        input.on_press(Position::new(4, field_row)),
        Outcome::Consumed
    );
    assert_eq!(input.on_drag(Position::new(9, field_row)), Outcome::Changed);
    assert!(input.editing, "dragging opens the edit session");
    assert_eq!(input.field.buffer.selection(), Some(2..7));
    assert_eq!(
        input.on_click(Position::new(9, field_row)),
        Outcome::Changed
    );
    assert_eq!(input.field.buffer.selection(), Some(2..7));
    // Double press selects a word, a double click does too.
    assert_eq!(
        input.on_press_count(Position::new(8, field_row), 2),
        Outcome::Changed
    );
    assert_eq!(input.field.buffer.selection(), Some(6..9));
    assert_eq!(
        input.on_click_count(Position::new(8, field_row), 2),
        Outcome::Changed
    );
    assert_eq!(input.field.buffer.selection(), Some(6..9));
    assert_eq!(
        input.on_click(Position::new(5, field_row)),
        Outcome::Changed
    );
    assert_eq!(
        (
            input.field.buffer.selection(),
            input.field.buffer.cursor_offset()
        ),
        (None, 3)
    );

    // Clicking the label row opens the field with its focus selection.
    let mut labelled = TextInput::new(id("tp.label"), "Name")
        .edit_entry(EditEntry::Explicit)
        .value("abc");
    text_input_frame(&mut labelled, 40, false);
    assert_eq!(labelled.on_click(Position::new(3, 0)), Outcome::Changed);
    assert!(labelled.editing);

    // Masked fields select the whole value on a double click.
    let mut secret = TextInput::new(id("tp.secret"), "Key")
        .edit_entry(EditEntry::Explicit)
        .masked()
        .value("one two");
    text_input_frame(&mut secret, 40, false);
    secret.on_click_count(Position::new(1, 1), 2);
    assert_eq!(secret.field.buffer.selection(), Some(0..7));

    // Pointer selection can be switched off; disabled ignores the pointer.
    let mut frozen = TextInput::new(id("tp.frozen"), "Name")
        .edit_entry(EditEntry::Explicit)
        .pointer_select(PointerSelect::OFF)
        .value("hello world");
    text_input_frame(&mut frozen, 40, false);
    assert_eq!(frozen.on_drag(Position::new(5, 1)), Outcome::Ignored);
    assert_eq!(frozen.field.buffer.selection(), None);
    let mut disabled = TextInput::new(id("tp.off"), "Name")
        .value("abc")
        .disabled(true);
    text_input_frame(&mut disabled, 40, false);
    assert_eq!(disabled.on_click(Position::new(2, 1)), Outcome::Consumed);
    assert_eq!(disabled.on_press(Position::new(2, 1)), Outcome::Consumed);
    assert!(!disabled.editing);

    // The context menu opens on a secondary click only when enabled.
    for (enabled, opens) in [(true, true), (false, false)] {
        let mut field = TextInput::new(id("tp.menu"), "Name")
            .edit_entry(EditEntry::Explicit)
            .context_menu(enabled)
            .value("copy me");
        let frame = text_input_frame(&mut field, 40, false);
        let ev = PointerEvent::new(
            &frame.hits,
            Phase::Secondary,
            field.id,
            Position::new(3, 1),
            1,
            KeyModifiers::NONE,
        );
        let _ = field.pointer(&ev);
        assert_eq!(field.edit_menu.is_open(), opens, "context_menu({enabled})");
        if opens {
            assert_ne!(
                field.hints(),
                vec![EDIT_HINT],
                "an open menu reports its own hints"
            );
        }
    }
}

fn modded(code: KeyCode, mods: KeyModifiers) -> Key {
    Key { code, mods }
}

fn text_area_frame(area: &mut TextArea, focus: bool) -> Ringed {
    let interaction = Interaction {
        focus: focus.then_some(area.id),
        ..Interaction::default()
    };
    paint_ring(Rect::new(0, 0, 40, 8), interaction, |a, buf, ctx| {
        area.render(a, buf, ctx, ctx.theme.canvas)
    })
}

fn fields_text_area_builders() {
    let base = |name: &str| TextArea::new(id(name), "Notes", 3).edit_entry(EditEntry::Explicit);

    // Geometry: the body rows plus the label and message rows.
    assert_eq!(base("ta.h").height(), 5);
    assert_eq!(TextArea::new(id("ta.h"), "Notes", 6).height(), 8);

    // Static content: value, placeholder, help, error.
    let mut empty = base("ta.ph").placeholder("Write something");
    assert_eq!(empty.placeholder, "Write something");
    assert!(text_of(&text_area_frame(&mut empty, false).buf).contains("Write something"));
    let mut filled = base("ta.val")
        .value("first\nsecond")
        .placeholder("Write something");
    let text = text_of(&text_area_frame(&mut filled, false).buf);
    assert!(text.contains("first") && text.contains("second") && !text.contains("Write something"));
    assert_eq!(
        filled.field.buffer.cursor_offset(),
        0,
        "value leaves the caret at the start"
    );
    let mut helped = base("ta.help").help("Markdown is fine");
    assert!(text_of(&text_area_frame(&mut helped, false).buf).contains("Markdown is fine"));
    let mut failed = base("ta.err")
        .help("hidden by the error")
        .error(Some("Too long"));
    assert_eq!(failed.field.error_message(), Some("Too long"));
    let text = text_of(&text_area_frame(&mut failed, false).buf);
    assert!(text.contains("Too long") && !text.contains("hidden by the error"));
    assert_eq!(
        base("ta.err")
            .error(Some("x"))
            .error(None)
            .field
            .error_message(),
        None
    );

    // Entry policy and what the field wants while idle.
    assert_eq!(base("ta.entry").entry(), EditEntry::Explicit);
    assert!(!base("ta.entry").wants_text_input());
    assert!(
        base("ta.entry")
            .edit_entry(EditEntry::TypeToEdit)
            .wants_text_input()
    );
    assert!(
        !base("ta.entry")
            .disabled(true)
            .edit_entry(EditEntry::TypeToEdit)
            .wants_text_input()
    );
    let mut idle = base("ta.entry");
    idle.begin_edit();
    assert!(idle.wants_text_input() && idle.editing);
    idle.commit();
    assert!(!idle.editing);
    let mut off = base("ta.off").disabled(true);
    off.begin_edit();
    assert!(!off.editing, "a disabled area cannot be edited");
    assert!(off.disabled);

    // Hints follow the mode.
    let mut hints = base("ta.hints");
    assert_eq!(hints.hints().len(), 2);
    assert_eq!(hints.hints()[0], EDIT_HINT);
    hints.begin_edit();
    assert_eq!(hints.hints().len(), 2);
    assert_eq!(hints.hints()[1].action, "Done");
    let mut reverting = base("ta.hints").esc_reverts(true);
    assert!(reverting.esc_reverts);
    reverting.begin_edit();
    assert_eq!(reverting.hints()[1].action, "Cancel");
    assert!(base("ta.hints").disabled(true).hints().is_empty());

    // Configuration lands on the field.
    assert_eq!(
        base("ta.cfg").field_keys(FieldKeys::Emacs).field.keys,
        FieldKeys::Emacs
    );
    assert_eq!(
        base("ta.cfg")
            .text_keymap(vec![(KeyBinding::ctrl('q'), TextCmd::Undo)])
            .field
            .text_cmd(&ctrl('q')),
        Some(TextCmd::Undo)
    );
    assert_eq!(
        base("ta.cfg").pointer_select(PointerSelect::OFF).field.sel,
        PointerSelect::OFF
    );
    assert_eq!(
        base("ta.cfg").blur_action(BlurAction::Keep).blur_action,
        BlurAction::Keep
    );
    assert_eq!(
        base("ta.cfg")
            .focus_select(FocusSelect::CaretStart)
            .focus_select,
        FocusSelect::CaretStart
    );
    assert!(base("ta.cfg").context_menu(true).edit_menu.enabled);
    assert!(!base("ta.cfg").context_menu(false).edit_menu.enabled);
    assert_eq!(base("ta.cfg").nav(NavKeys::VIM).nav, NavKeys::VIM);
    let narrow = HScroll {
        enabled: false,
        ..HScroll::default()
    };
    let mut scrolled = base("ta.cfg")
        .horizontal(narrow)
        .value("a very long line that needs scrolling sideways in the box");
    text_area_frame(&mut scrolled, false);
    assert_eq!(scrolled.horizontal, Some(narrow));
    assert!(!scrolled.scroll.h_cfg.enabled);

    // label_hit decides whether the label is a click target.
    let label_regions = |area: &mut TextArea| {
        text_area_frame(area, false)
            .hits
            .regions()
            .iter()
            .filter(|r| r.id == area.id && r.area.y == 0)
            .count()
    };
    assert_eq!(label_regions(&mut base("ta.lh")), 1);
    assert_eq!(label_regions(&mut base("ta.lh").label_hit(false)), 0);

    // The pointer policy decides whether a drag selects.
    for (sel, selects) in [(PointerSelect::DEFAULT, true), (PointerSelect::OFF, false)] {
        let mut area = base("ta.ptr").pointer_select(sel).value("hello world");
        text_area_frame(&mut area, false);
        area.on_press(Position::new(4, 1));
        area.on_drag(Position::new(9, 1));
        assert_eq!(area.field.buffer.selection().is_some(), selects, "{sel:?}");
    }

    // The context menu opens on a secondary click only when enabled.
    for (enabled, opens) in [(true, true), (false, false)] {
        let mut area = base("ta.menu").context_menu(enabled).value("copy me");
        let frame = text_area_frame(&mut area, false);
        let ev = PointerEvent::new(
            &frame.hits,
            Phase::Secondary,
            area.id,
            Position::new(4, 1),
            1,
            KeyModifiers::NONE,
        );
        let _ = area.pointer(&ev);
        assert_eq!(area.edit_menu.is_open(), opens, "context_menu({enabled})");
    }

    // field_keys picks the chord that selects all, once the area is painted.
    for (keys, chord, selects) in [
        (FieldKeys::Emacs, 'l', true),
        (FieldKeys::Emacs, 'a', false),
        (FieldKeys::Desktop, 'a', true),
    ] {
        let mut area = base("ta.keys").field_keys(keys).value("hello");
        text_area_frame(&mut area, false);
        area.begin_edit();
        area.on_key(&ctrl(chord));
        assert_eq!(
            area.field.buffer.selection().is_some(),
            selects,
            "{keys:?} ctrl+{chord}"
        );
    }
}

fn fields_text_area_editing() {
    let base = |name: &str| {
        TextArea::new(id(name), "Notes", 3)
            .edit_entry(EditEntry::Explicit)
            .value("one\ntwo\nthree\nfour\nfive")
    };

    // Explicit entry: Enter opens, Enter inserts a newline, Esc commits.
    let mut area = base("te.one");
    assert_eq!(
        area.on_key(&key(KeyCode::Char('x'))),
        (Outcome::Ignored, None)
    );
    assert_eq!(area.on_key(&key(KeyCode::Enter)), (Outcome::Changed, None));
    assert!(area.editing);
    assert_eq!(
        area.on_key(&key(KeyCode::Char('A'))),
        (Outcome::Changed, Some(InputAction::Changed))
    );
    assert_eq!(
        area.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(InputAction::Changed))
    );
    assert_eq!(area.field.text(), "A\none\ntwo\nthree\nfour\nfive");
    assert!(area.is_dirty());
    assert_eq!(
        area.on_key(&key(KeyCode::Esc)),
        (Outcome::Changed, Some(InputAction::Committed))
    );
    assert!(!area.editing && !area.is_dirty());

    // Idle navigation scrolls, vim keys only when asked.
    let mut tall = base("te.nav");
    text_area_frame(&mut tall, false);
    assert_eq!(tall.on_key(&key(KeyCode::Down)), (Outcome::Changed, None));
    assert_eq!(tall.scroll.offset, 1);
    assert_eq!(
        tall.on_key(&key(KeyCode::Char('j'))),
        (Outcome::Ignored, None)
    );
    let mut vim = base("te.nav").nav(NavKeys::VIM);
    text_area_frame(&mut vim, false);
    assert_eq!(
        vim.on_key(&key(KeyCode::Char('j'))),
        (Outcome::Changed, None)
    );
    assert_eq!(vim.scroll.offset, 1);
    assert_eq!(
        vim.on_key(&key(KeyCode::Char('k'))),
        (Outcome::Changed, None)
    );
    assert_eq!(vim.scroll.offset, 0);

    // Type-to-edit begins on a character; esc_reverts restores the snapshot.
    let mut reverting = base("te.rev")
        .edit_entry(EditEntry::TypeToEdit)
        .esc_reverts(true);
    assert_eq!(
        reverting.on_key(&key(KeyCode::Char('Z'))),
        (Outcome::Changed, Some(InputAction::Changed))
    );
    assert!(reverting.is_dirty());
    assert_eq!(
        reverting.on_key(&key(KeyCode::Esc)),
        (Outcome::Changed, Some(InputAction::Cancelled))
    );
    assert_eq!(reverting.field.text(), "one\ntwo\nthree\nfour\nfive");
    reverting.begin_edit();
    assert_eq!(
        reverting.on_key(&key(KeyCode::Esc)),
        (Outcome::Changed, Some(InputAction::Committed)),
        "an untouched session commits rather than cancels"
    );

    // Tab leaves in both directions and asks for the next field.
    let mut leaving = base("te.tab");
    leaving.begin_edit();
    assert_eq!(
        leaving.on_key(&key(KeyCode::Tab)),
        (
            Outcome::Changed,
            Some(InputAction::CommittedTab { backward: false })
        )
    );
    leaving.begin_edit();
    assert_eq!(
        leaving.on_key(&key(KeyCode::BackTab)),
        (
            Outcome::Changed,
            Some(InputAction::CommittedTab { backward: true })
        )
    );

    // Disabled fields ignore keys and pastes.
    let mut off = base("te.off").disabled(true);
    assert_eq!(off.on_key(&key(KeyCode::Enter)), (Outcome::Ignored, None));
    assert_eq!(off.on_paste("x"), Outcome::Ignored);

    // Paste: ignored while idle and explicit, inserted while editing,
    // starts an edit for type-to-edit fields.
    let mut paste = base("te.paste");
    assert_eq!(paste.on_paste("x"), Outcome::Ignored);
    paste.begin_edit();
    assert_eq!(paste.on_paste("X"), Outcome::Changed);
    assert!(paste.field.text().contains('X'));
    let mut quick = base("te.paste").edit_entry(EditEntry::TypeToEdit);
    assert_eq!(quick.on_paste("Q"), Outcome::Changed);
    assert!(quick.editing);

    // Blur follows the configured action.
    for (action, expect, text, editing) in [
        (
            BlurAction::Commit,
            Some(InputAction::Committed),
            "typed",
            false,
        ),
        (
            BlurAction::Cancel,
            Some(InputAction::Cancelled),
            "one\ntwo\nthree\nfour\nfive",
            false,
        ),
        (BlurAction::Keep, None, "typed", true),
    ] {
        let mut field = base("te.blur").blur_action(action);
        assert_eq!(field.on_blur(), None, "{action:?} while idle");
        field.begin_edit();
        field.field.set_text("typed");
        assert_eq!(field.on_blur(), expect, "{action:?}");
        assert_eq!(
            (field.field.text(), field.editing),
            (text, editing),
            "{action:?}"
        );
    }
    let mut clean = base("te.clean").edit_entry(EditEntry::Immediate);
    clean.begin_edit();
    assert_eq!(clean.on_blur(), None);
    assert!(!clean.editing, "an untouched immediate field just closes");

    // Focus selection applies when an immediate field first takes focus.
    for (select, selection, cursor) in [
        (FocusSelect::SelectAll, Some(0..23), 23),
        (FocusSelect::CaretStart, None, 0),
        (FocusSelect::CaretEnd, None, 23),
    ] {
        let mut focused = base("te.focus")
            .value("one\ntwo\nthree\nfour\nfive")
            .edit_entry(EditEntry::Immediate)
            .focus_select(select);
        text_area_frame(&mut focused, true);
        assert!(focused.editing, "{select:?}");
        assert_eq!(focused.field.buffer.selection(), selection, "{select:?}");
        assert_eq!(focused.field.buffer.cursor_offset(), cursor, "{select:?}");
    }

    // Pointer: render, then press, drag and click select text.
    let mut pointer = base("te.ptr").value("hello big world\nsecond line");
    text_area_frame(&mut pointer, false);
    let body = pointer.area;
    assert_eq!(body.y, 1);
    let at = |col: u16, row: u16| Position::new(body.x + 2 + col, body.y + row);
    assert_eq!(pointer.on_press(at(2, 0)), Outcome::Consumed);
    assert_eq!(pointer.on_drag(at(7, 0)), Outcome::Changed);
    assert_eq!(pointer.field.buffer.selection(), Some(2..7));
    assert_eq!(pointer.on_click(at(7, 0)), Outcome::Changed);
    assert_eq!(pointer.on_press_count(at(8, 0), 2), Outcome::Changed);
    assert_eq!(pointer.field.buffer.selection(), Some(6..9));
    assert_eq!(pointer.on_click_count(at(8, 0), 2), Outcome::Changed);
    pointer.on_click(at(3, 1));
    assert_eq!(pointer.field.buffer.selection(), None);
    assert_eq!(pointer.field.buffer.cursor_offset(), 16 + 3);
    assert_eq!(
        pointer.on_click(Position::new(body.x + 3, 0)),
        Outcome::Changed,
        "the label row focuses the field"
    );
    let mut disabled = base("te.dptr").disabled(true);
    text_area_frame(&mut disabled, false);
    assert_eq!(disabled.on_click(at(1, 0)), Outcome::Consumed);

    // Rendering shows the cursor line counter only while editing and focused.
    let mut live = base("te.live");
    live.begin_edit();
    let text = text_of(&text_area_frame(&mut live, true).buf);
    assert!(
        text.contains("ln 1/5"),
        "editing shows the line counter: {text}"
    );
    let mut idle = base("te.idle");
    assert!(!text_of(&text_area_frame(&mut idle, false).buf).contains("ln 1/5"));
}

fn paragraph_blocks(text: &str) -> Vec<std::ops::Range<usize>> {
    let mut blocks = Vec::new();
    let (mut start, mut end, mut offset) = (None, 0, 0);
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches('\n');
        if body.is_empty() {
            if let Some(from) = start.take() {
                blocks.push(from..end);
            }
        } else {
            start.get_or_insert(offset);
            end = offset + body.len();
        }
        offset += line.len();
    }
    if let Some(from) = start {
        blocks.push(from..end);
    }
    blocks
}

const SCRIPT: &str = "alpha = 1;\n\nbeta = 2;\n\ngamma = 3;";

fn code_frame(editor: &mut Editor, focus: bool) -> Ringed {
    let interaction = Interaction {
        focus: focus.then_some(editor.id),
        ..Interaction::default()
    };
    paint_ring(Rect::new(0, 0, 50, 8), interaction, |a, buf, ctx| {
        editor.render(a, buf, ctx, ctx.theme.canvas)
    })
}

fn script_editor(name: &str) -> Editor {
    Editor::new(id(name), SCRIPT)
        .edit_entry(EditEntry::Explicit)
        .segmenter(paragraph_blocks)
}

fn fields_code_editor_keys() {
    // Idle keys: block navigation and the vim-style entry keys.
    let mut editor = script_editor("ce.nav");
    assert_eq!(editor.blocks(), vec![0..10, 12..21, 23..33]);
    assert_eq!(editor.current_block(), Some(0..10));
    assert_eq!(
        editor.selection_or_block(),
        Some(("alpha = 1;".to_owned(), 0..10))
    );
    assert_eq!(
        editor.on_key(&key(KeyCode::Char('}'))),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    assert_eq!(editor.cursor_offset(), 12);
    assert_eq!(editor.current_block(), Some(12..21));
    assert_eq!(
        editor.on_key(&key(KeyCode::Char('}'))),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    assert_eq!(editor.cursor_offset(), 23);
    assert_eq!(
        editor.on_key(&key(KeyCode::Char('{'))),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    assert_eq!(editor.cursor_offset(), 12);
    editor.jump_to(0);
    assert_eq!(editor.cursor_offset(), 0);
    assert_eq!(
        editor.on_key(&key(KeyCode::Down)),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    assert_eq!(
        editor.on_key(&key(KeyCode::Home)),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    assert_eq!(editor.cursor_offset(), 0);
    assert_eq!(
        editor.on_key(&key(KeyCode::End)),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    assert_eq!(editor.cursor_offset(), SCRIPT.len());
    assert_eq!(
        editor.on_key(&key(KeyCode::Up)),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    assert_eq!(
        editor.on_key(&key(KeyCode::Char('z'))),
        (Outcome::Ignored, None)
    );
    assert!(!editor.wants_text_input());
    assert_eq!(editor.hints().len(), 2);
    assert_eq!(editor.hints()[0], EDIT_HINT);

    // Edit starts on Enter, `i` and `a`; `a` also steps past the caret.
    for (code, step) in [
        (KeyCode::Enter, 0),
        (KeyCode::Char('i'), 0),
        (KeyCode::Char('a'), 1),
    ] {
        let mut e = script_editor("ce.edit");
        assert_eq!(e.on_key(&key(code)), (Outcome::Changed, None));
        assert!(e.editing && e.wants_text_input());
        assert_eq!(e.cursor_offset(), step, "{code:?}");
    }

    // A remapped keymap replaces the defaults.
    let mut remapped = script_editor("ce.map").keymap(vec![(KeyBinding::char('e'), CodeCmd::Edit)]);
    assert_eq!(remapped.keymap.len(), 1);
    assert_eq!(
        remapped.on_key(&key(KeyCode::Char('i'))),
        (Outcome::Ignored, None)
    );
    assert_eq!(
        remapped.on_key(&key(KeyCode::Char('e'))),
        (Outcome::Changed, None)
    );
    assert!(remapped.editing);

    // Editing: typing changes, cursor keys move, Esc commits.
    let mut editor = script_editor("ce.type");
    editor.begin_edit();
    code_frame(&mut editor, true);
    assert!(editor.editing);
    assert_eq!(
        editor.on_key(&key(KeyCode::Char('Z'))),
        (Outcome::Changed, Some(EditorAction::Changed))
    );
    assert!(editor.text().starts_with('Z'));
    assert_eq!(
        editor.on_key(&key(KeyCode::Right)),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    assert_eq!(
        editor.on_key(&key(KeyCode::PageDown)),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    assert_eq!(
        editor.on_key(&key(KeyCode::PageUp)),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    assert_eq!(editor.on_key(&key(KeyCode::F(9))), (Outcome::Ignored, None));
    assert_eq!(editor.hints().len(), 3);
    assert_eq!(
        editor.on_key(&key(KeyCode::Esc)),
        (Outcome::Changed, Some(EditorAction::Committed))
    );
    assert!(!editor.editing);
    editor.begin_edit();
    editor.field.buffer.select_all();
    editor.commit();
    assert!(!editor.editing && !editor.field.buffer.has_selection());

    // Tab indents by the indent width unless tab_leaves hands it to focus.
    let mut indent = script_editor("ce.tab");
    indent.begin_edit();
    assert_eq!(
        indent.on_key(&key(KeyCode::Tab)),
        (Outcome::Changed, Some(EditorAction::Changed))
    );
    assert!(indent.text().starts_with("  alpha"));
    let mut leaves = script_editor("ce.tab").tab_leaves(true);
    assert!(leaves.tab_leaves);
    leaves.begin_edit();
    assert_eq!(leaves.hints()[0].action, "Next field");
    assert_eq!(
        leaves.on_key(&key(KeyCode::Tab)),
        (
            Outcome::Changed,
            Some(EditorAction::Leave { backward: false })
        )
    );
    assert!(!leaves.editing);
    leaves.begin_edit();
    assert_eq!(
        leaves.on_key(&key(KeyCode::BackTab)),
        (
            Outcome::Changed,
            Some(EditorAction::Leave { backward: true })
        )
    );
    let mut f6 = script_editor("ce.f6");
    f6.begin_edit();
    assert_eq!(
        f6.on_key(&key(KeyCode::F(6))),
        (
            Outcome::Changed,
            Some(EditorAction::Leave { backward: false })
        )
    );
    f6.begin_edit();
    assert_eq!(
        f6.on_key(&modded(KeyCode::F(6), KeyModifiers::SHIFT)),
        (
            Outcome::Changed,
            Some(EditorAction::Leave { backward: true })
        )
    );
    f6.begin_edit();
    assert_eq!(
        f6.on_key(&modded(KeyCode::Tab, KeyModifiers::CONTROL)),
        (
            Outcome::Changed,
            Some(EditorAction::Leave { backward: false })
        )
    );

    // esc_reverts restores the text from before the session.
    let mut reverting = script_editor("ce.rev").esc_reverts(true);
    assert!(reverting.esc_reverts);
    reverting.begin_edit();
    assert_eq!(reverting.hints()[2].action, "Cancel");
    reverting.on_key(&key(KeyCode::Char('Q')));
    assert!(reverting.text().starts_with('Q'));
    assert_eq!(
        reverting.on_key(&key(KeyCode::Esc)),
        (Outcome::Changed, Some(EditorAction::Committed))
    );
    assert_eq!(reverting.text(), SCRIPT);

    // Read-only refuses to enter edit mode.
    let mut locked = script_editor("ce.ro").read_only(true);
    assert!(locked.read_only);
    locked.begin_edit();
    assert!(!locked.editing && !locked.wants_text_input());
    assert_eq!(
        locked.on_key(&key(KeyCode::Char('i'))),
        (Outcome::Changed, None)
    );
    assert!(!locked.editing);
    assert_eq!(locked.hints().len(), 1);
    assert_eq!(locked.on_paste("x"), Outcome::Ignored);

    // Paste inserts while editing only.
    let mut paste = script_editor("ce.paste");
    assert_eq!(paste.on_paste("x"), Outcome::Ignored);
    paste.begin_edit();
    assert_eq!(paste.on_paste("NEW "), Outcome::Changed);
    assert!(paste.text().starts_with("NEW alpha"));

    // Pointer: click moves the caret, drag selects, the selection becomes the run target.
    let mut pointer = script_editor("ce.ptr");
    code_frame(&mut pointer, false);
    let origin = pointer.area;
    let at = |col: u16, row: u16| Position::new(origin.x + 6 + col, origin.y + row);
    assert_eq!(pointer.on_click(at(3, 2)), Outcome::Changed);
    assert_eq!(pointer.cursor_offset(), 12 + 3);
    assert!(pointer.editing, "a click opens the editor");
    pointer.commit();
    code_frame(&mut pointer, true);
    PointerField::on_press_count(&mut pointer, at(2, 0), 1);
    assert_eq!(pointer.on_drag(at(7, 0)), Outcome::Changed);
    assert_eq!(
        pointer.selection_or_block().map(|(t, _)| t),
        Some("pha =".to_owned())
    );
    pointer.commit();
    code_frame(&mut pointer, true);
    assert_eq!(pointer.cursor_cell().map(|c| c.y), Some(origin.y));
    let cell = pointer.cursor_cell().expect("the caret is on screen");
    assert_eq!(cell.x, origin.x + 6 + (pointer.cursor_offset() as u16));
    pointer.jump_to(0);
    pointer.scroll.offset = 5;
    assert_eq!(
        pointer.cursor_cell(),
        None,
        "a caret outside the viewport has no cell"
    );
}

fn fields_code_editor_find() {
    let src = "let a = 1;\nlet b = a;\nlet c = a + b;";
    let mut editor = Editor::new(id("cf.one"), src).edit_entry(EditEntry::Explicit);
    assert!(editor.find_state().is_none());

    // `/` opens the find bar, typing fills it and jumps to the match.
    assert_eq!(
        editor.on_key(&key(KeyCode::Char('/'))),
        (Outcome::Changed, None)
    );
    assert!(editor.find_state().is_some_and(|f| f.editing));
    assert!(editor.wants_text_input());
    assert_eq!(editor.hints().len(), 2);
    assert_eq!(editor.hints()[0].action, "Find");
    for c in "let".chars() {
        assert_eq!(
            editor.on_key(&key(KeyCode::Char(c))),
            (Outcome::Changed, Some(EditorAction::CursorMoved))
        );
    }
    let find = editor.find_state().unwrap();
    assert_eq!(find.matches.len(), 3);
    assert_eq!(find.needle.text(), "let");
    // Enter stops editing the query and keeps the matches.
    assert_eq!(
        editor.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    assert!(!editor.find_state().unwrap().editing);
    assert!(!editor.wants_text_input());
    // n and N walk the matches, wrapping around.
    assert_eq!(
        editor.on_key(&key(KeyCode::Char('n'))),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    assert_eq!(editor.cursor_offset(), 11);
    assert_eq!(
        editor.on_key(&key(KeyCode::Char('n'))),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    assert_eq!(editor.cursor_offset(), 22);
    assert_eq!(
        editor.on_key(&key(KeyCode::Char('n'))),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    assert_eq!(editor.cursor_offset(), 0);
    assert_eq!(
        editor.on_key(&key(KeyCode::Char('N'))),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    assert_eq!(editor.cursor_offset(), 22);
    // Esc closes the bar.
    assert_eq!(editor.on_key(&key(KeyCode::Esc)), (Outcome::Changed, None));
    assert!(editor.find_state().is_none());

    // open_find reopens with an empty needle; pasting fills it.
    editor.open_find();
    assert!(
        editor
            .find_state()
            .is_some_and(|f| f.editing && f.matches.is_empty())
    );
    assert_eq!(editor.on_paste("b"), Outcome::Changed);
    assert_eq!(editor.find_state().unwrap().matches.len(), 2);
    assert_eq!(
        editor.on_key(&modded(KeyCode::Enter, KeyModifiers::SHIFT)),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    // Enter on an empty needle just leaves the query field.
    let mut empty = Editor::new(id("cf.two"), src).edit_entry(EditEntry::Explicit);
    empty.open_find();
    assert_eq!(
        empty.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(EditorAction::CursorMoved))
    );
    assert!(!empty.find_state().unwrap().editing);
    // Esc while typing the query closes the bar.
    empty.open_find();
    assert_eq!(empty.on_key(&key(KeyCode::Esc)), (Outcome::Changed, None));
    assert!(empty.find_state().is_none());

    // The bar renders the match counter.
    let mut shown = Editor::new(id("cf.three"), src).edit_entry(EditEntry::Explicit);
    shown.set_find_query("let");
    let text = text_of(&code_frame(&mut shown, false).buf);
    assert!(text.contains("1/3"), "{text}");
    shown.set_find_query("zzz");
    assert!(text_of(&code_frame(&mut shown, false).buf).contains("no matches"));
}

fn fields_code_editor_config() {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    // Content accessors.
    let mut editor = Editor::new(id("cc.one"), "");
    assert!(editor.is_empty());
    editor.set_text("one\ntwo");
    assert_eq!(editor.text(), "one\ntwo");
    assert!(!editor.is_empty());
    assert_eq!(editor.cursor_offset(), 0);

    // Placeholder shows only while empty.
    let mut hinted = Editor::new(id("cc.two"), "").placeholder("-- write a query");
    assert_eq!(hinted.placeholder, "-- write a query");
    assert!(text_of(&code_frame(&mut hinted, false).buf).contains("-- write a query"));
    let mut full = Editor::new(id("cc.two"), "select 1").placeholder("-- write a query");
    let text = text_of(&code_frame(&mut full, false).buf);
    assert!(text.contains("select 1") && !text.contains("-- write a query"));

    // A builder-set highlighter colours the keyword it finds.
    fn keyword_tones(text: &str) -> Vec<(std::ops::Range<usize>, junie_tui::theme::SyntaxTone)> {
        text.find("select")
            .map(|at| vec![(at..at + 6, junie_tui::theme::SyntaxTone::Keyword)])
            .unwrap_or_default()
    }
    let mut coloured = Editor::new(id("cc.hl"), "select 1").highlighter(keyword_tones);
    assert!(coloured.highlighter_fn().is_some());
    let frame = code_frame(&mut coloured, false);
    let row = frame.buf.area.y;
    let col = coloured.area.x + 6;
    assert_ne!(
        frame.buf[(col, row)].style().fg,
        frame.buf[(col + 7, row)].style().fg,
        "the keyword is painted in its own tone"
    );

    // Running marker and the status line numbers.
    let mut running = script_editor("cc.run");
    running.set_running(Some(12..21));
    assert_eq!(running.running, Some(12..21));
    let text = text_of(&code_frame(&mut running, true).buf);
    assert!(text.contains("beta"));
    let spinner = theme.glyphs.spinner_frame(0);
    assert!(
        text.contains(spinner),
        "the running block shows the spinner in its gutter"
    );
    running.set_running(None);
    assert_eq!(running.running, None);
    assert!(!text_of(&code_frame(&mut running, true).buf).contains(spinner));

    // Builders land on the editor.
    assert_eq!(
        Editor::new(id("cc.b"), "x").nav(NavKeys::VIM).nav,
        NavKeys::VIM
    );
    assert_eq!(
        Editor::new(id("cc.b"), "x")
            .field_keys(FieldKeys::Emacs)
            .field
            .keys,
        FieldKeys::Emacs
    );
    assert_eq!(
        Editor::new(id("cc.b"), "x")
            .text_keymap(vec![(KeyBinding::ctrl('q'), TextCmd::Undo)])
            .field
            .text_cmd(&ctrl('q')),
        Some(TextCmd::Undo)
    );
    assert_eq!(
        Editor::new(id("cc.b"), "x")
            .edit_entry(EditEntry::TypeToEdit)
            .entry(),
        EditEntry::TypeToEdit
    );
    assert!(
        Editor::new(id("cc.b"), "x")
            .context_menu(true)
            .edit_menu
            .enabled
    );
    assert!(
        !Editor::new(id("cc.b"), "x")
            .context_menu(false)
            .edit_menu
            .enabled
    );

    // field_keys picks the chord that selects all, once the editor is painted.
    for (keys, chord, selects) in [
        (FieldKeys::Emacs, 'l', true),
        (FieldKeys::Emacs, 'a', false),
        (FieldKeys::Desktop, 'a', true),
    ] {
        let mut keyed = Editor::new(id("cc.keys"), "hello")
            .edit_entry(EditEntry::Explicit)
            .field_keys(keys);
        code_frame(&mut keyed, true);
        keyed.begin_edit();
        keyed.on_key(&ctrl(chord));
        assert_eq!(
            keyed.field.buffer.selection().is_some(),
            selects,
            "{keys:?} ctrl+{chord}"
        );
    }

    // nav picks the keys that move the caret while the editor is idle.
    for (nav, moves) in [(NavKeys::VIM, true), (NavKeys::DEFAULT, false)] {
        let mut idle = Editor::new(id("cc.nav"), "one\ntwo")
            .edit_entry(EditEntry::Explicit)
            .nav(nav);
        code_frame(&mut idle, true);
        idle.on_key(&key(KeyCode::Char('j')));
        assert_eq!(idle.cursor_offset() == 4, moves, "{nav:?}");
    }

    // The scrollbar gap narrows the text beside a vertical bar.
    let tall: String = (0..40)
        .map(|i| format!("row {i} {}\n", "x".repeat(100)))
        .collect();
    let mut snug = Editor::new(id("cc.gap"), &tall)
        .edit_entry(EditEntry::Explicit)
        .scrollbar_gap(0);
    let mut roomy = Editor::new(id("cc.gap"), &tall)
        .edit_entry(EditEntry::Explicit)
        .scrollbar_gap(3);
    code_frame(&mut snug, false);
    code_frame(&mut roomy, false);
    assert!(
        roomy.hbar().viewport_len < snug.hbar().viewport_len,
        "a wider gap leaves less room for text"
    );

    // The horizontal bar appears for a wide line and follows the policy.
    let wide = "x".repeat(120);
    let mut scrolling = Editor::new(id("cc.h"), &wide)
        .edit_entry(EditEntry::Explicit)
        .scrollbar_gap(2);
    assert_eq!(scrolling.scrollbar_gap, 2);
    code_frame(&mut scrolling, false);
    assert!(scrolling.hbar().content_len > scrolling.hbar().viewport_len);
    assert_eq!(
        scrolling.on_key(&key(KeyCode::Right)),
        (Outcome::Changed, None)
    );
    assert!(scrolling.hscroll > 0, "right scrolls the view sideways");
    let off = HScroll {
        enabled: false,
        ..HScroll::default()
    };
    let mut plain = Editor::new(id("cc.h"), &wide).horizontal(off);
    assert_eq!(plain.horizontal, Some(off));
    code_frame(&mut plain, false);
    assert!(!plain.scroll.h_cfg.enabled);
    let tracked =
        Editor::new(id("cc.h"), "x").track_click(junie_tui::core::scroll::TrackClick::default());
    assert!(tracked.scroll.track_click.is_some());
    let jumping =
        Editor::new(id("cc.h"), "x").track_click(junie_tui::core::scroll::TrackClick::Jump);
    assert_eq!(
        jumping.scroll.v_click(),
        junie_tui::core::scroll::TrackClick::Jump,
        "the editor's own track click wins over the app default"
    );

    // A secondary click opens the context menu only when enabled.
    for (enabled, opens) in [(true, true), (false, false)] {
        let mut field = script_editor("cc.menu").context_menu(enabled);
        let frame = code_frame(&mut field, false);
        let ev = PointerEvent::new(
            &frame.hits,
            Phase::Secondary,
            field.id,
            Position::new(field.area.x + 8, field.area.y),
            1,
            KeyModifiers::NONE,
        );
        let _ = field.pointer(&ev);
        assert_eq!(field.edit_menu.is_open(), opens, "context_menu({enabled})");
        if opens {
            assert_ne!(field.hints(), script_editor("cc.menu").hints());
        }
    }
}

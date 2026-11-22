//! Lane `dialogs` of the #14 burn-down: this lane's callable scenarios.
//!
//! Each scenario builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. Register one `ScenarioCase` per
//! `ScenarioId` variant from the `lane:dialogs` block in `../coverage.rs`. Edit only
//! this file and `../coverage/lane_dialogs.rs`.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Margin, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use termrock::core::edit::EditEntry;
use termrock::core::event::{Key, Outcome};
use termrock::core::focus::{Focus, FocusFallback, FocusRing};
use termrock::core::hints::{HasHints, HintCx};
use termrock::core::hit::HitRegistry;
use termrock::core::id::{Part, PartRef, WidgetId};
use termrock::core::intent::Phase;
use termrock::core::keymap::{Actions, KeyBinding, NavKeys};
use termrock::core::pointer::{PointerConfig, PointerEvent, PointerState};
use termrock::theme::{ColorLevel, StatusKind, Strings, Theme, Tone};
use termrock::ui::ctx::{Backdrop, Interaction, RenderCtx};
use termrock::ui::leave::{
    KeyContext, Leave, LeaveGuard, LeaveKey, QuitKeys, Resolved, Stakes, discard_dialog,
    letters_allowed, running_dialog, unsaved_dialog,
};
#[allow(deprecated)]
use termrock::ui::modal::FramePlacement;
use termrock::ui::modal::{self, Dismissal, Frame, Modal, ModalStack, modal_frame};
use termrock::ui::popup::{Dismiss, Place, Placement, RefuseFeedback};
use termrock::widgets::button::{Button, ButtonRow, RowAlign};
use termrock::widgets::choice::{Checkbox, RadioGroup};
use termrock::widgets::dialog::{
    AckEnter, AckInput, Dialog, DialogBody, DialogCmd, DialogResult, OUTER_MARGIN, body_bounds,
};
use termrock::widgets::form_dialog::{
    FieldValue, FormAction, FormDialog, FormField, FormFieldKind, FormValues,
};
use termrock::widgets::icon_button::IconButton;
use termrock::widgets::input::TextInput;
use termrock::widgets::props::Prop;
use termrock::widgets::select::Select;

use super::{ScenarioCase, key, text_of};
use crate::coverage::ScenarioId;

#[rustfmt::skip]
pub const CASES: &[ScenarioCase] = &[
    ScenarioCase { id: ScenarioId::DialogsLeaveGuardFlow, run: leave_guard_flow },
    ScenarioCase { id: ScenarioId::DialogsModalStackLifecycle, run: modal_stack_lifecycle },
    ScenarioCase { id: ScenarioId::DialogsModalFrameChrome, run: modal_frame_chrome },
    ScenarioCase { id: ScenarioId::DialogsRenderCtxRegistration, run: render_ctx_registration },
    ScenarioCase { id: ScenarioId::DialogsInteractionVisualState, run: interaction_visual_state },
    ScenarioCase { id: ScenarioId::DialogsPromptEditing, run: dialog_prompt_editing },
    ScenarioCase { id: ScenarioId::DialogsFactsAck, run: dialog_facts_ack },
    ScenarioCase { id: ScenarioId::DialogsActionModel, run: dialog_action_model },
    ScenarioCase { id: ScenarioId::DialogsPointerInput, run: dialog_pointer_input },
    ScenarioCase { id: ScenarioId::DialogsFormModel, run: form_dialog_model },
    ScenarioCase { id: ScenarioId::DialogsFormInput, run: form_dialog_input },
    ScenarioCase { id: ScenarioId::DialogsInfoFlow, run: info_dialog_flow },
    ScenarioCase { id: ScenarioId::DialogsChoiceFlow, run: choice_dialog_flow },
    ScenarioCase { id: ScenarioId::DialogsLayerResults, run: layer_results_by_widget },
    ScenarioCase { id: ScenarioId::DialogsLayerSpecs, run: layer_spec_policies },
];

/// A frame painted into a seeded buffer, with the hit regions and focus ring
/// the draw registered.
struct Stage {
    buf: Buffer,
    hits: HitRegistry,
    ring: FocusRing,
}

fn seed_style() -> Style {
    Style::new()
        .fg(Color::Rgb(200, 200, 200))
        .bg(Color::Rgb(9, 9, 9))
}

fn stage(
    screen: Rect,
    interaction: Interaction,
    draw: impl FnOnce(Rect, &mut Buffer, &mut RenderCtx),
) -> Stage {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(screen);
    buf.set_style(screen, seed_style());
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        draw(screen, &mut buf, &mut ctx);
        ctx.flush_overlays(&mut buf);
    }
    Stage { buf, hits, ring }
}

fn ctrl(c: char) -> Key {
    Key {
        code: KeyCode::Char(c),
        mods: KeyModifiers::CONTROL,
    }
}

fn pressed_enter(dialog: &mut Dialog, on: WidgetId) {
    let mut focus = Focus::default();
    focus.set(Some(on));
    dialog.on_key(&key(KeyCode::Enter), &mut focus, &FocusRing::default());
}

fn body_text(dialog: &Dialog) -> String {
    match &dialog.body {
        DialogBody::Text(text) => text.clone(),
        other => panic!("a leave dialog carries a text body, got {other:?}"),
    }
}

fn leave_guard_flow() {
    let strings = Theme::for_level(ColorLevel::TrueColor).strings;
    let id = WidgetId::of("scenario.leave");

    assert!(Stakes::Clean.is_clean());
    assert!(!Stakes::Unsaved { changes: 1 }.is_clean());
    assert!(
        !Stakes::Running {
            what: "build".into()
        }
        .is_clean()
    );
    assert_eq!(Stakes::default(), Stakes::Clean);
    assert_eq!(
        Stakes::Unsaved { changes: 2 }.max(Stakes::Unsaved { changes: 3 }),
        Stakes::Unsaved { changes: 5 },
        "unsaved counts add up across sources"
    );
    assert_eq!(
        Stakes::Clean.max(Stakes::Unsaved { changes: 1 }),
        Stakes::Unsaved { changes: 1 }
    );
    let running = Stakes::Running {
        what: "the build".into(),
    };
    assert_eq!(
        Stakes::Unsaved { changes: 9 }.max(running.clone()),
        running,
        "a running job outranks unsaved edits"
    );
    assert_eq!(Stakes::Clean.max(Stakes::Clean), Stakes::Clean);

    let mut guard: LeaveGuard<&str> = LeaveGuard::new(id);
    assert!(guard.save, "a guard offers Save by default");
    assert!(guard.is_dialog(id));
    assert!(!guard.is_dialog(id.sub("other")));
    assert_eq!(guard.pending(), None);
    assert_eq!(
        guard
            .request(Leave::To("home"), &Stakes::Clean, &strings)
            .ok(),
        Some(Leave::To("home")),
        "a clean page leaves without a prompt"
    );
    assert_eq!(guard.pending(), None);

    let Err(mut dialog) = guard.request(
        Leave::To("settings"),
        &Stakes::Unsaved { changes: 3 },
        &strings,
    ) else {
        panic!("unsaved edits must prompt");
    };
    assert_eq!(guard.pending(), Some(&Leave::To("settings")));
    assert_eq!(dialog.title, strings.leave_unsaved_title);
    assert!(body_text(&dialog).contains("3 unsaved changes"));
    assert_eq!(dialog.actions.len(), 3, "Cancel, Discard, Save");
    assert_eq!(dialog.initial_focus, id.sub("save"));
    let screen = Rect::new(0, 0, 80, 24);
    let shown = stage(screen, Interaction::default(), |screen, buf, ctx| {
        dialog.render(screen, buf, ctx)
    });
    assert!(text_of(&shown.buf).contains("Unsaved changes"));
    pressed_enter(&mut dialog, id.sub("save"));
    assert_eq!(dialog.result, Some(DialogResult::Action(2)));
    assert_eq!(
        guard.resolve(dialog.result.expect("save resolved")),
        Resolved::Save(Leave::To("settings"))
    );
    assert_eq!(guard.pending(), None, "resolving clears the request");
    assert_eq!(
        guard.resolve(DialogResult::Action(2)),
        Resolved::Stay,
        "a result with nothing pending stays"
    );

    let Err(mut dialog) = guard.request_quit(&Stakes::Unsaved { changes: 1 }, &strings) else {
        panic!("unsaved edits must prompt");
    };
    assert!(body_text(&dialog).contains("1 unsaved change"));
    pressed_enter(&mut dialog, id.sub("discard"));
    assert_eq!(
        guard.resolve(dialog.result.expect("discard resolved")),
        Resolved::Discard(Leave::Quit)
    );
    let Err(_) = guard.request_quit(&Stakes::Unsaved { changes: 0 }, &strings) else {
        panic!("unsaved edits must prompt");
    };
    assert_eq!(
        guard.resolve(DialogResult::Action(0)),
        Resolved::Stay,
        "Cancel keeps the page"
    );

    let mut no_save: LeaveGuard<&str> = LeaveGuard::new(id).save(false);
    assert!(!no_save.save);
    let Err(mut dialog) =
        no_save.request(Leave::To("away"), &Stakes::Unsaved { changes: 2 }, &strings)
    else {
        panic!("unsaved edits must prompt");
    };
    assert_eq!(dialog.actions.len(), 2, "a guard without Save drops it");
    pressed_enter(&mut dialog, id.sub("ok"));
    assert_eq!(
        no_save.resolve(dialog.result.expect("discard resolved")),
        Resolved::Discard(Leave::To("away")),
        "the destructive confirm discards"
    );
    let Err(_) = no_save.request(Leave::Quit, &Stakes::Unsaved { changes: 2 }, &strings) else {
        panic!("unsaved edits must prompt");
    };
    assert_eq!(
        no_save.resolve(DialogResult::Action(2)),
        Resolved::Stay,
        "Save is not a valid answer when the guard has none"
    );

    let mut job: LeaveGuard<&str> = LeaveGuard::new(id);
    let Err(mut dialog) = job.request_quit(&running, &strings) else {
        panic!("a running job must prompt");
    };
    assert_eq!(dialog.title, strings.leave_running_title);
    assert!(body_text(&dialog).contains("the build"));
    assert_eq!(dialog.actions[0].label, strings.leave_stay);
    pressed_enter(&mut dialog, id.sub("ok"));
    assert_eq!(
        job.resolve(dialog.result.expect("stop resolved")),
        Resolved::Discard(Leave::Quit)
    );

    let direct = unsaved_dialog(id, 4, &strings);
    assert!(body_text(&direct).contains("4 unsaved changes"));
    assert_eq!(direct.cancel_index, Some(0));
    let direct = discard_dialog(id, 1, &strings);
    assert!(body_text(&direct).contains("1 unsaved change"));
    assert_eq!(direct.actions[1].label, strings.discard);
    let direct = running_dialog(id, "sync", &strings);
    assert!(body_text(&direct).contains("sync"));
    assert_eq!(direct.actions[1].label, strings.leave_stop);

    let keys = QuitKeys::DEFAULT;
    assert_eq!(QuitKeys::default(), keys);
    let plain = KeyContext::default();
    assert_eq!(keys.classify(&ctrl('c'), plain), Some(LeaveKey::Quit));
    assert_eq!(
        keys.classify(
            &ctrl('c'),
            KeyContext {
                selection: true,
                ..plain
            }
        ),
        Some(LeaveKey::Copy),
        "Ctrl+C copies while text is selected"
    );
    assert_eq!(keys.classify(&ctrl('q'), plain), Some(LeaveKey::Quit));
    assert_eq!(
        keys.classify(&key(KeyCode::Char('q')), plain),
        Some(LeaveKey::Quit)
    );
    let typing = KeyContext {
        text_focus: true,
        ..plain
    };
    let dirty = KeyContext {
        dirty: true,
        ..plain
    };
    assert!(letters_allowed(plain));
    assert!(!letters_allowed(typing), "a text field keeps its letters");
    assert!(!letters_allowed(dirty), "unsaved edits keep their letters");
    assert_eq!(keys.classify(&key(KeyCode::Char('q')), typing), None);
    assert_eq!(keys.classify(&key(KeyCode::Char('q')), dirty), None);
    assert_eq!(keys.classify(&key(KeyCode::Esc), plain), None);
    let esc = keys.esc_quits(true);
    assert_eq!(
        esc.classify(&key(KeyCode::Esc), plain),
        Some(LeaveKey::Quit)
    );
    assert_eq!(esc.classify(&key(KeyCode::Esc), typing), None);
    let custom = keys.letter(Some('x'));
    assert_eq!(
        custom.classify(&key(KeyCode::Char('x')), plain),
        Some(LeaveKey::Quit)
    );
    assert_eq!(custom.classify(&key(KeyCode::Char('q')), plain), None);
    assert_eq!(
        keys.letter(None).classify(&key(KeyCode::Char('q')), plain),
        None
    );
}

fn modal_stack_lifecycle() {
    let (a, b, c) = (
        WidgetId::of("scenario.stack.a"),
        WidgetId::of("scenario.stack.b"),
        WidgetId::of("scenario.stack.c"),
    );
    let dialog = |name: &str, dismiss: Dismiss| {
        Dialog::confirm(WidgetId::of("scenario.stack.dialog"), name, "Sure?", "Yes")
            .dismiss(dismiss)
    };
    let mut focus = Focus::default();
    focus.set(Some(a));
    let mut stack: ModalStack<Dialog> = ModalStack::new();
    assert!(stack.is_empty());
    assert_eq!(stack.len(), 0);
    assert!(stack.top().is_none());
    assert_eq!(stack.outside_click(), Dismissal::None);
    assert_eq!(stack.escape(), Dismissal::None);
    assert!(
        stack.pop(&mut focus).is_none(),
        "popping nothing is a no-op"
    );

    stack.push(dialog("Lower", Dismiss::LIGHT), &mut focus, Some(b));
    assert_eq!(focus.current(), Some(b), "push moves focus into the modal");
    assert_eq!(stack.outside_click(), Dismissal::Close);
    assert_eq!(stack.escape(), Dismissal::Close);
    stack.push(dialog("Upper", Dismiss::NEVER), &mut focus, Some(c));
    assert_eq!(stack.len(), 2);
    assert_eq!(
        stack.outside_click(),
        Dismissal::Keep,
        "only the top modal's policy counts"
    );
    assert_eq!(stack.escape(), Dismissal::Keep);
    assert_eq!(Modal::dismiss(stack.top().expect("a top")), Dismiss::NEVER);
    assert_eq!(
        stack.iter().map(|d| d.title.as_str()).collect::<Vec<_>>(),
        ["Lower", "Upper"]
    );
    assert_eq!(
        stack.iter().next_back().map(|d| d.title.as_str()),
        Some("Upper")
    );
    for d in stack.iter_mut() {
        d.title.push('!');
    }
    stack.top_mut().expect("a top").title.push('?');
    assert_eq!(stack.top().map(|d| d.title.as_str()), Some("Upper!?"));

    let popped = stack.pop(&mut focus).expect("the upper modal");
    assert_eq!(popped.title, "Upper!?");
    assert_eq!(focus.current(), Some(b), "pop restores the opener's focus");
    assert_eq!(stack.len(), 1);

    stack.push(dialog("Again", Dismiss::ESCAPE), &mut focus, Some(c));
    assert_eq!(stack.outside_click(), Dismissal::Keep);
    assert_eq!(stack.escape(), Dismissal::Close);
    stack.clear(&mut focus);
    assert!(stack.is_empty());
    assert_eq!(
        focus.current(),
        Some(a),
        "clear returns focus to what the first modal saved"
    );
    stack.clear(&mut focus);
    assert_eq!(focus.current(), Some(a), "clearing an empty stack is quiet");

    stack.push(dialog("One", Dismiss::LIGHT), &mut focus, Some(b));
    stack.push(dialog("Two", Dismiss::LIGHT), &mut focus, Some(c));
    stack.retain(|d| d.title != "Two");
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.top().map(|d| d.title.as_str()), Some("One"));
    stack.retain(|_| true);
    assert_eq!(stack.len(), 1);
    stack.clear(&mut focus);

    focus.set(Some(a));
    stack.push(dialog("Keep", Dismiss::LIGHT), &mut focus, Some(b));
    stack.push(dialog("Drop", Dismiss::LIGHT), &mut focus, Some(c));
    stack.retain(|d| d.title != "Keep");
    assert_eq!(stack.len(), 1);
    stack.pop(&mut focus);
    assert_eq!(
        focus.current(),
        None,
        "a modal that outlived the one below it has nothing to restore"
    );

    let mut ring = FocusRing::default();
    ring.register(a);
    ring.register(b);
    focus.set(Some(c));
    modal::ensure_focus(&mut focus, &ring, FocusFallback::First);
    assert_eq!(
        focus.current(),
        Some(a),
        "a vanished focus falls back to the first stop"
    );
}

fn modal_frame_chrome() {
    let screen = Rect::new(0, 0, 80, 24);

    let area = Frame::new(40, 10).area(screen);
    assert_eq!((area.width, area.height), (40, 10));
    assert_eq!(area.x * 2 + area.width, screen.width, "centered by default");
    let grown = Frame::new(10, 2).min_size(30, 7).area(screen);
    assert_eq!((grown.width, grown.height), (30, 7), "min_size is a floor");
    assert_eq!(
        Frame::new(40, 60).area(screen).height,
        20,
        "a tall frame keeps the outer margin"
    );
    let chromed = Frame::new(40, 60).chrome(4, 4).area(screen);
    assert_eq!(chromed.height, 14, "reserved chrome rows shrink the room");
    assert!(chromed.y >= 4 && chromed.bottom() <= 20);
    let anchor = Rect::new(10, 2, 6, 1);
    let below = Frame::new(40, 10)
        .anchor(anchor, Placement::Below)
        .area(screen);
    assert_eq!((below.x, below.y), (10, 3), "an anchored frame opens below");
    let edge = Rect::new(70, 2, 8, 1);
    let kept = Frame::new(40, 10)
        .anchor(edge, Placement::Below)
        .fit(Place::DEFAULT)
        .area(screen);
    let flush = Frame::new(40, 10)
        .anchor(edge, Placement::Below)
        .fit(Place::FLUSH)
        .area(screen);
    assert_eq!(
        kept.right() + 1,
        screen.right(),
        "the default fit keeps a margin"
    );
    assert_eq!(
        flush.right(),
        screen.right(),
        "a flush fit reaches the edge"
    );
    #[allow(deprecated)]
    {
        assert_eq!(FramePlacement::default(), FramePlacement::Upper);
        assert_eq!(Placement::from(FramePlacement::Upper), Placement::Center);
        assert_eq!(Placement::from(FramePlacement::Middle), Placement::Center);
        assert_eq!(
            Frame::new(40, 10)
                .placement(FramePlacement::Middle)
                .area(screen),
            area
        );
    }
    assert_eq!(
        Frame::new(40, 10).placement(Placement::Center).area(screen),
        area
    );

    let mut frame_rects = None;
    let shown = stage(screen, Interaction::default(), |screen, buf, ctx| {
        frame_rects = Some(
            Frame::new(40, 10)
                .title("Settings")
                .meta(Some("draft"))
                .dismiss(Dismiss::LIGHT)
                .render(screen, buf, ctx),
        );
    });
    let (drawn, inner) = frame_rects.expect("the frame rendered");
    assert_eq!(drawn, area);
    assert_eq!(inner, drawn.inner(Margin::new(3, 1)), "default margin");
    let top: String = (drawn.x..drawn.right())
        .map(|x| shown.buf[(x, drawn.y)].symbol().to_owned())
        .collect();
    assert!(
        top.contains(" Settings "),
        "the title sits on the top border: {top}"
    );
    assert!(top.contains(" draft "), "the meta sits beside it: {top}");
    let region = |id: WidgetId| shown.hits.regions().iter().find(|r| r.id == id);
    assert_eq!(region(modal::BACKDROP).map(|r| r.area), Some(screen));
    assert!(
        region(modal::BACKDROP).is_some_and(|r| r.part == PartRef::of(Part::BACKDROP)),
        "the backdrop region is tagged as a backdrop"
    );
    assert_eq!(region(modal::SURFACE).map(|r| r.area), Some(drawn));
    assert!(
        region(modal::CLOSE).is_some(),
        "a light frame shows its close button"
    );
    assert_ne!(
        shown.buf[(0, 0)].style().bg,
        seed_style().bg,
        "the page behind is dimmed"
    );
    assert_eq!(
        shown.buf[(0, screen.bottom() - 1)].style().bg,
        seed_style().bg,
        "the default backdrop keeps the last row for the status bar"
    );

    let render = |frame: Frame<'_>| {
        stage(screen, Interaction::default(), |screen, buf, ctx| {
            frame.render(screen, buf, ctx);
        })
    };
    let quiet = render(Frame::new(40, 10).title("Quiet"));
    assert!(
        !quiet.hits.regions().iter().any(|r| r.id == modal::BACKDROP),
        "a frame with no dismiss policy registers no light-dismiss region"
    );
    assert!(!quiet.hits.regions().iter().any(|r| r.id == modal::CLOSE));
    let never = render(Frame::new(40, 10).dismiss(Dismiss::NEVER));
    assert!(never.hits.regions().iter().any(|r| r.id == modal::BACKDROP));
    assert!(
        !never.hits.regions().iter().any(|r| r.id == modal::CLOSE),
        "a never-dismissed frame has no close button"
    );
    let bare = render(Frame::new(40, 10).backdrop(false).dismiss(Dismiss::LIGHT));
    assert_eq!(
        bare.buf[(0, 0)].style().bg,
        seed_style().bg,
        "no dim without a backdrop"
    );
    assert!(!bare.hits.regions().iter().any(|r| r.id == modal::BACKDROP));
    let full = render(Frame::new(40, 10).dim(termrock::ui::ctx::Backdrop::FULL));
    assert_ne!(
        full.buf[(0, screen.bottom() - 1)].style().bg,
        seed_style().bg
    );
    let held = render(
        Frame::new(40, 10).dim(
            termrock::ui::ctx::Backdrop::DEFAULT
                .keep_top(2)
                .keep_bottom(3),
        ),
    );
    assert_eq!(
        held.buf[(0, 1)].style().bg,
        seed_style().bg,
        "kept top rows"
    );
    assert_ne!(held.buf[(0, 3)].style().bg, seed_style().bg);
    assert_eq!(
        held.buf[(0, screen.bottom() - 3)].style().bg,
        seed_style().bg
    );

    let margined = Frame::new(40, 10).margin(Margin::new(5, 2));
    let mut inner_rect = Rect::ZERO;
    stage(screen, Interaction::default(), |screen, buf, ctx| {
        inner_rect = margined.render(screen, buf, ctx).1;
    });
    assert_eq!((inner_rect.width, inner_rect.height), (30, 6));

    let narrow = render(
        Frame::new(24, 8)
            .title("Settings")
            .meta(Some("draft"))
            .dismiss(Dismiss::LIGHT),
    );
    assert!(
        !text_of(&narrow.buf).contains("draft"),
        "the meta yields to the title"
    );
    let plain = render(Frame::new(40, 10).title("Settings").meta(None));
    assert!(text_of(&plain.buf).contains("Settings"));

    let custom = IconButton::new(modal::CLOSE, "Q");
    let glyph = render(
        Frame::new(40, 10)
            .close_button(custom.clone())
            .dismiss(Dismiss::LIGHT),
    );
    assert!(
        text_of(&glyph.buf).contains('Q'),
        "a custom close glyph is drawn"
    );
    let borrowed = termrock::widgets::info_dialog::InfoDialog::new(
        WidgetId::of("scenario.frame.info"),
        "Server",
        vec![],
    )
    .close_button(custom)
    .dismiss(Dismiss::ESCAPE);
    let overlaid = render(Frame::new(40, 10).overlay(&borrowed));
    assert!(
        text_of(&overlaid.buf).contains('Q'),
        "the overlay lends its close button"
    );
    assert!(
        overlaid
            .hits
            .regions()
            .iter()
            .any(|r| r.id == modal::BACKDROP)
    );
    let nothing = termrock::widgets::info_dialog::InfoDialog::new(
        WidgetId::of("scenario.frame.info"),
        "Server",
        vec![],
    )
    .dismiss(Dismiss::NEVER);
    assert!(
        !render(Frame::new(40, 10).overlay(&nothing))
            .hits
            .regions()
            .iter()
            .any(|r| r.id == modal::CLOSE),
        "the overlay's policy decides the close button"
    );

    let helper = stage(screen, Interaction::default(), |screen, buf, ctx| {
        let (a, i) = modal_frame(
            screen,
            buf,
            ctx,
            40,
            10,
            "Settings",
            Some("draft"),
            Dismiss::LIGHT,
        );
        assert_eq!(a, area);
        assert_eq!(i, inner);
    });
    assert_eq!(
        text_of(&helper.buf),
        text_of(&shown.buf),
        "modal_frame is the Frame with a title"
    );

    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut buf = Buffer::empty(screen);
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    let host = Rect::new(10, 5, 30, 5);
    let hit = modal::close_button(host, &mut buf, &mut ctx, theme.surface_elevated);
    assert!(!hit.is_empty());
    assert_eq!(hit.width, modal::CLOSE_WIDTH);
    assert_eq!(
        hit.right(),
        host.right() - 2,
        "the close button hugs the right edge"
    );
    let tiny = modal::close_button(
        Rect::new(0, 0, 12, 5),
        &mut buf,
        &mut ctx,
        theme.surface_elevated,
    );
    assert!(tiny.is_empty(), "a too-narrow frame draws no close button");
    let glyphy = modal::close_button_with(
        &IconButton::new(modal::CLOSE, "Z"),
        host,
        &mut buf,
        &mut ctx,
        theme.surface_elevated,
    );
    assert!(!glyphy.is_empty());
    let calm = modal::border_style(&ctx, theme.surface_elevated);
    assert_eq!(calm.bg, Some(theme.surface_elevated));
    ctx.interaction.flash = Some(modal::SURFACE);
    let flashed = modal::border_style(&ctx, theme.surface_elevated);
    assert_eq!(
        flashed.fg,
        Some(theme.warning),
        "a refused click flashes the border"
    );
    assert!(flashed.add_modifier.contains(Modifier::BOLD));
    assert_ne!(flashed, calm);

    let mut pointer = PointerState::new(PointerConfig::default());
    assert!(!modal::refuse_outside_click(
        &mut pointer,
        Dismiss::LIGHT,
        100
    ));
    assert!(!pointer.has_flash(), "an allowed click is not refused");
    assert!(modal::refuse_outside_click(
        &mut pointer,
        Dismiss::ESCAPE,
        100
    ));
    assert_eq!(pointer.flashing(101), Some(modal::SURFACE));
    let mut silent = PointerState::new(PointerConfig::default());
    assert!(!modal::refuse_outside_click(
        &mut silent,
        Dismiss::ESCAPE.refuse_feedback(RefuseFeedback::None),
        100
    ));
    assert!(!silent.has_flash());
}

fn render_ctx_registration() {
    use termrock::core::hit::Axis;
    use termrock::core::shape::PointerShape;
    use termrock::ui::ctx::{Backdrop, fill, legible_on_fill};

    let theme = Theme::for_level(ColorLevel::TrueColor);
    let screen = Rect::new(0, 0, 40, 10);
    let (a, b, c, d, e) = (
        WidgetId::of("scenario.ctx.a"),
        WidgetId::of("scenario.ctx.b"),
        WidgetId::of("scenario.ctx.c"),
        WidgetId::of("scenario.ctx.d"),
        WidgetId::of("scenario.ctx.e"),
    );
    let (bar, owner, s1, s2) = (
        WidgetId::of("scenario.ctx.bar"),
        WidgetId::of("scenario.ctx.owner"),
        WidgetId::of("scenario.ctx.s1"),
        WidgetId::of("scenario.ctx.s2"),
    );
    let mut buf = Buffer::empty(screen);
    buf.set_style(screen, seed_style());
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);

    ctx.control(a, Rect::new(0, 0, 10, 1), false);
    ctx.control(b, Rect::new(0, 1, 10, 1), true);
    ctx.focus_stop(c);
    ctx.selection(a);
    ctx.clickable(d, Rect::new(0, 2, 10, 1));
    let label = ctx.register_part(a, PartRef::of(Part::TITLE), Rect::new(0, 3, 10, 1));
    assert_eq!(label, a.part(PartRef::of(Part::TITLE)));
    ctx.register_part_as(
        d.sub("icon"),
        a,
        PartRef::of(Part::ICON),
        Rect::new(0, 4, 4, 1),
    );
    ctx.pointer_shape(a, PointerShape::Pointer);
    ctx.clickable_under(e, screen);
    ctx.scrollbar(bar, Rect::new(39, 0, 1, 10), owner, Axis::Vertical);
    ctx.scrollbar_thumb(bar, Rect::new(39, 2, 1, 3), owner);
    ctx.scrollable(s1, Rect::new(0, 5, 20, 3));
    ctx.scrollable_under(s2, screen);
    ctx.autoscroll(Rect::new(0, 6, 10, 2));

    assert!(ctx.ring.contains(a) && ctx.ring.contains(c));
    assert!(
        !ctx.ring.contains(b),
        "a disabled control is clickable but not a focus stop"
    );
    assert!(ctx.ring.has_selection(a));
    assert_eq!(ctx.hits.hit(Position::new(0, 0)), Some(a));
    assert_eq!(ctx.hits.hit(Position::new(0, 1)), Some(b));
    assert_eq!(ctx.hits.hit(Position::new(0, 2)), Some(d));
    assert_eq!(ctx.hits.hit(Position::new(0, 3)), Some(label));
    assert_eq!(ctx.hits.hit(Position::new(1, 4)), Some(d.sub("icon")));
    assert_eq!(
        ctx.hits.hit(Position::new(30, 8)),
        Some(e),
        "a region registered under sits beneath the page's own"
    );
    assert_eq!(ctx.hits.hit(Position::new(5, 0)), Some(a));
    assert_eq!(
        ctx.hits.part_of(d.sub("icon")).map(|h| (h.owner, h.part)),
        Some((a, PartRef::of(Part::ICON)))
    );
    assert_eq!(ctx.hits.shape_of(a), Some(PointerShape::Pointer));
    assert_eq!(ctx.hits.bar(bar).map(|b| b.axis), Some(Axis::Vertical));
    assert!(ctx.hits.regions().iter().any(|r| {
        r.id == bar && r.part == PartRef::of(Part::THUMB) && r.area == Rect::new(39, 2, 1, 3)
    }));
    assert_eq!(ctx.hits.hit_scroll(Position::new(1, 6)), Some(s1));
    let scrollers: Vec<WidgetId> = ctx.hits.scrollers().into_iter().map(|(id, _)| id).collect();
    assert!(scrollers.contains(&s1) && scrollers.contains(&s2));
    assert_eq!(
        ctx.hits.autoscroll_at(Position::new(1, 6)),
        Some(Rect::new(0, 6, 10, 2))
    );

    let (hits_before, ring_before) = (ctx.hits.len(), ctx.ring.len());
    ctx.inert = true;
    ctx.control(WidgetId::of("scenario.ctx.ghost"), screen, false);
    ctx.focus_stop(WidgetId::of("scenario.ctx.ghost"));
    ctx.selection(WidgetId::of("scenario.ctx.ghost"));
    ctx.clickable(WidgetId::of("scenario.ctx.ghost"), screen);
    ctx.clickable_under(WidgetId::of("scenario.ctx.ghost"), screen);
    ctx.register_part(
        WidgetId::of("scenario.ctx.ghost"),
        PartRef::of(Part::ICON),
        screen,
    );
    ctx.scrollable(WidgetId::of("scenario.ctx.ghost"), screen);
    ctx.scrollable_under(WidgetId::of("scenario.ctx.ghost"), screen);
    ctx.autoscroll(Rect::new(30, 0, 5, 5));
    ctx.pointer_shape(WidgetId::of("scenario.ctx.ghost"), PointerShape::Text);
    ctx.scrollbar(
        WidgetId::of("scenario.ctx.ghost"),
        screen,
        owner,
        Axis::Horizontal,
    );
    ctx.set_cursor(Position::new(1, 1));
    assert_eq!(
        (ctx.hits.len(), ctx.ring.len()),
        (hits_before, ring_before),
        "an inert ctx registers nothing"
    );
    assert_eq!(ctx.cursor, None);
    assert_eq!(ctx.hits.autoscroll_at(Position::new(31, 1)), None);
    ctx.inert = false;
    ctx.set_cursor(Position::new(3, 3));
    assert_eq!(ctx.cursor, Some(Position::new(3, 3)));

    buf.set_string(0, 0, "page", seed_style());
    ctx.overlay(|buf, _| buf.set_string(0, 0, "OVER", seed_style()));
    buf.set_string(0, 0, "late", seed_style());
    assert!(
        text_of(&buf).starts_with("late"),
        "the page draws in call order"
    );
    ctx.flush_overlays(&mut buf);
    assert!(
        text_of(&buf).starts_with("OVER"),
        "an overlay lands on top whatever the draw order"
    );

    let clipped = Rect::new(0, 2, 5, 1);
    let kept = ctx.clip(&mut buf, clipped, |buf, ctx| {
        buf.set_string(0, 2, "abcdefghij", seed_style());
        ctx.clickable(WidgetId::of("scenario.ctx.clip"), Rect::new(0, 2, 10, 1));
        ctx.set_cursor(Position::new(9, 2));
        7
    });
    assert_eq!(kept, 7, "clip hands back what the draw returned");
    assert_eq!(&rows_of(&buf)[2][..10].trim_end(), &"abcde");
    assert_eq!(
        ctx.hits.hit(Position::new(2, 2)),
        Some(WidgetId::of("scenario.ctx.clip"))
    );
    assert_ne!(
        ctx.hits.hit(Position::new(7, 2)),
        Some(WidgetId::of("scenario.ctx.clip")),
        "hit regions are cut to the clip"
    );
    assert_eq!(
        ctx.cursor,
        Some(Position::new(3, 3)),
        "a cursor outside the clip is dropped"
    );

    let early = WidgetId::of("scenario.ctx.early");
    let late = WidgetId::of("scenario.ctx.late");
    let start = ctx.ring.len();
    let sorted = ctx.visual_order(|ctx| {
        ctx.control(late, Rect::new(0, 8, 5, 1), false);
        ctx.control(early, Rect::new(0, 7, 5, 1), false);
        "drawn"
    });
    assert_eq!(sorted, "drawn");
    assert_eq!(
        &ctx.ring.reachable()[start..],
        [early, late],
        "focus order follows the screen, not the draw order"
    );

    ctx.page_under_layer(true);
    assert!(ctx.interaction.focus_hidden);
    ctx.page_under_layer(false);
    assert!(!ctx.interaction.focus_hidden);
    ctx.page_under_layer(true);
    ctx.cover(&mut buf);
    assert!(
        ctx.interaction.focus_hidden,
        "cover leaves the page's focus hidden"
    );
    assert!(
        !ctx.ring.contains(a),
        "cover walls off the page's focus stops"
    );
    assert_eq!(
        ctx.hits.hit(Position::new(0, 0)),
        None,
        "and its hit regions"
    );
    ctx.control(
        WidgetId::of("scenario.ctx.above"),
        Rect::new(0, 0, 3, 1),
        false,
    );
    assert_eq!(
        ctx.hits.hit(Position::new(1, 0)),
        Some(WidgetId::of("scenario.ctx.above"))
    );

    let bar_glyph = theme.glyphs.focus_bar;
    buf.set_style(screen, seed_style());
    buf[(2, 3)].set_symbol(bar_glyph).set_fg(theme.focus);
    ctx.interaction.focus_hidden = false;
    ctx.modal_backdrop(&mut buf, screen);
    assert_ne!(
        buf[(5, 5)].style().bg,
        seed_style().bg,
        "the page is dimmed"
    );
    assert_eq!(
        buf[(5, 9)].style().bg,
        seed_style().bg,
        "the default backdrop keeps the status row"
    );
    assert_eq!(
        buf[(2, 3)].symbol(),
        " ",
        "a dimmed page shows no focus bar"
    );
    assert!(!ctx.interaction.focus_hidden && !ctx.inert);

    buf.set_style(screen, seed_style());
    ctx.modal_backdrop_with(&mut buf, screen, Backdrop::FULL);
    assert_ne!(
        buf[(5, 9)].style().bg,
        seed_style().bg,
        "a full backdrop dims every row"
    );
    buf.set_style(screen, seed_style());
    ctx.modal_backdrop_with(
        &mut buf,
        screen,
        Backdrop::DEFAULT.keep_top(3).keep_bottom(0),
    );
    assert_eq!(buf[(5, 2)].style().bg, seed_style().bg);
    assert_ne!(buf[(5, 3)].style().bg, seed_style().bg);
    assert_ne!(buf[(5, 9)].style().bg, seed_style().bg);

    ctx.overlay(|buf, _| buf.set_string(0, 5, "flush", seed_style()));
    ctx.begin_modal(&mut buf);
    assert!(
        text_of(&buf).contains("flush"),
        "begin_modal paints queued overlays before it walls the page off"
    );
    assert!(!ctx.interaction.focus_hidden && !ctx.inert);

    assert_eq!(Backdrop::default(), Backdrop::DEFAULT);
    assert_eq!(
        (Backdrop::DEFAULT.keep_top, Backdrop::DEFAULT.keep_bottom),
        (0, 1)
    );
    assert_eq!(
        (Backdrop::FULL.keep_top, Backdrop::FULL.keep_bottom),
        (0, 0)
    );
    assert_eq!(
        Backdrop::FULL.keep_top(2).keep_bottom(4),
        Backdrop {
            keep_top: 2,
            keep_bottom: 4
        }
    );

    let mut cells = Buffer::empty(Rect::new(0, 0, 6, 2));
    fill(
        &mut cells,
        Rect::new(1, 0, 3, 2),
        Style::new().bg(Color::Rgb(1, 2, 3)),
    );
    assert_eq!(cells[(1, 1)].bg, Color::Rgb(1, 2, 3));
    assert_ne!(
        cells[(0, 1)].bg,
        Color::Rgb(1, 2, 3),
        "fill stays in its area"
    );
    assert_eq!(cells[(2, 0)].symbol(), " ");

    let plane = theme.surface_elevated;
    let mut text = Buffer::empty(Rect::new(0, 0, 8, 1));
    text.set_string(
        0,
        0,
        "faint",
        Style::new()
            .fg(Color::Rgb(12, 12, 12))
            .bg(Color::Rgb(10, 10, 10)),
    );
    text.set_string(
        6,
        0,
        "ok",
        Style::new().fg(Color::Rgb(12, 12, 12)).bg(plane),
    );
    legible_on_fill(&mut text, Rect::new(0, 0, 8, 1), &theme, plane);
    assert_eq!(
        text[(0, 0)].fg,
        theme.legible(Color::Rgb(12, 12, 12), Color::Rgb(10, 10, 10)),
        "text on a fill is lifted to a legible colour"
    );
    assert_ne!(text[(0, 0)].fg, Color::Rgb(12, 12, 12));
    assert_eq!(
        text[(6, 0)].fg,
        Color::Rgb(12, 12, 12),
        "the plane's own cells stay"
    );
}

fn rows_of(buf: &Buffer) -> Vec<String> {
    super::rows(buf)
}

fn interaction_visual_state() {
    use termrock::theme::Kind;
    use termrock::ui::ctx::{FocusVisible, VisualState};

    let theme = Theme::for_level(ColorLevel::TrueColor);
    let (a, b, c, container) = (
        WidgetId::of("scenario.ix.a"),
        WidgetId::of("scenario.ix.b"),
        WidgetId::of("scenario.ix.c"),
        WidgetId::of("scenario.ix.container"),
    );
    let live = Interaction {
        focus: Some(a),
        hover: Some(b),
        pressed: Some(b),
        ..Default::default()
    };
    assert!(live.focused(a) && !live.focused(b));
    assert!(live.has_focus(a));
    assert_eq!(live.shown_focus(), Some(a));
    assert!(live.focused_under(a, false));
    assert!(
        !live.focused_under(a, true),
        "a page under a layer shows no focus"
    );
    assert_eq!(live.live_hover(), Some(b));
    assert!(live.hovered(b) && !live.hovered(a));
    assert!(live.hovered_any([a, c, b]) && !live.hovered_any([a, c]));
    assert!(
        live.pressed(b),
        "pressed counts while the pointer is still over it"
    );
    assert!(live.pressed_any([c, b]) && !live.pressed_any([a, c]));
    assert!(live.focus_visible(a, FocusVisible::Always));
    assert!(live.focus_visible(a, FocusVisible::KeyboardOnly));
    assert_eq!(FocusVisible::default(), FocusVisible::Always);

    let dragged_off = Interaction {
        hover: Some(c),
        ..live
    };
    assert!(
        !dragged_off.pressed(b),
        "pressing, then leaving, un-presses"
    );
    let hidden = Interaction {
        focus_hidden: true,
        ..live
    };
    assert!(!hidden.focused(a) && hidden.has_focus(a));
    assert_eq!(hidden.shown_focus(), None);
    assert!(!hidden.focus_visible(a, FocusVisible::Always));
    let by_pointer = Interaction {
        pointer_focus: true,
        ..live
    };
    assert!(by_pointer.focus_visible(a, FocusVisible::Always));
    assert!(
        !by_pointer.focus_visible(a, FocusVisible::KeyboardOnly),
        "a pointer-given focus is not drawn in keyboard-only mode"
    );
    let still = Interaction {
        hover_suppressed: true,
        ..live
    };
    assert_eq!(still.live_hover(), None);
    assert!(!still.hovered(b) && !still.hovered_any([b]));
    assert!(
        still.pressed(b),
        "suppressed hover does not cancel a held press"
    );
    let flashing = Interaction {
        flash: Some(container),
        ..Default::default()
    };
    assert!(flashing.flash_within(container) && !flashing.flash_within(a));
    assert!(flashing.pressed(container), "a flash reads as a press");
    assert!(flashing.pressed_any([a, container]));

    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let ctx = RenderCtx::new(&theme, live, &mut hits, &mut ring);
    assert_eq!(
        ctx.state(a),
        VisualState {
            focused: true,
            ..Default::default()
        }
    );
    assert_eq!(
        ctx.state(b),
        VisualState {
            hovered: true,
            pressed: true,
            ..Default::default()
        }
    );
    assert_eq!(ctx.state(c), VisualState::default());
    assert_eq!(
        ctx.chosen(false),
        theme.chosen_in(VisualState::default()),
        "a chosen value follows the theme"
    );
    let title_bg = theme.surface_elevated;
    assert_eq!(
        ctx.title_style(a, title_bg),
        theme
            .interactive(
                Kind::Title,
                VisualState::default(),
                theme.plane_of(title_bg)
            )
            .style,
        "a title never shows keyboard focus"
    );
    assert_eq!(
        ctx.title_style(b, title_bg),
        theme
            .interactive(
                Kind::Title,
                VisualState {
                    hovered: true,
                    pressed: true,
                    ..Default::default()
                },
                theme.plane_of(title_bg)
            )
            .style
    );
    drop(ctx);

    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let covered = Interaction {
        focus_hidden: true,
        flash: Some(container),
        ..live
    };
    let ctx = RenderCtx::new(&theme, covered, &mut hits, &mut ring);
    assert_eq!(
        ctx.state(a),
        VisualState {
            backdrop: true,
            ..Default::default()
        },
        "under a modal the state is marked as backdrop and loses focus"
    );
    assert_eq!(
        ctx.chosen(true),
        theme.chosen_in(VisualState {
            focused: true,
            backdrop: true,
            ..Default::default()
        })
    );
    let row = ctx.child_state(container, a, true);
    assert!(
        row.pressed,
        "the cursor row of a flashing container reads as pressed"
    );
    let other = ctx.child_state(container, a, false);
    assert!(!other.pressed, "other children do not");
    assert!(
        !ctx.child_state(c, a, true).pressed,
        "another container's flash is not ours"
    );
    assert!(
        ctx.child_state(container, b, false).pressed,
        "the child's own press stays"
    );
}

fn area_of(hits: &HitRegistry, id: WidgetId) -> Rect {
    hits.regions()
        .iter()
        .rev()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("the frame registered a hit region for {id:?}"))
        .area
}

fn center(area: Rect) -> Position {
    Position::new(area.x + area.width / 2, area.y + area.height / 2)
}

fn click(hits: &HitRegistry, id: WidgetId, count: u8) -> PointerEvent {
    PointerEvent::new(
        hits,
        Phase::Click,
        id,
        center(area_of(hits, id)),
        count,
        KeyModifiers::NONE,
    )
}

fn input_text(dialog: &Dialog) -> String {
    match &dialog.body {
        DialogBody::Input(input) => input.text().to_owned(),
        DialogBody::Facts { ack: Some(ack), .. } => ack.input.text().to_owned(),
        other => panic!("no text field in {other:?}"),
    }
}

fn draw_dialog(dialog: &mut Dialog, screen: Rect) -> Stage {
    stage(screen, Interaction::default(), |screen, buf, ctx| {
        dialog.render(screen, buf, ctx)
    })
}

fn dialog_prompt_editing() {
    let id = WidgetId::of("scenario.dialog.prompt");
    let screen = Rect::new(0, 0, 80, 24);
    let make = || {
        Dialog::prompt(
            id,
            "Rename",
            TextInput::new(id.sub("name"), "Name")
                .value("old")
                .required(true),
            "Rename",
        )
        .edit_entry(EditEntry::Immediate)
    };
    let mut dialog = make();
    assert!(!dialog.prompt_dirty());
    assert!(!dialog.is_editing());
    assert_eq!(dialog.initial_focus, id.sub("name"));
    let mut focus = Focus::default();
    focus.set(Some(dialog.initial_focus));
    let shown = draw_dialog(&mut dialog, screen);
    assert!(text_of(&shown.buf).contains("Rename"));
    assert!(text_of(&shown.buf).contains("old"));
    let ring = shown.ring;

    let hints = dialog.hints(Some(id.sub("name")));
    assert!(
        hints.iter().any(|h| h.key == "Esc"),
        "the field offers Esc while the dialog can be dismissed"
    );
    let outcome = dialog.on_key(&key(KeyCode::Char('x')), &mut focus, &ring);
    assert_ne!(outcome, Outcome::Ignored);
    assert_ne!(input_text(&dialog), "old", "typing edits the prompt");
    assert!(dialog.is_editing());
    assert!(dialog.prompt_dirty(), "a changed prompt is dirty");
    assert_eq!(
        dialog.on_paste("zz"),
        Outcome::Changed,
        "paste lands in the field"
    );
    assert!(input_text(&dialog).contains("zz"));
    assert_eq!(
        dialog.on_click_outside(),
        Outcome::Consumed,
        "a dirty prompt refuses a light dismiss"
    );
    assert_eq!(dialog.result, None);

    let mut open = make().guard_dirty_prompt(false);
    open.on_key(&key(KeyCode::Char('x')), &mut focus, &ring);
    assert!(open.prompt_dirty());
    open.on_click_outside();
    assert_eq!(
        open.result,
        Some(DialogResult::Action(0)),
        "without the guard an outside click cancels"
    );

    let mut clean = make();
    clean.on_click_outside();
    assert_eq!(
        clean.result,
        Some(DialogResult::Action(0)),
        "a clean prompt dismisses"
    );

    let change = termrock::core::focus::FocusChange {
        from: id.sub("name"),
        to: None,
    };
    assert_eq!(
        dialog.on_blur(change),
        Outcome::Changed,
        "blur commits the edit"
    );
    assert!(!dialog.is_editing());
    let elsewhere = termrock::core::focus::FocusChange {
        from: id.sub("other"),
        to: None,
    };
    assert_eq!(dialog.on_blur(elsewhere), Outcome::Ignored);

    let (input_area, cancel) = (area_of(&shown.hits, id.sub("name")), id.sub("cancel"));
    assert_ne!(
        dialog.on_press(id.sub("name"), input_area.as_position(), 1),
        Outcome::Ignored,
        "a press inside the field starts a selection"
    );
    assert_ne!(
        dialog.on_drag(
            id.sub("name"),
            Position::new(input_area.x + 2, input_area.y)
        ),
        Outcome::Ignored
    );
    assert_eq!(
        dialog.on_press(cancel, Position::new(0, 0), 1),
        Outcome::Ignored
    );
    assert_eq!(
        dialog.on_drag(WidgetId::of("scenario.dialog.nothing"), Position::new(0, 0)),
        Outcome::Consumed
    );
    assert_ne!(
        dialog.on_click(id.sub("name"), input_area.as_position(), &mut focus),
        Outcome::Ignored
    );
    assert_ne!(
        dialog.on_click_count(id.sub("name"), input_area.as_position(), 2, &mut focus),
        Outcome::Ignored,
        "a double click selects a word"
    );
    assert_eq!(focus.current(), Some(id.sub("name")));

    let mut required = Dialog::prompt(
        id,
        "Rename",
        TextInput::new(id.sub("name"), "Name")
            .value("")
            .required(true),
        "Rename",
    );
    let mut focus = Focus::default();
    focus.set(Some(id.sub("ok")));
    required.on_key(&key(KeyCode::Enter), &mut focus, &ring);
    assert_eq!(
        required.result, None,
        "an invalid prompt keeps the dialog open"
    );
    let shown = draw_dialog(&mut required, screen);
    assert!(
        text_of(&shown.buf).contains("Required"),
        "the field says what is wrong"
    );

    let mut accept = make();
    let mut focus = Focus::default();
    focus.set(Some(id.sub("name")));
    accept.on_key(&key(KeyCode::Char('!')), &mut focus, &ring);
    accept.on_key(&key(KeyCode::Enter), &mut focus, &ring);
    assert_eq!(
        accept.result,
        Some(DialogResult::Action(1)),
        "Enter in a valid prompt confirms"
    );
    assert!(input_text(&accept).contains('!'));

    let mut esc = make();
    let mut focus = Focus::default();
    focus.set(Some(id.sub("name")));
    esc.on_key(&key(KeyCode::Esc), &mut focus, &ring);
    assert_eq!(
        esc.result,
        Some(DialogResult::Action(0)),
        "Esc in an immediate prompt cancels"
    );
}

fn dialog_facts_ack() {
    let id = WidgetId::of("scenario.dialog.facts");
    let screen = Rect::new(0, 0, 100, 30);
    let facts = || {
        Dialog::facts(
            id,
            "Delete cluster",
            vec![Prop::new("Host", "db.internal"), Prop::new("User", "admin")],
            vec!["DROP DATABASE prod;".to_owned()],
            Some("prod"),
            Button::danger(id.sub("ok"), "Delete"),
        )
    };
    let mut dialog = facts();
    assert!(!dialog.armed(), "a typed confirmation starts unarmed");
    assert_eq!(dialog.confirm_index, Some(1));
    assert_eq!(dialog.initial_focus, id.sub("ack"));
    assert_eq!(
        dialog.dismiss,
        Dismiss::ESCAPE,
        "an acknowledged dialog refuses outside clicks"
    );
    let DialogBody::Facts {
        facts: rows,
        code,
        ack,
    } = &dialog.body
    else {
        panic!("a facts dialog carries a facts body");
    };
    assert_eq!((rows.len(), code.len()), (2, 1));
    let AckInput { token, input } = ack.clone().expect("an ack input");
    assert_eq!(token, "prod");
    assert_eq!(input.id, id.sub("ack"));

    let shown = draw_dialog(&mut dialog, screen);
    let text = text_of(&shown.buf);
    for needle in [
        "db.internal",
        "admin",
        "DROP DATABASE",
        "Type prod to confirm",
    ] {
        assert!(text.contains(needle), "the dialog shows {needle}");
    }
    let ring = shown.ring;
    let mut focus = Focus::default();
    focus.set(Some(id.sub("ack")));
    let hint_keys = |d: &Dialog| -> Vec<String> {
        d.hints(Some(id.sub("ack")))
            .iter()
            .map(|h| format!("{} {}", h.key, h.action))
            .collect()
    };
    assert_eq!(hint_keys(&dialog)[0], "Type \"prod\" to confirm");

    dialog.on_key(&key(KeyCode::Enter), &mut focus, &ring);
    assert_eq!(
        dialog.result, None,
        "Enter before the token matches refuses"
    );
    let shown = draw_dialog(&mut dialog, screen);
    assert!(text_of(&shown.buf).contains("Does not match"));
    for c in "prod".chars() {
        dialog.on_key(&key(KeyCode::Char(c)), &mut focus, &ring);
    }
    assert!(dialog.armed(), "typing the token arms the dialog");
    assert_eq!(hint_keys(&dialog)[0], "Enter Delete");
    let mut gated = dialog.clone();
    gated.on_key(&key(KeyCode::Enter), &mut focus, &ring);
    assert_eq!(gated.result, Some(DialogResult::Action(1)));

    let mut next = facts().ack_enter(AckEnter::NextField);
    assert_eq!(next.ack_enter, AckEnter::NextField);
    draw_dialog(&mut next, screen);
    for c in "prod".chars() {
        next.on_key(&key(KeyCode::Char(c)), &mut focus, &ring);
    }
    next.on_key(&key(KeyCode::Enter), &mut focus, &ring);
    assert_eq!(next.result, None, "Enter moves on instead of confirming");
    assert_ne!(focus.current(), Some(id.sub("ack")));
    assert!(
        next.hints(Some(id.sub("ack")))
            .iter()
            .any(|h| h.key == "Enter" && h.action == Strings::DEFAULT.hint_next),
        "NextField advertises Enter as Next"
    );
    assert_eq!(AckEnter::default(), AckEnter::Submit);

    let mut plain = Dialog::facts(
        id,
        "Server",
        vec![Prop::new("Host", "db")],
        vec![],
        None,
        Button::primary(id.sub("ok"), "Close"),
    );
    assert!(plain.armed(), "no token means always armed");
    assert_eq!(plain.confirm_index, None);
    assert_eq!(plain.dismiss, Dismiss::LIGHT);
    assert_eq!(plain.initial_focus, id.sub("cancel"));
    draw_dialog(&mut plain, screen);
    assert!(!plain.is_editing());

    let destructive = Dialog::destructive(id, "Delete", "It is gone for good.", "Delete");
    assert_eq!(destructive.initial_focus, id.sub("cancel"));
    let enter_on = |first: WidgetId| {
        let mut d = destructive.clone();
        let mut focus = Focus::default();
        focus.set(Some(first));
        d.on_key(&key(KeyCode::Enter), &mut focus, &ring);
        d.result
    };
    assert_eq!(
        enter_on(destructive.initial_focus),
        Some(DialogResult::Action(0)),
        "Enter on the safe default cancels"
    );
    assert_eq!(
        enter_on(id.sub("ok")),
        Some(DialogResult::Action(1)),
        "the destructive confirm needs its own focus"
    );
    assert_eq!(
        destructive.actions[1].kind,
        termrock::theme::ButtonKind::Danger
    );
    let danger_cancel = |guard: bool| {
        Dialog::confirm(id, "Discard", "Lose edits?", "Keep")
            .actions(
                vec![
                    Button::danger(id.sub("discard"), "Discard"),
                    Button::primary(id.sub("keep"), "Keep"),
                ],
                Some(0),
            )
            .guard_destructive_dismiss(guard)
    };
    let mut guarded = danger_cancel(true);
    assert_eq!(guarded.on_click_outside(), Outcome::Consumed);
    assert_eq!(
        guarded.result, None,
        "a destructive dismissal is not one click away"
    );
    let mut open = danger_cancel(false);
    open.on_click_outside();
    assert_eq!(open.result, Some(DialogResult::Action(0)));
}

fn dialog_action_model() {
    let id = WidgetId::of("scenario.dialog.actions");
    let screen = Rect::new(0, 0, 80, 24);
    let labels = ["Cancel", "Save", "Discard"];
    let buttons = |labels: &[&str]| -> Vec<Button> {
        labels
            .iter()
            .enumerate()
            .map(|(i, l)| Button::secondary(id.sub(&format!("b{i}")), l))
            .collect()
    };
    let build = |labels: &[&str]| {
        Dialog::confirm(id, "Close", "Save first?", "Yes").actions(buttons(labels), Some(0))
    };
    let dialog = build(&labels);
    assert_eq!(dialog.actions.len(), 3);
    assert_eq!(
        dialog.mnemonic_keys(),
        [Some(('c', 0)), Some(('s', 0)), Some(('d', 0))]
    );
    assert_eq!(
        build(&["Save", "Send"]).mnemonic_keys(),
        [Some(('s', 0)), Some(('e', 1))],
        "a taken letter falls through to the next free one"
    );
    let mut focus = Focus::default();
    let ring = FocusRing::default();
    let mut by_letter = build(&labels);
    by_letter.on_key(&key(KeyCode::Char('s')), &mut focus, &ring);
    assert_eq!(
        by_letter.result,
        Some(DialogResult::Action(1)),
        "a mnemonic activates its button"
    );
    let mut off = build(&labels).mnemonics(false);
    assert!(!off.mnemonics);
    off.on_key(&key(KeyCode::Char('s')), &mut focus, &ring);
    assert_eq!(off.result, None);
    let mut chord = build(&labels);
    chord.on_key(&ctrl('s'), &mut focus, &ring);
    assert_eq!(chord.result, None, "a chord is not a mnemonic");

    let mut keyed = build(&labels).accelerators(Some(('y', 'n')));
    assert_eq!(keyed.keymap.len(), 2);
    keyed.on_key(&key(KeyCode::Char('n')), &mut focus, &ring);
    assert_eq!(keyed.result, Some(DialogResult::Action(0)), "n cancels");
    let mut keyed = Dialog::confirm(id, "Close", "Sure?", "Yes").accelerators(Some(('y', 'n')));
    keyed.on_key(&key(KeyCode::Char('y')), &mut focus, &ring);
    assert_eq!(keyed.result, Some(DialogResult::Action(1)), "y confirms");
    assert!(
        Dialog::confirm(id, "Close", "Sure?", "Yes")
            .accelerators(Some(('y', 'n')))
            .hints(None)
            .iter()
            .any(|h| h.action == "Yes" && h.key != "Enter"),
        "the confirm accelerator is advertised"
    );
    assert!(
        Dialog::confirm(id, "Close", "Sure?", "Yes")
            .accelerators(None)
            .keymap
            .is_empty()
    );
    let custom = Dialog::confirm(id, "Close", "Sure?", "Yes")
        .mnemonics(false)
        .keymap(vec![(KeyBinding::char('k'), DialogCmd::Confirm)]);
    let mut custom = custom;
    custom.on_key(&key(KeyCode::Char('k')), &mut focus, &ring);
    assert_eq!(
        custom.result,
        Some(DialogResult::Action(1)),
        "a custom keymap confirms"
    );
    assert_eq!(DialogCmd::Confirm.label(), "Confirm");
    assert_eq!(DialogCmd::Cancel.label(), "Cancel");
    assert_ne!(DialogCmd::Confirm, DialogCmd::Cancel);
    assert!(DialogCmd::default_keymap().is_empty());

    #[allow(deprecated)]
    let legacy =
        Dialog::confirm(id, "Close", "Sure?", "Yes").with_actions(buttons(&["No", "Go"]), Some(0));
    assert_eq!(legacy.actions.len(), 2);
    assert_eq!(legacy.actions[1].label, "Go");

    let relabeled = Dialog::confirm(id, "Close", "Sure?", "Yes").cancel_label("Stay");
    assert_eq!(relabeled.actions[0].label, "Stay");
    let mut no_cancel = Dialog::confirm(id, "Close", "Sure?", "Yes")
        .actions(vec![Button::primary(id.sub("only"), "Got it")], None)
        .cancel_label("Ignored");
    assert_eq!(
        no_cancel.actions[0].label, "Got it",
        "no cancel button, nothing to relabel"
    );
    no_cancel.on_key(&key(KeyCode::Esc), &mut focus, &ring);
    assert_eq!(
        no_cancel.result,
        Some(DialogResult::Cancelled),
        "dismissing a dialog with no cancel button reports a bare cancel"
    );
    assert_eq!(
        Dialog::confirm(id, "Close", "Sure?", "Yes")
            .confirm_index(Some(1))
            .confirm_index,
        Some(1)
    );

    let status = Dialog::confirm(id, "Careful", "Sure?", "Yes").title_status(StatusKind::Warn);
    assert_eq!(status.title_status, Some(StatusKind::Warn));
    let mut plain = Dialog::confirm(id, "Careful", "Sure?", "Yes");
    let mut flagged = status;
    let (plain_stage, flagged_stage) = (
        draw_dialog(&mut plain, screen),
        draw_dialog(&mut flagged, screen),
    );
    let glyph = Theme::for_level(ColorLevel::TrueColor)
        .status(StatusKind::Warn)
        .0;
    assert!(
        text_of(&flagged_stage.buf).contains(glyph),
        "the status glyph leads the title"
    );
    assert!(!text_of(&plain_stage.buf).contains(glyph));

    let mut keys = build(&labels).nav(NavKeys::VIM);
    keys.on_key(&key(KeyCode::Right), &mut focus, &ring);
    let shown = draw_dialog(&mut keys, screen);
    focus.focus(id.sub("b0"));
    keys.on_key(&key(KeyCode::Char('l')), &mut focus, &shown.ring);
    assert_eq!(
        focus.current(),
        Some(id.sub("b1")),
        "vim navigation steps through the buttons"
    );
    let mut desk = build(&labels).nav(NavKeys::DEFAULT);
    let shown = draw_dialog(&mut desk, screen);
    focus.focus(id.sub("b0"));
    desk.on_key(&key(KeyCode::Char('l')), &mut focus, &shown.ring);
    assert_eq!(
        focus.current(),
        Some(id.sub("b0")),
        "plain letters do not navigate by default"
    );
    desk.on_key(&key(KeyCode::Right), &mut focus, &shown.ring);
    assert_eq!(focus.current(), Some(id.sub("b1")));

    let long = "A long explanation that keeps going. ".repeat(12);
    let make_long = || Dialog::confirm(id, "Long", &long, "Yes");
    assert!(
        make_long().height(30) > make_long().height(80),
        "narrow dialogs are taller"
    );
    let mut wide = make_long().width(70);
    let shown = draw_dialog(&mut wide, screen);
    assert_eq!(wide.area.width, 70);
    assert_eq!(
        wide.area,
        area_of(&shown.hits, id),
        "the dialog registers its surface"
    );
    assert_eq!(wide.area.height, wide.height(70).min(screen.height - 2));
    let mut short = make_long().max_height(Some(10));
    draw_dialog(&mut short, screen);
    assert!(short.area.height <= 10, "max_height caps the dialog");
    assert!(short.scroll.overflows(), "the body scrolls inside the cap");
    let mut pinned = make_long().max_height(Some(10)).body_scroll(false);
    assert!(!pinned.body_scroll);
    pinned.scroll.offset = 3;
    draw_dialog(&mut pinned, screen);
    assert_eq!(pinned.scroll.offset, 0, "a pinned body never scrolls");
    let mut roomy = make_long().width(200).outer_margin(Margin::new(10, 3));
    assert_eq!(OUTER_MARGIN, Margin::new(3, 1));
    draw_dialog(&mut roomy, screen);
    assert!(
        roomy.area.width <= screen.width - 20,
        "a bigger outer margin narrows the dialog"
    );
    let mut walled = make_long().chrome(4, 4).max_height(Some(30));
    draw_dialog(&mut walled, screen);
    assert!(walled.area.y >= 4 && walled.area.bottom() <= screen.bottom() - 4);
    assert_eq!(
        body_bounds(screen, 3, 2),
        Rect::new(0, 3, 80, 19),
        "chrome rows are carved off the screen"
    );
    assert_eq!(
        body_bounds(Rect::new(0, 0, 80, 2), 3, 2),
        Rect::new(0, 0, 80, 2)
    );

    let anchor = Rect::new(60, 2, 8, 1);
    let mut below = Dialog::confirm(id, "Menu", "Short.", "Yes")
        .width(30)
        .anchor(anchor, Placement::Below);
    draw_dialog(&mut below, screen);
    assert!(
        below.area.y > anchor.y,
        "an anchored dialog opens beside its anchor"
    );
    let mut shifted = Dialog::confirm(id, "Menu", "Short.", "Yes")
        .width(30)
        .placement(Placement::Center);
    draw_dialog(&mut shifted, screen);
    assert_eq!(shifted.area.x * 2 + shifted.area.width, screen.width);
    let full = {
        let mut d = Dialog::confirm(id, "Menu", "Short.", "Yes").backdrop(Backdrop::FULL);
        draw_dialog(&mut d, screen)
    };
    let default = {
        let mut d = Dialog::confirm(id, "Menu", "Short.", "Yes");
        draw_dialog(&mut d, screen)
    };
    assert_ne!(
        full.buf[(0, 23)].style().bg,
        seed_style().bg,
        "a full backdrop dims the last row"
    );
    assert_eq!(default.buf[(0, 23)].style().bg, seed_style().bg);

    let mut nested = Dialog::confirm(id, "Menu", "Short.", "Yes").width(30);
    let bounds = Rect::new(10, 5, 40, 12);
    let nested_stage = stage(screen, Interaction::default(), |screen, buf, ctx| {
        nested.render_in(screen, bounds, buf, ctx)
    });
    assert!(
        bounds.contains(nested.area.as_position()) && nested.area.right() <= bounds.right(),
        "render_in centers the dialog inside the bounds it is given"
    );
    assert!(text_of(&nested_stage.buf).contains("Menu"));

    let row_at = |row: ButtonRow| {
        let mut d = Dialog::confirm(id, "Menu", "Short.", "Yes").button_row(row);
        draw_dialog(&mut d, screen);
        d.actions[0].area
    };
    assert!(
        row_at(ButtonRow::new().align(RowAlign::Start)).x
            < row_at(ButtonRow::new().align(RowAlign::End)).x,
        "the button row's alignment is the dialog's to configure"
    );
}

fn dialog_pointer_input() {
    use termrock::widgets::scrollbar;

    let id = WidgetId::of("scenario.dialog.pointer");
    let screen = Rect::new(0, 0, 80, 24);
    let make = || Dialog::confirm(id, "Close", "Save first?", "Yes");

    let mut dialog = make();
    let shown = draw_dialog(&mut dialog, screen);
    let mut focus = Focus::default();
    let ok = area_of(&shown.hits, id.sub("ok"));
    assert_eq!(
        dialog.on_click(id.sub("cancel"), Position::new(0, 0), &mut focus),
        Outcome::Changed
    );
    assert_eq!(
        dialog.result,
        Some(DialogResult::Action(0)),
        "clicking Cancel cancels"
    );
    assert_eq!(
        focus.current(),
        Some(id.sub("cancel")),
        "a click focuses the button"
    );

    let mut dialog = make();
    draw_dialog(&mut dialog, screen);
    dialog.on_click_count(id.sub("ok"), ok.as_position(), 2, &mut focus);
    assert_eq!(
        dialog.result,
        Some(DialogResult::Action(1)),
        "clicking the confirm button confirms"
    );

    let mut dialog = make();
    let shown = draw_dialog(&mut dialog, screen);
    assert!(shown.hits.regions().iter().any(|r| r.id == modal::CLOSE));
    dialog.on_click(modal::CLOSE, Position::new(0, 0), &mut focus);
    assert_eq!(
        dialog.result,
        Some(DialogResult::Action(0)),
        "the close button dismisses to cancel"
    );

    let mut glyph = make().close_button(IconButton::new(modal::CLOSE, "Q"));
    assert!(
        text_of(&draw_dialog(&mut glyph, screen).buf).contains('Q'),
        "a custom close glyph is drawn"
    );
    let mut never = make().dismiss(Dismiss::NEVER);
    let shown = draw_dialog(&mut never, screen);
    assert!(
        !shown.hits.regions().iter().any(|r| r.id == modal::CLOSE),
        "no close button to click"
    );
    never.on_click(modal::CLOSE, Position::new(0, 0), &mut focus);
    assert_eq!(
        never.result, None,
        "a stale close click does nothing when dismissal is off"
    );

    let mut routed = make();
    let shown = draw_dialog(&mut routed, screen);
    let response = routed.pointer_in(&click(&shown.hits, id.sub("ok"), 1), &mut focus);
    assert_eq!(response.outcome(), Outcome::Changed);
    assert_eq!(
        routed.result,
        Some(DialogResult::Action(1)),
        "a routed click confirms"
    );
    let wheel = PointerEvent::wheel_on(
        &shown.hits,
        id,
        termrock::core::hit::Axis::Vertical,
        1,
        Position::new(40, 12),
        KeyModifiers::NONE,
    );
    let mut routed = make();
    draw_dialog(&mut routed, screen);
    let _ = routed.pointer_in(&wheel, &mut focus);
    assert_eq!(routed.result, None, "wheeling a short body does nothing");

    let long = "line of text. ".repeat(80);
    let mut scrolling = Dialog::confirm(id, "Long", &long, "Yes").max_height(Some(12));
    let shown = draw_dialog(&mut scrolling, screen);
    assert!(scrolling.scroll.overflows());
    let bar = scrollbar::id_for(id);
    let track = shown
        .hits
        .bar(bar)
        .expect("a scrolling body registers its bar")
        .track;
    assert_eq!(scrolling.scroll.offset, 0);
    let past_thumb = Position::new(track.x, track.bottom() - 1);
    assert_ne!(
        scrolling.on_click(bar, past_thumb, &mut focus),
        Outcome::Ignored
    );
    assert!(
        scrolling.scroll.offset > 0,
        "clicking the track pages the body"
    );
    let before = scrolling.scroll.offset;
    let _ = scrolling.on_drag(bar, Position::new(track.x, track.y));
    assert!(
        scrolling.scroll.offset <= before,
        "dragging the thumb toward the top scrolls back"
    );
    let wheel = PointerEvent::wheel_on(
        &shown.hits,
        id,
        termrock::core::hit::Axis::Vertical,
        3,
        Position::new(40, 10),
        KeyModifiers::NONE,
    );
    let before = scrolling.scroll.offset;
    let moved = scrolling.pointer_in(&wheel, &mut focus);
    assert!(moved.is_consumed() || scrolling.scroll.offset != before);
}

fn build_form(id: WidgetId) -> FormDialog {
    FormDialog::new(
        id,
        "Settings",
        vec![
            FormField::input(
                "name",
                TextInput::new(id.sub("name"), "Name").required(true),
            ),
            FormField::select(
                "kind",
                Select::new(id.sub("kind"), "Kind", &["Alpha", "Beta", "Gamma"], 0),
            ),
            FormField::check("live", Checkbox::new(id.sub("live"), "Live", false)),
            FormField::radio(
                "mode",
                RadioGroup::new(id.sub("mode"), "Mode", &["Fast", "Slow"], 0),
            ),
            FormField::chooser("folder", id.sub("folder"), "Folder", "/home", "Browse"),
            FormField::note("hint", vec![("Be careful".to_owned(), Tone::Warning)]),
        ],
    )
}

fn draw_form(form: &mut FormDialog, screen: Rect, focus: &Focus) -> Stage {
    let interaction = Interaction {
        focus: focus.current(),
        ..Default::default()
    };
    stage(screen, interaction, |screen, buf, ctx| {
        form.render(screen, buf, ctx)
    })
}

fn form_dialog_model() {
    let id = WidgetId::of("scenario.form.model");
    let screen = Rect::new(0, 0, 100, 40);
    use termrock::widgets::list::{ListBox, ListItem, SelectMode};

    let choices = ListBox::new(
        id.sub("replicas"),
        vec![ListItem::new("Primary"), ListItem::new("Secondary")],
        SelectMode::Single,
    );
    let mut inline = FormDialog::new(
        id,
        "Regions",
        vec![
            FormField::note(
                "help",
                vec![(
                    "Choose the available replica closest to your deployment region".into(),
                    Tone::Secondary,
                )],
            )
            .wrap(true),
            FormField::list("replicas", choices, 2),
        ],
    )
    .width(28);
    let rendered = draw_form(&mut inline, screen, &Focus::default());
    let rows = rows_of(&rendered.buf);
    assert!(
        rows.iter()
            .any(|line| line.contains("region") && !line.contains("Choose"))
    );
    assert!(rows.iter().any(|line| line.contains("Secondary")));
    assert!(inline.field("help").is_some_and(|f| f.wrap));
    assert!(
        matches!(inline.field("replicas").map(|f| &f.kind), Some(FormFieldKind::List { rows: 2, list }) if list.area.height == 2)
    );
    assert_eq!(inline.choice("replicas"), 0);
    let mut form = build_form(id).meta("draft").width(60);
    assert_eq!((form.width, form.meta.as_deref()), (60, Some("draft")));

    let values: FormValues = form.values();
    assert_eq!(
        values,
        vec![
            ("name".to_owned(), FieldValue::Text(String::new())),
            ("kind".to_owned(), FieldValue::Choice(0)),
            ("live".to_owned(), FieldValue::Bool(false)),
            ("mode".to_owned(), FieldValue::Choice(0)),
            ("folder".to_owned(), FieldValue::Text("/home".to_owned())),
            ("hint".to_owned(), FieldValue::Text(String::new())),
        ],
        "every field reports its value in its own shape"
    );
    assert!(matches!(
        form.field("name").map(|f| &f.kind),
        Some(FormFieldKind::Input(_))
    ));
    assert!(matches!(
        form.field("kind").map(|f| &f.kind),
        Some(FormFieldKind::Select(_))
    ));
    assert!(matches!(
        form.field("live").map(|f| &f.kind),
        Some(FormFieldKind::Check(_))
    ));
    assert!(matches!(
        form.field("mode").map(|f| &f.kind),
        Some(FormFieldKind::Radio(_))
    ));
    assert!(matches!(
        form.field("folder").map(|f| &f.kind),
        Some(FormFieldKind::Chooser { .. })
    ));
    assert!(matches!(
        form.field("hint").map(|f| &f.kind),
        Some(FormFieldKind::Note(_))
    ));
    assert!(form.field("missing").is_none());
    assert_eq!(
        form.field("live").map(FormField::value),
        Some(FieldValue::Bool(false))
    );

    if let Some(FormFieldKind::Input(input)) = form.field_mut("name").map(|f| &mut f.kind) {
        input.set_value("Ada");
    }
    assert_eq!(form.text("name"), "Ada");
    assert_eq!(form.choice("kind"), 0);
    assert_eq!(
        (form.text("missing"), form.choice("missing")),
        (String::new(), 0)
    );
    assert_eq!(form.text("live"), "", "a check is not text");
    form.set_text("name", "Bob");
    form.set_text("folder", "/tmp");
    form.set_text("live", "ignored");
    assert_eq!(
        (form.text("name"), form.text("folder")),
        ("Bob".into(), "/tmp".into())
    );
    form.set_chooser("folder", "/srv", Some("3 files"));
    form.set_chooser("name", "ignored", None);
    assert_eq!(form.text("folder"), "/srv");
    form.set_note("hint", vec![("Heads up".to_owned(), Tone::Danger)]);
    form.set_note("name", vec![("ignored".to_owned(), Tone::Normal)]);
    assert_eq!(form.text("name"), "Bob");

    let focus = Focus::default();
    let shown = draw_form(&mut form, screen, &focus);
    let text = text_of(&shown.buf);
    for needle in [
        "Settings", "draft", "Name", "Kind", "Live", "Mode", "Folder", "Browse", "/srv", "3 files",
        "Heads up", "Cancel", "Save",
    ] {
        assert!(text.contains(needle), "the form shows {needle}");
    }
    assert!(!text.contains("Be careful"), "set_note replaced the note");
    for id in [
        id.sub("name"),
        id.sub("kind"),
        id.sub("live"),
        id.sub("folder"),
        id.sub("cancel"),
        id.sub("save"),
    ] {
        area_of(&shown.hits, id);
    }
    area_of(
        &shown.hits,
        RadioGroup::new(id.sub("mode"), "", &["Fast", "Slow"], 0).option_id(1),
    );
    assert_eq!(form.area, area_of(&shown.hits, modal::SURFACE));

    assert_eq!(form.initial_focus(), id.sub("name"));
    assert_eq!(form.restored_focus(Some(id.sub("kind"))), id.sub("kind"));
    assert!(!form.is_editing());
    if let Some(FormFieldKind::Input(input)) = form.field_mut("name").map(|f| &mut f.kind) {
        input.begin_edit();
    }
    assert!(
        form.is_editing(),
        "an input mid-edit makes the form editing"
    );
    form.set_visible("name", false);
    assert!(!form.is_editing(), "a hidden input stops editing");
    assert_eq!(form.restored_focus(Some(id.sub("name"))), id.sub("kind"));
    assert_eq!(
        form.initial_focus(),
        id.sub("kind"),
        "focus starts at the first visible field"
    );
    assert_eq!(form.text("name"), "Bob", "a hidden field keeps its value");
    let shown = draw_form(&mut form, screen, &focus);
    assert!(
        !text_of(&shown.buf).contains("Name"),
        "a hidden field is not drawn"
    );
    assert!(!shown.hits.regions().iter().any(|r| r.id == id.sub("name")));
    form.set_visible("name", true);
    assert!(text_of(&draw_form(&mut form, screen, &focus).buf).contains("Name"));
    form.set_visible("missing", false);

    let hidden = FormField::check("x", Checkbox::new(id.sub("x"), "Secret", true)).hidden();
    assert!(!hidden.visible);
    let mut only_hidden = FormDialog::new(id.sub("h"), "Hidden", vec![hidden]);
    assert_eq!(
        only_hidden.initial_focus(),
        id.sub("h").sub("save"),
        "with nothing visible, Save is first"
    );
    assert_eq!(
        only_hidden.values(),
        vec![("x".to_owned(), FieldValue::Bool(true))]
    );
    assert!(!text_of(&draw_form(&mut only_hidden, screen, &focus).buf).contains("Secret"));

    let mut tall = FormDialog::new(
        id.sub("tall"),
        "Long form",
        (0..24)
            .map(|i| FormField::note(&format!("n{i}"), vec![(format!("filler {i}"), Tone::Muted)]))
            .chain([FormField::note(
                "tail",
                vec![("tail marker".to_owned(), Tone::Normal)],
            )])
            .collect(),
    );
    let small = Rect::new(0, 0, 100, 16);
    assert!(!text_of(&draw_form(&mut tall, small, &focus).buf).contains("tail marker"));
    tall.reveal("tail");
    draw_form(&mut tall, small, &focus);
    assert!(
        text_of(&draw_form(&mut tall, small, &focus).buf).contains("tail marker"),
        "reveal scrolls the named field into view"
    );
    tall.reveal("filler");
    tall.reveal("n0");
    assert!(text_of(&draw_form(&mut tall, small, &focus).buf).contains("filler 0"));

    let bare = build_form(id).keep_open_on_save();
    assert!(!bare.pop_on_save);
    assert!(build_form(id).pop_on_save);
    let with_action =
        build_form(id).action("export", Button::secondary(id.sub("export"), "Export"));
    assert_eq!(with_action.actions.len(), 1);
    let mut with_action = with_action;
    assert!(text_of(&draw_form(&mut with_action, screen, &focus).buf).contains("Export"));
    let glyph = build_form(id).close_button(IconButton::new(modal::CLOSE, "Q"));
    let mut glyph = glyph.dismiss(Dismiss::ESCAPE);
    assert!(text_of(&draw_form(&mut glyph, screen, &focus).buf).contains('Q'));
    let mut never = build_form(id).dismiss(Dismiss::NEVER);
    let shown = draw_form(&mut never, screen, &focus);
    assert!(!shown.hits.regions().iter().any(|r| r.id == modal::CLOSE));
}

struct FormRig {
    form: FormDialog,
    focus: Focus,
    shown: Stage,
}

const FORM_SCREEN: Rect = Rect::new(0, 0, 100, 40);

impl FormRig {
    fn new(form: FormDialog) -> Self {
        let mut rig = Self {
            form,
            focus: Focus::default(),
            shown: stage(FORM_SCREEN, Interaction::default(), |_, _, _| {}),
        };
        rig.draw();
        rig
    }

    fn draw(&mut self) {
        self.shown = draw_form(&mut self.form, FORM_SCREEN, &self.focus);
    }

    fn press(&mut self, on: WidgetId, code: KeyCode) -> Outcome {
        self.focus.focus(on);
        let o = self
            .form
            .on_key(&key(code), &mut self.focus, &self.shown.ring);
        self.draw();
        o
    }

    fn click(&mut self, id: WidgetId) -> Outcome {
        self.draw();
        let at = center(area_of(&self.shown.hits, id));
        let o = self.form.on_click(id, at, &mut self.focus);
        self.draw();
        o
    }

    fn take(&mut self) -> Vec<FormAction> {
        std::mem::take(&mut self.form.events)
    }
}

fn form_dialog_input() {
    let id = WidgetId::of("scenario.form.input");
    let option_one = RadioGroup::new(id.sub("mode"), "", &["Fast", "Slow"], 0).option_id(1);
    let export = id.sub("export");
    let make = || build_form(id).action("export", Button::secondary(export, "Export"));
    let mut rig = FormRig::new(make());
    assert_eq!(rig.form.nav, NavKeys::NO_VIM);

    let o = rig.press(id.sub("name"), KeyCode::Char('A'));
    assert_ne!(o, Outcome::Ignored);
    assert!(rig.form.is_editing(), "typing starts an edit");
    assert!(rig.take().is_empty(), "nothing is reported mid-edit");
    let hints = HasHints::hints(&rig.form, &HintCx::new(Some(id.sub("name"))));
    assert_eq!(
        hints,
        termrock::widgets::keyhint::EDIT_FIELD.to_vec(),
        "an edit shows the edit hints"
    );
    rig.press(id.sub("name"), KeyCode::Enter);
    assert_eq!(
        rig.take(),
        [FormAction::Changed("name".into())],
        "committing reports the change"
    );
    assert!(rig.form.dirty);
    assert_eq!(rig.form.text("name"), "A");

    rig.press(id.sub("kind"), KeyCode::Char('b'));
    assert_eq!(rig.take(), [FormAction::Changed("kind".into())]);
    assert_eq!(
        rig.form.choice("kind"),
        1,
        "a letter picks the matching option"
    );

    rig.press(id.sub("live"), KeyCode::Char(' '));
    assert_eq!(rig.take(), [FormAction::Changed("live".into())]);
    assert_eq!(
        rig.form.field("live").map(FormField::value),
        Some(FieldValue::Bool(true))
    );

    rig.press(id.sub("mode"), KeyCode::Down);
    assert_eq!(rig.take(), [FormAction::Changed("mode".into())]);
    assert_eq!(rig.form.choice("mode"), 1);

    rig.press(id.sub("folder"), KeyCode::Enter);
    assert_eq!(rig.take(), [FormAction::Choose("folder".into())]);
    rig.press(export, KeyCode::Enter);
    assert_eq!(rig.take(), [FormAction::Action("export".into())]);
    rig.press(id.sub("cancel"), KeyCode::Enter);
    assert_eq!(rig.take(), [FormAction::Cancel]);
    rig.press(id.sub("save"), KeyCode::Enter);
    assert_eq!(rig.take(), [FormAction::Save], "a valid form saves");
    rig.form.on_blur(termrock::core::focus::FocusChange {
        from: id.sub("name"),
        to: Some(id.sub("save")),
    });
    let hints = HasHints::hints(&rig.form, &HintCx::new(Some(id.sub("save"))));
    assert_eq!(hints[0].key, "Tab");
    assert!(hints.iter().any(|h| h.key == "Esc"));

    let mut keep = FormRig::new(make().keep_open_on_save());
    keep.form.set_text("name", "x");
    keep.press(id.sub("save"), KeyCode::Enter);
    assert_eq!(
        keep.take(),
        [FormAction::Action("save".into())],
        "a kept-open form reports save as an action"
    );

    let mut invalid = FormRig::new(make());
    invalid.press(id.sub("save"), KeyCode::Enter);
    assert!(invalid.take().is_empty(), "an invalid form does not save");
    assert_eq!(
        invalid.focus.current(),
        Some(id.sub("name")),
        "focus goes to the first problem"
    );
    assert!(text_of(&invalid.shown.buf).contains("Required"));

    let mut esc = FormRig::new(make());
    esc.press(id.sub("live"), KeyCode::Esc);
    assert_eq!(
        esc.take(),
        [FormAction::Cancel],
        "Esc cancels through the dismiss policy"
    );
    let mut stuck = FormRig::new(make().dismiss(Dismiss::NEVER));
    assert_eq!(stuck.press(id.sub("live"), KeyCode::Esc), Outcome::Consumed);
    assert!(
        stuck.take().is_empty(),
        "a never-dismissed form ignores Esc"
    );
    let hints = HasHints::hints(&stuck.form, &HintCx::new(None));
    assert!(
        !hints.iter().any(|h| h.key == "Esc"),
        "and does not advertise it"
    );

    let mut tab = FormRig::new(make());
    tab.focus.set(None);
    tab.form
        .on_key(&key(KeyCode::Tab), &mut tab.focus, &tab.shown.ring);
    assert_eq!(
        tab.focus.current(),
        tab.shown.ring.first(),
        "Tab enters the ring"
    );
    let first = tab.focus.current();
    tab.form
        .on_key(&key(KeyCode::BackTab), &mut tab.focus, &tab.shown.ring);
    assert_ne!(tab.focus.current(), first, "Shift+Tab leaves it backwards");
    tab.focus.focus(id.sub("cancel"));
    tab.form
        .on_key(&key(KeyCode::Right), &mut tab.focus, &tab.shown.ring);
    assert_eq!(
        tab.focus.current(),
        Some(id.sub("save")),
        "arrows step across the button row"
    );
    tab.form
        .on_key(&key(KeyCode::Enter), &mut tab.focus, &tab.shown.ring);
    assert!(tab.focus.current().is_some());

    let mut wrap = FormRig::new(make().button_row(ButtonRow::new().wrap(true)));
    wrap.press(id.sub("save"), KeyCode::Right);
    assert_eq!(
        wrap.focus.current(),
        Some(export),
        "a wrapping button row loops"
    );
    let mut flat = FormRig::new(make());
    flat.press(id.sub("save"), KeyCode::Right);
    assert_eq!(
        flat.focus.current(),
        Some(id.sub("save")),
        "the default row stops at its end"
    );
    let mut vim = FormRig::new(make().nav(NavKeys::VIM));
    vim.press(id.sub("cancel"), KeyCode::Char('l'));
    assert_eq!(
        vim.focus.current(),
        Some(id.sub("save")),
        "vim keys step the buttons when enabled"
    );
    let mut desk = FormRig::new(make());
    desk.press(id.sub("cancel"), KeyCode::Char('l'));
    assert_eq!(
        desk.focus.current(),
        Some(id.sub("cancel")),
        "and not by default"
    );

    let mut pointer = FormRig::new(make());
    pointer.click(id.sub("live"));
    assert_eq!(pointer.take(), [FormAction::Changed("live".into())]);
    assert_eq!(
        pointer.focus.current(),
        Some(id.sub("live")),
        "a click focuses the field"
    );
    pointer.click(option_one);
    assert_eq!(pointer.take(), [FormAction::Changed("mode".into())]);
    assert_eq!(pointer.form.choice("mode"), 1);
    pointer.click(id.sub("folder"));
    assert_eq!(pointer.take(), [FormAction::Choose("folder".into())]);
    pointer.click(export);
    assert_eq!(pointer.take(), [FormAction::Action("export".into())]);
    pointer.click(id.sub("kind"));
    pointer.draw();
    let at = center(area_of(&pointer.shown.hits, id.sub("kind")));
    assert_ne!(
        pointer
            .form
            .on_click_count(id.sub("name"), at, 2, &mut pointer.focus),
        Outcome::Ignored
    );
    pointer.click(id.sub("save"));
    assert!(pointer.take().is_empty(), "a click on Save validates first");
    pointer.form.set_text("name", "Zed");
    pointer.click(id.sub("save"));
    assert_eq!(pointer.take(), [FormAction::Save]);
    pointer.click(id.sub("cancel"));
    assert_eq!(pointer.take(), [FormAction::Cancel]);

    let mut routed = FormRig::new(make());
    let ev = click(&routed.shown.hits, id.sub("live"), 1);
    let response = routed.form.pointer_in(&ev, &mut routed.focus);
    assert_eq!(response.outcome(), Outcome::Changed);
    assert_eq!(routed.take(), [FormAction::Changed("live".into())]);
    let ev = click(&routed.shown.hits, id.sub("name"), 1);
    let _ = routed.form.pointer_in(&ev, &mut routed.focus);
    assert_eq!(
        routed.focus.current(),
        Some(id.sub("name")),
        "a routed click focuses an input"
    );
    let wheel = PointerEvent::wheel_on(
        &routed.shown.hits,
        id.sub("name"),
        termrock::core::hit::Axis::Vertical,
        1,
        Position::new(50, 20),
        KeyModifiers::NONE,
    );
    let _ = routed.form.pointer_in(&wheel, &mut routed.focus);
    assert!(routed.take().is_empty());

    let mut press = FormRig::new(make());
    let name_at = area_of(&press.shown.hits, id.sub("name")).as_position();
    assert_ne!(
        press.form.on_press(id.sub("name"), name_at, 1),
        Outcome::Ignored
    );
    assert_ne!(
        press
            .form
            .on_drag(id.sub("name"), Position::new(name_at.x + 3, name_at.y)),
        Outcome::Ignored
    );
    assert_eq!(
        press.form.on_press(export, name_at, 1),
        Outcome::Ignored,
        "buttons take no press"
    );
    assert_eq!(press.form.on_drag(export, name_at), Outcome::Ignored);
    assert_eq!(press.form.on_release(export, name_at), Outcome::Ignored);
    let kind_at = center(area_of(&press.shown.hits, id.sub("kind")));
    press.form.on_press(id.sub("kind"), kind_at, 1);
    press.form.on_drag(id.sub("kind"), kind_at);
    let _ = press.form.on_release(id.sub("kind"), kind_at);
    assert!(
        press.form.on_release(id.sub("kind"), kind_at) == Outcome::Ignored
            || press.form.events.len() <= 1,
        "releasing a select reports at most one change"
    );

    let mut paste = FormRig::new(make());
    paste.press(id.sub("name"), KeyCode::Char('Q'));
    assert_ne!(
        paste.form.on_paste("rs", &paste.focus),
        Outcome::Consumed,
        "paste lands in the edited field"
    );
    paste.press(id.sub("name"), KeyCode::Enter);
    assert_eq!(paste.form.text("name"), "Qrs");
    paste.take();
    paste.focus.focus(id.sub("live"));
    assert_eq!(
        paste.form.on_paste("ignored", &paste.focus),
        Outcome::Consumed,
        "paste with no edit is swallowed"
    );
    assert_eq!(paste.form.text("name"), "Qrs");

    let mut blur = FormRig::new(make());
    blur.press(id.sub("name"), KeyCode::Char('Z'));
    let away = termrock::core::focus::FocusChange {
        from: id.sub("name"),
        to: Some(id.sub("kind")),
    };
    assert_eq!(
        blur.form.on_blur(away),
        Outcome::Changed,
        "blur commits the edit"
    );
    assert_eq!(blur.take(), [FormAction::Changed("name".into())]);
    assert_eq!(blur.form.text("name"), "Z");
    assert_eq!(
        blur.form.on_blur(termrock::core::focus::FocusChange {
            from: id.sub("live"),
            to: None
        }),
        Outcome::Ignored
    );

    let mut vanish = FormRig::new(make());
    vanish.press(id.sub("name"), KeyCode::Char('H'));
    vanish.form.set_visible("name", false);
    vanish.press(id.sub("kind"), KeyCode::Down);
    assert_eq!(
        vanish.form.text("name"),
        "H",
        "hiding a field mid-edit keeps what was typed"
    );
    assert!(
        vanish.take().contains(&FormAction::Changed("name".into())),
        "and reports it"
    );
    vanish.focus.focus(id.sub("name"));
    vanish
        .form
        .on_key(&key(KeyCode::Tab), &mut vanish.focus, &vanish.shown.ring);
    assert_ne!(
        vanish.focus.current(),
        Some(id.sub("name")),
        "focus never rests on a hidden field"
    );
}

fn info_dialog_flow() {
    use termrock::core::keymap::KeyBinding as Bind;
    use termrock::widgets::info_dialog::{InfoCmd, InfoDialog, InfoResult};
    use termrock::widgets::scrollbar;

    let id = WidgetId::of("scenario.info");
    let screen = Rect::new(0, 0, 100, 40);
    let open = id.sub("open");
    let make = || {
        InfoDialog::new(
            id,
            "Server",
            vec![
                Prop::new("Host", "db.internal").copyable(),
                Prop::new("Port", "5432"),
            ],
        )
        .width(70)
        .meta("v1")
        .intro(vec![("Heads up".to_owned(), Tone::Warning)])
        .detail((0..30).map(|i| format!("detail line {i}")).collect())
        .action(Button::primary(open, "Open"))
    };
    let draw = |info: &mut InfoDialog| {
        stage(screen, Interaction::default(), |screen, buf, ctx| {
            info.render(screen, buf, ctx)
        })
    };

    let mut info = make();
    assert_eq!((info.width, info.meta.as_deref()), (70, Some("v1")));
    assert_eq!(
        info.initial_focus(),
        id.sub("props"),
        "focus starts on the properties"
    );
    assert_eq!(
        InfoDialog::new(id, "Empty", vec![]).initial_focus(),
        id.sub("close"),
        "with nothing to browse it starts on Close"
    );
    let shown = draw(&mut info);
    let text = text_of(&shown.buf);
    for needle in [
        "Server",
        "v1",
        "Heads up",
        "Host",
        "db.internal",
        "5432",
        "detail line 0",
        "Open",
        "Close",
    ] {
        assert!(text.contains(needle), "the dialog shows {needle}");
    }
    assert_eq!(info.area, area_of(&shown.hits, modal::SURFACE));
    assert!(!text.contains("! Server"));
    let mut failed = make().error();
    assert!(failed.error_title);
    assert!(
        text_of(&draw(&mut failed).buf).contains("! Server"),
        "an error dialog marks its title"
    );

    let mut focus = Focus::default();
    let ring = shown.ring;
    let mut press = |info: &mut InfoDialog, on: WidgetId, code: KeyCode| {
        focus.focus(on);
        info.on_key(&key(code), &mut focus, &ring)
    };
    let mut copy = make();
    press(&mut copy, id.sub("props"), KeyCode::Char('y'));
    assert_eq!(
        copy.result,
        Some(InfoResult::Copy("db.internal".to_owned())),
        "y copies the first copyable value"
    );
    let mut quit = make();
    press(&mut quit, id.sub("props"), KeyCode::Char('q'));
    assert_eq!(quit.result, Some(InfoResult::Closed));
    let mut act = make();
    press(&mut act, open, KeyCode::Enter);
    assert_eq!(
        act.result,
        Some(InfoResult::Action(0)),
        "an action button reports its index"
    );
    let mut close = make();
    press(&mut close, id.sub("close"), KeyCode::Enter);
    assert_eq!(close.result, Some(InfoResult::Closed));
    let mut enter = make();
    press(&mut enter, id.sub("other"), KeyCode::Enter);
    assert_eq!(
        enter.result,
        Some(InfoResult::Closed),
        "Enter closes from anywhere else"
    );
    let mut esc = make();
    press(&mut esc, id.sub("other"), KeyCode::Esc);
    assert_eq!(
        esc.result,
        Some(InfoResult::Closed),
        "Esc closes through the policy"
    );
    let mut stuck = make().dismiss(Dismiss::NEVER);
    assert_eq!(
        press(&mut stuck, id.sub("other"), KeyCode::Esc),
        Outcome::Consumed
    );
    assert_eq!(stuck.result, None);
    let mut own = make().keymap(vec![(Bind::char('x'), InfoCmd::Close)]);
    press(&mut own, id.sub("other"), KeyCode::Char('q'));
    assert_eq!(own.result, None, "a replaced keymap drops the defaults");
    press(&mut own, id.sub("other"), KeyCode::Char('x'));
    assert_eq!(own.result, Some(InfoResult::Closed));
    assert_eq!(InfoCmd::Close.label(), "Close");
    assert_eq!(InfoCmd::Copy.label(), "Copy");
    assert_eq!(InfoCmd::default_keymap().len(), 2);
    let tabbed = {
        let mut i = make();
        press(&mut i, id.sub("close"), KeyCode::Tab);
        i
    };
    assert_eq!(tabbed.result, None);

    let mut scrolled = make();
    draw(&mut scrolled);
    assert!(scrolled.detail_scroll.overflows());
    press(&mut scrolled, id.sub("close"), KeyCode::Down);
    assert_eq!(scrolled.detail_scroll.offset, 1, "Down scrolls the detail");
    press(&mut scrolled, id.sub("close"), KeyCode::Up);
    assert_eq!(scrolled.detail_scroll.offset, 0);
    let mut vim = make().nav(NavKeys::VIM);
    draw(&mut vim);
    press(&mut vim, id.sub("close"), KeyCode::Char('j'));
    assert_eq!(vim.detail_scroll.offset, 1, "vim keys scroll when enabled");

    let hints = HasHints::hints(&make(), &HintCx::new(None));
    assert!(hints.iter().any(|h| h.key == "Esc"));
    assert!(
        hints.iter().any(|h| h.action == "Copy"),
        "a copyable row advertises copy"
    );
    let hints = HasHints::hints(&make().dismiss(Dismiss::NEVER), &HintCx::new(None));
    assert!(!hints.iter().any(|h| h.key == "Esc"));

    let mut clicked = make();
    let shown = draw(&mut clicked);
    let mut focus = Focus::default();
    let at = Position::new(0, 0);
    assert_eq!(clicked.on_click(open, at, &mut focus), Outcome::Changed);
    assert_eq!(clicked.result, Some(InfoResult::Action(0)));
    assert_eq!(focus.current(), Some(open));
    let mut clicked = make();
    draw(&mut clicked);
    clicked.on_click(id.sub("close"), at, &mut focus);
    assert_eq!(clicked.result, Some(InfoResult::Closed));
    let mut row = make();
    let shown_row = draw(&mut row);
    let host_row = shown_row
        .hits
        .regions()
        .iter()
        .find(|r| r.owner == id.sub("props") && r.part.part == Part::ROW)
        .map(|r| (r.id, r.area))
        .expect("the properties register their rows");
    row.on_click(host_row.0, host_row.1.as_position(), &mut focus);
    assert_eq!(
        focus.current(),
        Some(id.sub("props")),
        "clicking a row focuses the list"
    );
    let mut other = make();
    assert_eq!(
        other.on_click(WidgetId::of("scenario.info.nothing"), at, &mut focus),
        Outcome::Consumed
    );
    assert_eq!(
        other.on_drag(WidgetId::of("scenario.info.nothing"), at),
        Outcome::Ignored
    );
    let bar = scrollbar::id_for(id);
    let track = shown
        .hits
        .bar(bar)
        .expect("the detail registers its bar")
        .track;
    let mut bar_click = make();
    draw(&mut bar_click);
    assert_ne!(
        bar_click.on_click(bar, Position::new(track.x, track.bottom() - 1), &mut focus),
        Outcome::Ignored
    );
    assert!(
        bar_click.detail_scroll.offset > 0,
        "clicking the track pages the detail"
    );
    let _ = bar_click.on_drag(bar, Position::new(track.x, track.y));

    let mut routed = make();
    let shown = draw(&mut routed);
    let r = routed.pointer_in(&click(&shown.hits, id.sub("close"), 1), &mut focus);
    assert_eq!(r.outcome(), Outcome::Changed);
    assert_eq!(
        routed.result,
        Some(InfoResult::Closed),
        "a routed click on Close closes"
    );
    let wheel = PointerEvent::wheel_on(
        &shown.hits,
        id,
        termrock::core::hit::Axis::Vertical,
        2,
        Position::new(50, 20),
        KeyModifiers::NONE,
    );
    let mut wheeled = make();
    draw(&mut wheeled);
    let _ = wheeled.pointer_in(&wheel, &mut focus);
    let glyph = make().close_button(IconButton::new(modal::CLOSE, "Q"));
    let mut glyph = glyph;
    assert!(text_of(&draw(&mut glyph).buf).contains('Q'));
}

fn choice_dialog_flow() {
    use termrock::widgets::choice_dialog::ChoiceDialog;

    let id = WidgetId::of("scenario.choice");
    let screen = Rect::new(0, 0, 100, 40);
    let (back, next) = (id.sub("back"), id.sub("next"));
    let make = || {
        ChoiceDialog::new(
            id,
            "Channel",
            "Updates come from",
            &["Stable", "Beta", "Nightly"],
            1,
        )
        .line("Pick one.", Tone::Muted)
        .stepper("Step 1 of 2")
        .width(60)
        .option_tones(vec![Tone::Normal, Tone::Warning, Tone::Danger])
        .buttons(
            vec![
                Button::subtle(back, "Back"),
                Button::primary(next, "Continue"),
            ],
            0,
        )
    };
    let draw = |d: &mut ChoiceDialog| {
        stage(screen, Interaction::default(), |screen, buf, ctx| {
            d.render(screen, buf, ctx)
        })
    };
    let mut dialog = make();
    assert_eq!((dialog.width, dialog.cancel_index), (60, 0));
    assert_eq!(dialog.initial_focus(), id.sub("radio"));
    assert_eq!(dialog.lines.len(), 1);
    assert_eq!(dialog.stepper.as_deref(), Some("Step 1 of 2"));
    let shown = draw(&mut dialog);
    let text = text_of(&shown.buf);
    for needle in [
        "Channel",
        "Updates come from",
        "Stable",
        "Beta",
        "Nightly",
        "Pick one.",
        "Step 1 of 2",
        "Back",
        "Continue",
    ] {
        assert!(text.contains(needle), "the dialog shows {needle}");
    }
    assert_eq!(
        dialog.area.height,
        dialog.height(),
        "height() is what the dialog draws"
    );
    assert!(dialog.height() > ChoiceDialog::new(id, "T", "L", &["a"], 0).height());
    let mut disabled_radio = make();
    disabled_radio.radio.disabled = true;
    assert_eq!(
        disabled_radio.initial_focus(),
        next,
        "a locked list starts on the primary button"
    );

    let ring = shown.ring;
    let mut focus = Focus::default();
    let mut press = |d: &mut ChoiceDialog, on: WidgetId, code: KeyCode| {
        focus.focus(on);
        let o = d.on_key(&key(code), &mut focus, &ring);
        (o, focus.current())
    };
    let mut d = make();
    press(&mut d, id.sub("radio"), KeyCode::Enter);
    assert_eq!(
        (d.result, d.fired),
        (Some(Some(1)), Some(1)),
        "Enter on the list takes the highlighted option"
    );
    let mut d = make();
    press(&mut d, id.sub("radio"), KeyCode::Down);
    assert_eq!(d.radio.selected, 2);
    press(&mut d, id.sub("radio"), KeyCode::Enter);
    assert_eq!(d.result, Some(Some(2)));
    let mut d = make();
    press(&mut d, back, KeyCode::Enter);
    assert_eq!(
        (d.result, d.fired),
        (Some(None), Some(0)),
        "the cancel button resolves to nothing chosen"
    );
    let mut d = make();
    press(&mut d, next, KeyCode::Enter);
    assert_eq!(d.result, Some(Some(1)));
    let mut d = make();
    press(&mut d, back, KeyCode::Esc);
    assert_eq!(
        (d.result, d.fired),
        (Some(None), Some(0)),
        "Esc is the cancel button"
    );
    let mut d = make().dismiss(Dismiss::NEVER);
    assert_eq!(press(&mut d, back, KeyCode::Esc).0, Outcome::Consumed);
    assert_eq!(d.result, None);
    let (o, at) = press(&mut make(), back, KeyCode::Right);
    assert_eq!(
        (o, at),
        (Outcome::Changed, Some(next)),
        "arrows step across the buttons"
    );
    let (_, at) = press(&mut make(), next, KeyCode::Left);
    assert_eq!(at, Some(back));
    let (_, at) = press(&mut make(), back, KeyCode::Tab);
    assert_ne!(at, Some(back));
    let (_, at) = press(&mut make(), id.sub("radio"), KeyCode::BackTab);
    assert_ne!(at, Some(id.sub("radio")));
    let (_, at) = press(&mut make().nav(NavKeys::VIM), back, KeyCode::Char('l'));
    assert_eq!(
        at,
        Some(next),
        "vim keys step across the buttons when enabled"
    );
    let (_, at) = press(&mut make(), back, KeyCode::Char('l'));
    assert_eq!(at, Some(back));
    let mut tab = make();
    assert_eq!(
        press(&mut tab, id.sub("other"), KeyCode::Char('z')).0,
        Outcome::Consumed
    );

    let mut blocked = make();
    blocked.buttons[1].disabled = true;
    let (o, _) = press(&mut blocked, id.sub("radio"), KeyCode::Enter);
    assert_eq!(
        (o, blocked.result),
        (Outcome::Changed, None),
        "a disabled primary refuses Enter"
    );
    let shown = draw(&mut blocked);
    let theme = Theme::for_level(ColorLevel::TrueColor);
    assert_eq!(
        shown.buf[(blocked.area.x, blocked.area.y)].fg,
        theme.warning,
        "the refused Enter flashes the border"
    );
    press(&mut blocked, id.sub("radio"), KeyCode::Down);
    assert_ne!(
        draw(&mut blocked).buf[(blocked.area.x, blocked.area.y)].fg,
        theme.warning,
        "the next key clears the flash"
    );

    let hints = HasHints::hints(&make(), &HintCx::new(Some(id.sub("radio"))));
    assert!(
        hints.iter().any(|h| h.action == "Continue"),
        "the primary action is advertised"
    );
    assert!(hints.iter().any(|h| h.key == "Tab"));
    assert!(hints.iter().any(|h| h.key == "Esc" && h.action == "Back"));
    let hints = HasHints::hints(&make().dismiss(Dismiss::NEVER), &HintCx::new(Some(back)));
    assert!(!hints.iter().any(|h| h.key == "Esc"));

    let mut clicked = make();
    let shown = draw(&mut clicked);
    let mut focus = Focus::default();
    let slow = clicked.radio.option_id(2);
    assert_eq!(clicked.on_click(slow, &mut focus), Outcome::Changed);
    assert_eq!(clicked.radio.selected, 2, "clicking an option selects it");
    assert_eq!(focus.current(), Some(id.sub("radio")));
    clicked.on_click(next, &mut focus);
    assert_eq!(clicked.result, Some(Some(2)));
    let mut clicked = make();
    draw(&mut clicked);
    clicked.on_click(back, &mut focus);
    assert_eq!(clicked.result, Some(None));
    assert_eq!(
        clicked.on_click(WidgetId::of("scenario.choice.nothing"), &mut focus),
        Outcome::Consumed
    );

    let routed_click = |dialog: &mut ChoiceDialog, target: WidgetId, count: u8| {
        let shown = draw(dialog);
        let ev = PointerEvent::new(
            &shown.hits,
            Phase::Click,
            target,
            center(area_of(&shown.hits, target)),
            count,
            KeyModifiers::NONE,
        );
        dialog.pointer_in(&ev, &mut Focus::default())
    };
    let mut dbl = make();
    let beta = dbl.radio.option_id(1);
    let _ = routed_click(&mut dbl, beta, 2);
    assert_eq!(
        dbl.result,
        Some(Some(1)),
        "double-clicking the chosen option confirms"
    );
    let mut single = make();
    let _ = routed_click(&mut single, beta, 1);
    assert_eq!(single.result, None);
    let mut off = make().double_click_confirms(false);
    assert!(!off.double_click_confirms);
    let _ = routed_click(&mut off, beta, 2);
    assert_eq!(
        off.result, None,
        "the double-click shortcut can be turned off"
    );
    let mut other = make();
    let slow = other.radio.option_id(2);
    let _ = routed_click(&mut other, slow, 2);
    assert_eq!(other.radio.selected, 2);
    assert_eq!(
        other.result,
        Some(Some(2)),
        "double-clicking an option picks it and confirms"
    );
    let mut btn = make();
    let _ = routed_click(&mut btn, next, 1);
    assert_eq!(btn.result, Some(Some(1)));
    let wheel = PointerEvent::wheel_on(
        &shown.hits,
        id.sub("radio"),
        termrock::core::hit::Axis::Vertical,
        1,
        Position::new(50, 20),
        KeyModifiers::NONE,
    );
    assert_eq!(
        make().pointer_in(&wheel, &mut Focus::default()).outcome(),
        Outcome::Ignored
    );

    let mut custom = make()
        .close_button(IconButton::new(modal::CLOSE, "Q"))
        .dismiss(Dismiss::LIGHT);
    assert!(text_of(&draw(&mut custom).buf).contains('Q'));
    let mut wrap = make().button_row(ButtonRow::new().wrap(true));
    let (_, at) = press(&mut wrap, next, KeyCode::Right);
    assert_eq!(at, Some(back), "a wrapping button row loops");
    let (_, at) = press(&mut make(), next, KeyCode::Right);
    assert_eq!(at, Some(next), "the default row stops at its end");
    let tone_style = |d: &mut ChoiceDialog| {
        let shown = draw(d);
        let row = d.radio.areas[2];
        shown.buf[(row.x + 5, row.y)].fg
    };
    let mut toned = make();
    let mut plain = make().option_tones(vec![]);
    assert_ne!(
        tone_style(&mut toned),
        tone_style(&mut plain),
        "an option tone recolours its label"
    );
}

fn layer_results_by_widget() {
    use termrock::core::intent::DismissReason;
    use termrock::ui::layer::{
        Dismissed, Layer, LayerCx, LayerEntry, LayerOut, LayerPointer, LayerTag, ShellLayers,
        Taken, dismiss,
    };
    use termrock::ui::layers::LayerResult;
    use termrock::widgets::choice_dialog::ChoiceDialog;
    use termrock::widgets::file_browser::{FileBrowser, FileBrowserResult, MemorySource};
    use termrock::widgets::help::HelpOverlay;
    use termrock::widgets::info_dialog::{InfoDialog, InfoResult};
    use termrock::widgets::menu::{ContextMenu, MenuAction, MenuItem};
    use termrock::widgets::picker::{Picker, PickerAction};

    let id = WidgetId::of("scenario.layers");
    let before = WidgetId::of("scenario.layers.before");
    let screen = Rect::new(0, 0, 100, 40);
    let mut focus = Focus::default();
    focus.set(Some(before));
    let mut layers: ShellLayers<LayerResult> = ShellLayers::new();

    assert!(
        matches!(
            layers.dismiss(DismissReason::Esc, &mut focus),
            Dismissed::None
        ),
        "nothing to dismiss"
    );
    assert!(layers.take(&mut focus).is_none());
    assert!(!layers.hides_page_focus());
    assert!(layers.top_as::<Dialog>().is_none());

    let ev = PointerEvent::new(
        &HitRegistry::default(),
        Phase::Click,
        id,
        Position::new(0, 0),
        1,
        KeyModifiers::NONE,
    );
    assert!(
        matches!(
            layers.pointer(&ev, &mut focus, &FocusRing::default(), &()),
            LayerPointer::Handled(Outcome::Ignored)
        ),
        "a pointer event with no layer open is not ours"
    );

    let cases: Vec<(Box<dyn Layer<LayerResult>>, LayerResult)> = vec![
        (
            Box::new(Dialog::confirm(id.sub("d"), "Close", "Sure?", "Yes")),
            LayerResult::Dialog {
                action: Some(0),
                text: None,
            },
        ),
        (
            Box::new(Dialog::prompt(
                id.sub("p"),
                "Name",
                TextInput::new(id.sub("p.in"), "Name").value("abc"),
                "OK",
            )),
            LayerResult::Dialog {
                action: Some(0),
                text: Some("abc".to_owned()),
            },
        ),
        (
            Box::new(ChoiceDialog::new(
                id.sub("c"),
                "Channel",
                "From",
                &["Stable", "Beta"],
                0,
            )),
            LayerResult::Choice(None),
        ),
        (
            Box::new(FormDialog::new(id.sub("f"), "Form", vec![])),
            LayerResult::Form(FormAction::Cancel),
        ),
        (
            Box::new(InfoDialog::new(
                id.sub("i"),
                "Info",
                vec![Prop::new("Host", "db")],
            )),
            LayerResult::Info(InfoResult::Closed),
        ),
        (
            Box::new(HelpOverlay::new(
                id.sub("h"),
                "Scope",
                vec![("Navigation", vec![("Tab", "Next")])],
            )),
            LayerResult::Help,
        ),
        (
            Box::new(FileBrowser::new(
                id.sub("b"),
                "Open",
                MemorySource::new(vec![]),
                "/",
            )),
            LayerResult::File(FileBrowserResult::Cancelled),
        ),
        (
            Box::new(Picker::new(id.sub("k"), "Pick")),
            LayerResult::Picker(PickerAction::Cancelled),
        ),
        (
            Box::new(ContextMenu::new(id.sub("m"), vec![MenuItem::new("Open")])),
            LayerResult::Menu(MenuAction::Dismissed),
        ),
    ];
    for (layer, expected) in cases {
        layers.open(layer, LayerTag::new("probe").key("k").n(3), (), &mut focus);
        assert_eq!(layers.len(), 1);
        let Dismissed::Closed(entry, result) = layers.dismiss(DismissReason::Esc, &mut focus)
        else {
            panic!("every one of these layers is dismissable");
        };
        assert_eq!(
            result,
            Some(expected),
            "Esc resolves to the layer's cancel result"
        );
        assert_eq!(
            (entry.tag.kind, entry.tag.key.as_str(), entry.tag.n),
            ("probe", "k", 3)
        );
        assert!(layers.is_empty());
        assert_eq!(
            focus.current(),
            Some(before),
            "closing returns focus to the opener"
        );
    }

    let mut ring = FocusRing::default();
    ring.register(id.sub("ok"));
    layers.open(
        Box::new(Dialog::confirm(id.sub("d"), "Close", "Sure?", "Yes")),
        LayerTag::new("dialog"),
        (),
        &mut focus,
    );
    assert!(layers.hides_page_focus(), "a modal hides the page's focus");
    assert_eq!(
        focus.current(),
        Some(id.sub("d").sub("ok")),
        "a dialog opens on its initial focus"
    );
    {
        let top = layers.top_mut().expect("a layer");
        let mut cx = LayerCx {
            focus: &mut focus,
            ring: &ring,
            app: &(),
        };
        assert_eq!(
            top.layer.on_key(&key(KeyCode::Enter), &mut cx),
            Outcome::Changed
        );
    }
    let Some(Taken::Closed(entry, result)) = layers.take(&mut focus) else {
        panic!("Enter on the confirm button closes the dialog");
    };
    assert_eq!(
        result,
        LayerResult::Dialog {
            action: Some(1),
            text: None
        }
    );
    assert!(entry.get::<Dialog>().is_some() && entry.get::<InfoDialog>().is_none());
    assert_eq!(focus.current(), Some(before));

    let theme_focus = |name: &str| WidgetId::of("scenario.layers").sub(name);
    let mut form = FormDialog::new(
        theme_focus("form"),
        "Form",
        vec![FormField::check(
            "live",
            Checkbox::new(theme_focus("live"), "Live", false),
        )],
    );
    form.dismiss = Dismiss::ESCAPE;
    layers.open(Box::new(form), LayerTag::new("form"), (), &mut focus);
    assert_eq!(focus.current(), Some(theme_focus("live")));
    {
        let top = layers.top_mut().expect("a layer");
        let mut cx = LayerCx {
            focus: &mut focus,
            ring: &ring,
            app: &(),
        };
        top.layer.on_key(&key(KeyCode::Char(' ')), &mut cx);
    }
    let Some(Taken::Event(result)) = layers.take(&mut focus) else {
        panic!("a toggled check is an event, not a close");
    };
    assert_eq!(
        result,
        LayerResult::Form(FormAction::Changed("live".to_owned()))
    );
    assert!(layers.take(&mut focus).is_none(), "the queue is drained");
    assert_eq!(layers.len(), 1, "an event leaves the layer open");
    assert!(layers.top_as::<FormDialog>().is_some_and(|f| f.dirty));
    assert!(layers.top_as_mut::<FormDialog>().is_some());
    assert!(
        layers.top_as::<Dialog>().is_none(),
        "the wrong type is not handed out"
    );
    assert!(layers.top().expect("a layer").get::<FormDialog>().is_some());
    assert!(
        layers
            .top_mut()
            .expect("a layer")
            .get_mut::<FormDialog>()
            .is_some()
    );
    assert_eq!(
        layers.on_blur(termrock::core::focus::FocusChange {
            from: theme_focus("elsewhere"),
            to: None
        }),
        Outcome::Ignored
    );
    {
        let top = layers.top_mut().expect("a layer");
        let mut cx = LayerCx {
            focus: &mut focus,
            ring: &ring,
            app: &(),
        };
        top.layer.on_key(&key(KeyCode::Esc), &mut cx);
    }
    let Some(Taken::Closed(_, result)) = layers.take(&mut focus) else {
        panic!("Esc cancels the form");
    };
    assert_eq!(result, LayerResult::Form(FormAction::Cancel));

    layers.open(
        Box::new(InfoDialog::new(
            id.sub("copy"),
            "Info",
            vec![Prop::new("Host", "db").copyable()],
        )),
        LayerTag::new("info"),
        (),
        &mut focus,
    );
    {
        let top = layers.top_mut().expect("a layer");
        let mut cx = LayerCx {
            focus: &mut focus,
            ring: &ring,
            app: &(),
        };
        top.layer.on_key(&key(KeyCode::Char('y')), &mut cx);
    }
    let Some(Taken::Event(result)) = layers.take(&mut focus) else {
        panic!("copying leaves the dialog open");
    };
    assert_eq!(result, LayerResult::Info(InfoResult::Copy("db".to_owned())));
    assert_eq!(layers.len(), 1);
    layers.pop(&mut focus);

    let mut open_info = InfoDialog::new(id.sub("click"), "Info", vec![]);
    open_info.dismiss = Dismiss::LIGHT;
    layers.open(Box::new(open_info), LayerTag::new("info"), (), &mut focus);
    let shown = stage(screen, Interaction::default(), |screen, buf, ctx| {
        layers
            .top_mut()
            .expect("a layer")
            .layer
            .render(screen, buf, ctx, &());
    });
    let outside = PointerEvent::new(
        &shown.hits,
        Phase::Click,
        modal::BACKDROP,
        Position::new(1, 1),
        1,
        KeyModifiers::NONE,
    );
    let close = click(&shown.hits, id.sub("click").sub("close"), 1);
    assert!(
        matches!(
            layers.pointer(&close, &mut focus, &shown.ring, &()),
            LayerPointer::Handled(Outcome::Changed)
        ),
        "a click on a layer's own button is handled by the layer"
    );
    assert!(matches!(
        layers.take(&mut focus),
        Some(Taken::Closed(_, LayerResult::Info(InfoResult::Closed)))
    ));
    layers.open(
        Box::new(InfoDialog::new(id.sub("click"), "Info", vec![])),
        LayerTag::new("info"),
        (),
        &mut focus,
    );
    assert!(
        matches!(
            layers.pointer(&outside, &mut focus, &shown.ring, &()),
            LayerPointer::Dismissed(Dismissed::Closed(
                _,
                Some(LayerResult::Info(InfoResult::Closed))
            ))
        ),
        "a click outside the surface dismisses a light layer"
    );
    assert!(layers.is_empty());

    let mut stuck = InfoDialog::new(id.sub("stuck"), "Info", vec![]);
    stuck.dismiss = Dismiss::NEVER;
    layers.open(Box::new(stuck), LayerTag::new("info"), (), &mut focus);
    assert!(
        matches!(
            layers.pointer(&outside, &mut focus, &shown.ring, &()),
            LayerPointer::Dismissed(Dismissed::Refused)
        ),
        "a never-dismissed layer refuses the click"
    );
    assert!(matches!(
        layers.dismiss(DismissReason::Esc, &mut focus),
        Dismissed::Refused
    ));
    assert!(matches!(
        layers.dismiss(DismissReason::OutsideClick, &mut focus),
        Dismissed::Refused
    ));
    assert!(matches!(
        layers.dismiss(DismissReason::FocusOut, &mut focus),
        Dismissed::Refused
    ));
    assert_eq!(layers.len(), 1, "a refusal keeps the layer");
    assert!(
        matches!(
            layers.dismiss(DismissReason::Programmatic, &mut focus),
            Dismissed::Closed(_, Some(LayerResult::Info(InfoResult::Closed)))
        ),
        "the host can always close a layer"
    );

    let mut owned: ShellLayers<LayerResult, (), u8> = ShellLayers::new();
    owned.open(
        Box::new(Dialog::confirm(id.sub("o"), "Close", "Sure?", "Yes")),
        LayerTag::new("dialog"),
        7,
        &mut focus,
    );
    let entry: &LayerEntry<LayerResult, (), u8> = owned.top().expect("a layer");
    assert_eq!(entry.owner, 7, "an entry remembers who opened it");
    assert_eq!(Modal::dismiss(entry), Dismiss::LIGHT);

    let mut dialog = Dialog::confirm(id.sub("pol"), "Close", "Sure?", "Yes");
    let closed = dismiss(&mut dialog, DismissReason::Esc);
    assert!(
        closed.is_some(),
        "an allowed reason runs the overlay's dismissal"
    );
    let mut never = Dialog::confirm(id.sub("pol"), "Close", "Sure?", "Yes").dismiss(Dismiss::NEVER);
    assert!(
        dismiss(&mut never, DismissReason::Esc).is_none(),
        "a refused reason runs nothing"
    );
    assert!(never.result.is_none());

    assert!(LayerOut::Close(1).closes() && !LayerOut::Event(1).closes());
    assert_eq!(
        (
            LayerOut::Close(5).into_inner(),
            LayerOut::Event(6).into_inner()
        ),
        (5, 6)
    );
}

fn layer_spec_policies() {
    use termrock::core::intent::DismissReason;
    use termrock::ui::layer::{
        Dismissed, Layer, LayerCx, LayerKind, LayerOut, LayerPointer, LayerSpec, LayerTag, Overlay,
        ShellLayers, Taken,
    };

    struct Probe {
        spec: LayerSpec,
        results: Vec<LayerOut<&'static str>>,
        log: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    impl Layer<&'static str> for Probe {
        fn spec(&self) -> LayerSpec {
            self.spec
        }
        fn render(&mut self, screen: Rect, buf: &mut Buffer, _: &mut RenderCtx, _: &()) {
            buf.set_string(screen.x, screen.y, "probe", Style::new());
        }
        fn on_key(&mut self, key: &Key, _: &mut LayerCx<'_, ()>) -> Outcome {
            self.log.borrow_mut().push(format!("key {:?}", key.code));
            Outcome::Consumed
        }
        fn on_dismiss(&mut self, reason: DismissReason) {
            self.log.borrow_mut().push(format!("dismiss {reason:?}"));
            self.results.clear();
            self.results.push(LayerOut::Close("cancelled"));
        }
        fn take_result(&mut self) -> Option<LayerOut<&'static str>> {
            (!self.results.is_empty()).then(|| self.results.remove(0))
        }
    }

    let (a, b, c) = (
        WidgetId::of("scenario.spec.a"),
        WidgetId::of("scenario.spec.b"),
        WidgetId::of("scenario.spec.c"),
    );
    let modal_spec = LayerSpec::modal(Dismiss::LIGHT);
    assert_eq!(modal_spec.kind, LayerKind::Modal);
    assert_eq!(modal_spec.placement, Placement::Center);
    assert_eq!(modal_spec.backdrop, Some(Backdrop::DEFAULT));
    assert_eq!(modal_spec.dismiss, Dismiss::LIGHT);
    assert!(modal_spec.restore_focus && modal_spec.inert_below && modal_spec.hides_page_focus);
    assert_eq!((modal_spec.anchor, modal_spec.initial_focus), (None, None));
    let popover = LayerSpec::popover(Dismiss::ESCAPE);
    assert_eq!(popover.kind, LayerKind::Popover);
    assert_eq!(popover.placement, Placement::Below);
    assert_eq!(popover.backdrop, None, "a popover does not dim the page");
    assert_eq!(popover.dismiss, Dismiss::ESCAPE);
    let tip = LayerSpec::tooltip();
    assert_eq!(tip.kind, LayerKind::Tooltip);
    assert_eq!(tip.dismiss, Dismiss::LIGHT);
    assert!(
        !tip.restore_focus && !tip.inert_below && !tip.hides_page_focus,
        "a tooltip never takes the page"
    );
    assert_ne!(LayerKind::Modal, LayerKind::Popover);
    assert_ne!(LayerKind::Popover, LayerKind::Tooltip);

    let anchor = Rect::new(4, 5, 6, 1);
    let built = LayerSpec::popover(Dismiss::LIGHT)
        .anchor(anchor, Placement::Above)
        .backdrop(Some(Backdrop::FULL))
        .initial_focus(b)
        .restore_focus(false)
        .inert_below(false)
        .hides_page_focus(false);
    assert_eq!(
        (built.anchor, built.placement),
        (Some(anchor), Placement::Above)
    );
    assert_eq!(built.backdrop, Some(Backdrop::FULL));
    assert_eq!(built.initial_focus, Some(b));
    assert!(!built.restore_focus && !built.inert_below && !built.hides_page_focus);
    assert_eq!(modal_spec.backdrop(None).backdrop, None);
    assert!(modal_spec.restore_focus(true).restore_focus);
    assert!(modal_spec.inert_below(true).inert_below);
    assert!(modal_spec.hides_page_focus(true).hides_page_focus);

    let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let probe = |spec: LayerSpec, results: Vec<LayerOut<&'static str>>| {
        Box::new(Probe {
            spec,
            results,
            log: log.clone(),
        }) as Box<dyn Layer<&'static str>>
    };
    let mut focus = Focus::default();
    focus.set(Some(a));
    let mut layers: ShellLayers<&'static str> = ShellLayers::new();

    layers.open(
        probe(modal_spec, vec![]),
        LayerTag::new("modal"),
        (),
        &mut focus,
    );
    assert_eq!(
        focus.current(),
        None,
        "an inert-below layer clears the page's focus"
    );
    assert!(layers.hides_page_focus());
    layers.pop(&mut focus);
    assert_eq!(focus.current(), Some(a), "and restores it on close");

    layers.open(
        probe(tip.initial_focus(c), vec![]),
        LayerTag::new("moved"),
        (),
        &mut focus,
    );
    assert_eq!(focus.current(), Some(c), "an initial focus wins");
    assert!(
        !layers.hides_page_focus(),
        "a tooltip leaves page focus visible"
    );
    layers.pop(&mut focus);
    assert_eq!(
        focus.current(),
        Some(c),
        "a layer that does not restore leaves focus where it put it"
    );

    focus.set(Some(a));
    layers.open(probe(tip, vec![]), LayerTag::new("tip"), (), &mut focus);
    assert_eq!(
        focus.current(),
        Some(a),
        "a passive layer keeps the page's focus"
    );
    layers.pop(&mut focus);

    layers.open(
        probe(
            popover.restore_focus(false),
            vec![LayerOut::Event("first"), LayerOut::Close("last")],
        ),
        LayerTag::new("popover"),
        (),
        &mut focus,
    );
    assert!(
        layers.hides_page_focus(),
        "a popover hides page focus until it says otherwise"
    );
    {
        let top = layers.top_mut().expect("a layer");
        let mut cx = LayerCx {
            focus: &mut focus,
            ring: &FocusRing::default(),
            app: &(),
        };
        assert_eq!(
            top.layer.on_key(&key(KeyCode::Char('x')), &mut cx),
            Outcome::Consumed
        );
        assert!(!top.layer.is_editing() && !top.layer.animating());
        assert!(top.layer.hints(None).is_empty());
        assert_eq!(top.layer.on_paste("text"), Outcome::Ignored);
        assert_eq!(
            top.layer.on_paste_focused("text", &Focus::default()),
            Outcome::Ignored
        );
        assert_eq!(
            top.layer
                .on_blur(termrock::core::focus::FocusChange { from: a, to: None }),
            Outcome::Ignored
        );
        assert_eq!(top.layer.on_tick(&()), Outcome::Ignored);
        assert_eq!(
            top.layer.on_wheel(1, Position::new(0, 0)),
            Outcome::Consumed
        );
        assert_eq!(
            top.layer.on_wheel_h(1, Position::new(0, 0)),
            Outcome::Consumed
        );
        assert_eq!(
            top.layer.on_press(a, Position::new(0, 0), 1),
            Outcome::Ignored
        );
        assert_eq!(top.layer.on_drag(a, Position::new(0, 0)), Outcome::Ignored);
        assert_eq!(
            top.layer.on_release(a, Position::new(0, 0)),
            Outcome::Ignored
        );
        assert_eq!(
            top.layer.on_click(a, Position::new(0, 0), &mut cx),
            Outcome::Consumed
        );
        assert_eq!(
            top.layer.on_click_count(a, Position::new(0, 0), 2, &mut cx),
            Outcome::Consumed
        );
    }
    let mut hits = HitRegistry::default();
    hits.register(a, Rect::new(0, 0, 10, 3));
    let route =
        |layers: &mut ShellLayers<&'static str>, focus: &mut Focus, ev: PointerEvent| match layers
            .pointer(&ev, focus, &FocusRing::default(), &())
        {
            LayerPointer::Handled(o) => o,
            LayerPointer::Dismissed(_) => panic!("only the backdrop dismisses"),
        };
    let at = Position::new(1, 1);
    let mk = |phase, count| PointerEvent::new(&hits, phase, a, at, count, KeyModifiers::NONE);
    assert_eq!(
        route(&mut layers, &mut focus, mk(Phase::Press, 1)),
        Outcome::Ignored
    );
    assert_eq!(
        route(&mut layers, &mut focus, mk(Phase::Drag, 1)),
        Outcome::Ignored
    );
    assert_eq!(
        route(&mut layers, &mut focus, mk(Phase::Release, 1)),
        Outcome::Ignored
    );
    assert_eq!(
        route(&mut layers, &mut focus, mk(Phase::Click, 2)),
        Outcome::Consumed,
        "the default click is swallowed"
    );
    let wheel = PointerEvent::wheel_on(
        &hits,
        a,
        termrock::core::hit::Axis::Vertical,
        1,
        at,
        KeyModifiers::NONE,
    );
    assert_eq!(route(&mut layers, &mut focus, wheel), Outcome::Consumed);
    let wheel_h = PointerEvent::wheel_on(
        &hits,
        a,
        termrock::core::hit::Axis::Horizontal,
        1,
        at,
        KeyModifiers::NONE,
    );
    assert_eq!(route(&mut layers, &mut focus, wheel_h), Outcome::Consumed);

    let Some(Taken::Event(first)) = layers.take(&mut focus) else {
        panic!("queued event")
    };
    assert_eq!(first, "first");
    let Some(Taken::Closed(_, last)) = layers.take(&mut focus) else {
        panic!("queued close")
    };
    assert_eq!(last, "last");
    assert_eq!(
        focus.current(),
        None,
        "a layer that does not restore leaves focus cleared"
    );
    assert!(log.borrow().iter().any(|l| l.contains("Char('x')")));

    layers.open(
        probe(modal_spec, vec![LayerOut::Event("stale")]),
        LayerTag::new("modal"),
        (),
        &mut focus,
    );
    let Dismissed::Closed(_, result) = layers.dismiss(DismissReason::CloseButton, &mut focus)
    else {
        panic!("a light modal closes");
    };
    assert_eq!(
        result,
        Some("cancelled"),
        "dismissal replaces what was queued"
    );
    assert!(log.borrow().iter().any(|l| l == "dismiss CloseButton"));
    let stage = stage(
        Rect::new(0, 0, 20, 5),
        Interaction::default(),
        |screen, buf, ctx| {
            let mut shown: Box<dyn Layer<&'static str>> = probe(modal_spec, vec![]);
            shown.render(screen, buf, ctx, &());
        },
    );
    assert!(text_of(&stage.buf).starts_with("probe"));

    let dialog = Dialog::confirm(a, "Close", "Sure?", "Yes");
    assert_eq!(Overlay::dismiss(&dialog), Dismiss::LIGHT);
    let spec = Overlay::spec(&dialog);
    assert_eq!(spec.kind, LayerKind::Modal);
    assert_eq!(spec.initial_focus, Some(dialog.initial_focus));
    assert!(Overlay::close_button(&dialog).is_some());
    let stuck = Dialog::confirm(a, "Close", "Sure?", "Yes").dismiss(Dismiss::NEVER);
    assert_eq!(Overlay::spec(&stuck).dismiss, Dismiss::NEVER);
}

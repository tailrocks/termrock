//! Lane `routing` of the #14 burn-down: this lane's callable scenarios.
//!
//! Each scenario builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. Register one `ScenarioCase` per
//! `ScenarioId` variant from the `lane:routing` block in `../coverage.rs`. Edit only
//! this file and `../coverage/lane_routing.rs`.

use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Position, Rect};
use termrock::core::event::{
    ClientPlatform, External, Input, InputConfig, Key, Mouse, MouseKind, Outcome,
};
use termrock::core::hit::{Axis, Bar, Hit, HitMark, HitRegion, HitRegistry, ReleaseFrom};
use termrock::core::id::{ItemKey, Part, PartRef, WidgetId};
use termrock::core::keyboard::{
    self, AltKeys, DISAMBIGUATE, FLAGS, KeyboardProtocol, POP, ProtocolState, QUERY,
    REPORT_ALTERNATE_KEYS,
};
use termrock::core::refusal::{self, Control, Reason, clear_refusal, take_refusal};
use termrock::widgets::choice::Checkbox;

use ratatui::buffer::Buffer;
use ratatui::layout::Size;
use termrock::core::focus::{Focus, FocusChange, FocusRing};
use termrock::core::hints::{
    Binding, Bindings, HasHints, HintCx, TYPE_TO_JUMP, actions_bindings, keymap_bindings,
    nav_bindings, nav_hints,
};
use termrock::core::intent::{DismissReason, Intent, Phase};
use termrock::core::keymap::{
    Actions, KeyBinding, KeyMap, KeyboardCapabilities, Nav, NavKeys, SELECT_ALL, binding_for,
    deliverable_binding_for, deliverable_binding_for_with, hint_key, intern, label_for, lookup,
    lookup_with, normalized_key_mods, select_all_bindings, unassigned_chord,
};
use termrock::core::pointer::PointerEvent;
use termrock::core::pointer::{
    FocusOn, MouseTarget, PointerConfig, PointerHost, PointerParts, PointerRouter, PointerState,
    Release, SecondaryOutside, SecondaryRoute, scroll_input,
};
use termrock::core::response::Response;
use termrock::core::shape::{CaretShape, PointerShape, shapes_for};
use termrock::theme::{ColorLevel, Theme};
use termrock::ui::ctx::{Interaction, RenderCtx};
use termrock::ui::popup::Dismiss;
use termrock::ui::scroll_view::{Axes, ScrollView};
use termrock::widgets::choice::RadioGroup;
use termrock::widgets::choice_dialog::ChoiceDialog;
use termrock::widgets::keyhint::HintPriority;
use termrock::widgets::list::{ListBox, ListItem, SelectMode};

use super::{ScenarioCase, ScenarioId, paint, paint_hits, text_of};

#[rustfmt::skip]
pub const CASES: &[ScenarioCase] = &[
    ScenarioCase { id: ScenarioId::InputDecoding, run: input_decoding },
    ScenarioCase { id: ScenarioId::KeyboardNegotiation, run: keyboard_negotiation },
    ScenarioCase { id: ScenarioId::RefusalReasons, run: refusal_reasons },
    ScenarioCase { id: ScenarioId::HitRegistryRouting, run: hit_registry_routing },
    ScenarioCase { id: ScenarioId::KeymapResolution, run: keymap_resolution },
    ScenarioCase { id: ScenarioId::NavKeyProfiles, run: nav_key_profiles },
    ScenarioCase { id: ScenarioId::HintBindings, run: hint_bindings },
    ScenarioCase { id: ScenarioId::IntentVocabulary, run: intent_vocabulary },
    ScenarioCase { id: ScenarioId::PointerClickRouting, run: pointer_click_routing },
    ScenarioCase { id: ScenarioId::PointerScrollGestures, run: pointer_scroll_gestures },
    ScenarioCase { id: ScenarioId::PointerStatePolicies, run: pointer_state_policies },
];

fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    }
}

fn key_event(code: KeyCode, mods: KeyModifiers, kind: KeyEventKind) -> Event {
    Event::Key(KeyEvent::new_with_kind(code, mods, kind))
}

fn mouse_event(kind: MouseEventKind, mods: KeyModifiers) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column: 4,
        row: 2,
        modifiers: mods,
    })
}

/// Terminal events decode into typed input: platform rules, mouse kinds, key
/// predicates and the outcome a real checkbox reports for the decoded key.
fn input_decoding() {
    // The client platform comes from the environment, an override first.
    assert_eq!(ClientPlatform::OVERRIDE_ENV, "JUNIE_TUI_CLIENT_PLATFORM");
    for value in ["macos", "Mac", " DARWIN "] {
        let platform = ClientPlatform::from_env(|name| {
            (name == ClientPlatform::OVERRIDE_ENV).then(|| value.to_owned())
        });
        assert_eq!(platform, ClientPlatform::MacOs, "override {value:?}");
    }
    for value in ["other", "linux", "Windows"] {
        let platform = ClientPlatform::from_env(|name| {
            (name == ClientPlatform::OVERRIDE_ENV).then(|| value.to_owned())
        });
        assert_eq!(platform, ClientPlatform::Other, "override {value:?}");
    }
    assert_eq!(
        ClientPlatform::from_env(env_of(&[
            (ClientPlatform::OVERRIDE_ENV, "other"),
            ("TERM_PROGRAM", "Apple_Terminal"),
        ])),
        ClientPlatform::Other,
        "the override beats the terminal program"
    );
    for program in ClientPlatform::MACOS_TERM_PROGRAMS {
        let platform =
            ClientPlatform::from_env(|name| (name == "TERM_PROGRAM").then(|| program.to_owned()));
        assert_eq!(platform, ClientPlatform::MacOs, "{program}");
    }
    assert_eq!(
        ClientPlatform::from_env(env_of(&[("LC_TERMINAL", "iTerm2")])),
        ClientPlatform::MacOs
    );
    assert_eq!(
        ClientPlatform::from_env(env_of(&[
            (ClientPlatform::OVERRIDE_ENV, "unknown"),
            ("TERM_PROGRAM", "xterm"),
        ])),
        ClientPlatform::build_target(),
        "an unknown override and terminal fall back to the build target"
    );
    assert_eq!(
        ClientPlatform::build_target().is_macos(),
        cfg!(target_os = "macos")
    );
    assert!(ClientPlatform::MacOs.is_macos());
    assert!(!ClientPlatform::Other.is_macos());
    assert_eq!(
        ClientPlatform::current(),
        ClientPlatform::from_env(|name| std::env::var(name).ok()),
        "the cached platform is the one the process environment names"
    );

    let mac = InputConfig::for_platform(ClientPlatform::MacOs);
    let other = InputConfig::for_platform(ClientPlatform::Other);
    assert!(mac.ctrl_click_secondary && !other.ctrl_click_secondary);
    assert!(mac.shift_wheel_horizontal && other.shift_wheel_horizontal);
    assert!(mac.legacy_ctrl_punctuation && other.legacy_ctrl_punctuation);

    // Every mouse kind is produced by the crossterm event that names it.
    let at = Position::new(4, 2);
    let kinds = [
        (MouseEventKind::Moved, MouseKind::Move),
        (MouseEventKind::Down(MouseButton::Left), MouseKind::Down),
        (MouseEventKind::Up(MouseButton::Left), MouseKind::Up),
        (MouseEventKind::Drag(MouseButton::Left), MouseKind::Drag),
        (
            MouseEventKind::Down(MouseButton::Right),
            MouseKind::Secondary,
        ),
        (
            MouseEventKind::Drag(MouseButton::Right),
            MouseKind::SecondaryDrag,
        ),
        (
            MouseEventKind::Up(MouseButton::Right),
            MouseKind::SecondaryUp,
        ),
        (MouseEventKind::Down(MouseButton::Middle), MouseKind::Middle),
        (MouseEventKind::ScrollUp, MouseKind::WheelUp),
        (MouseEventKind::ScrollDown, MouseKind::WheelDown),
        (MouseEventKind::ScrollLeft, MouseKind::WheelLeft),
        (MouseEventKind::ScrollRight, MouseKind::WheelRight),
    ];
    for (raw, kind) in kinds {
        assert_eq!(
            Input::from_crossterm_with(mouse_event(raw, KeyModifiers::NONE), other),
            Some(Input::Mouse(Mouse::new(kind, at))),
            "{raw:?}"
        );
    }
    assert_eq!(
        Input::from_crossterm_with(
            mouse_event(MouseEventKind::Up(MouseButton::Middle), KeyModifiers::NONE),
            other
        ),
        None,
        "a middle release is not an input"
    );

    // Shift turns the vertical wheel into a horizontal one, unless switched off.
    let shifted = mouse_event(MouseEventKind::ScrollUp, KeyModifiers::SHIFT);
    assert_eq!(
        Input::from_crossterm_with(shifted.clone(), other),
        Some(Input::Mouse(
            Mouse::new(MouseKind::WheelLeft, at).with_mods(KeyModifiers::SHIFT)
        ))
    );
    let no_shift_wheel = InputConfig {
        shift_wheel_horizontal: false,
        ..other
    };
    assert_eq!(
        Input::from_crossterm_with(shifted, no_shift_wheel),
        Some(Input::Mouse(
            Mouse::new(MouseKind::WheelUp, at).with_mods(KeyModifiers::SHIFT)
        ))
    );

    // Ctrl+click is a secondary click only where the platform says so.
    let ctrl_click = mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        KeyModifiers::CONTROL,
    );
    let ctrl_drag = mouse_event(
        MouseEventKind::Drag(MouseButton::Left),
        KeyModifiers::CONTROL,
    );
    let ctrl_up = mouse_event(MouseEventKind::Up(MouseButton::Left), KeyModifiers::CONTROL);
    for (event, secondary, plain) in [
        (ctrl_click, MouseKind::Secondary, MouseKind::Down),
        (ctrl_drag, MouseKind::SecondaryDrag, MouseKind::Drag),
        (ctrl_up, MouseKind::SecondaryUp, MouseKind::Up),
    ] {
        let kind_of = |config| match Input::from_crossterm_with(event.clone(), config) {
            Some(Input::Mouse(mouse)) => mouse.kind,
            other => panic!("expected a mouse input, got {other:?}"),
        };
        assert_eq!(kind_of(mac), secondary);
        assert_eq!(kind_of(other), plain);
    }

    // Keys: presses and repeats decode, releases do not; legacy Ctrl+4..7 map
    // to their punctuation unless that is switched off.
    let press = |code, mods| key_event(code, mods, KeyEventKind::Press);
    let space = Input::from_crossterm_with(press(KeyCode::Char(' '), KeyModifiers::NONE), other);
    assert_eq!(
        space,
        Some(Input::Key(Key {
            code: KeyCode::Char(' '),
            mods: KeyModifiers::NONE
        }))
    );
    assert!(
        Input::from_crossterm_with(
            key_event(KeyCode::Char('a'), KeyModifiers::NONE, KeyEventKind::Repeat),
            other
        )
        .is_some()
    );
    assert_eq!(
        Input::from_crossterm_with(
            key_event(
                KeyCode::Char('a'),
                KeyModifiers::NONE,
                KeyEventKind::Release
            ),
            other
        ),
        None
    );
    for (digit, mapped) in [('4', '\\'), ('5', ']'), ('6', '^'), ('7', '_')] {
        let ctrl_digit = press(KeyCode::Char(digit), KeyModifiers::CONTROL);
        assert_eq!(
            Input::from_crossterm_with(ctrl_digit.clone(), other),
            Some(Input::Key(Key {
                code: KeyCode::Char(mapped),
                mods: KeyModifiers::CONTROL
            }))
        );
        let raw = InputConfig {
            legacy_ctrl_punctuation: false,
            ..other
        };
        assert_eq!(
            Input::from_crossterm_with(ctrl_digit, raw),
            Some(Input::Key(Key {
                code: KeyCode::Char(digit),
                mods: KeyModifiers::CONTROL
            }))
        );
    }
    let ambient = press(KeyCode::Char('x'), KeyModifiers::ALT);
    assert_eq!(
        Input::from_crossterm(ambient.clone()),
        Input::from_crossterm_with(ambient, InputConfig::default()),
        "the ambient decoder is the configured one with the default config"
    );

    // Resize, paste, tick and host messages are the other inputs.
    assert_eq!(
        Input::from_crossterm_with(Event::Resize(80, 24), other),
        Some(Input::Resize(80, 24))
    );
    assert_eq!(
        Input::from_crossterm_with(Event::Paste("pasted".into()), other),
        Some(Input::Paste("pasted".to_owned()))
    );
    assert_eq!(
        Input::from_crossterm_with(Event::FocusGained, other),
        None,
        "focus reports are not input"
    );
    let message = External::new(String::from("job-7"));
    assert_eq!(
        message.downcast_ref::<String>().map(String::as_str),
        Some("job-7")
    );
    assert!(message.downcast_ref::<u32>().is_none(), "wrong type");
    let delivered = Input::External(message.clone());
    assert_eq!(delivered, Input::External(message.clone()));
    assert_ne!(
        delivered,
        Input::External(External::new(String::from("job-7"))),
        "host messages compare by identity"
    );
    assert_ne!(delivered, Input::Tick);

    // Mouse helpers and the wheel vector of each wheel kind.
    let click = Mouse::new(MouseKind::Down, at);
    assert_eq!(click.mods, KeyModifiers::NONE);
    assert!(!click.shift() && !click.ctrl() && !click.alt());
    let held = click.with_mods(KeyModifiers::SHIFT | KeyModifiers::CONTROL | KeyModifiers::ALT);
    assert!(held.shift() && held.ctrl() && held.alt());
    for (kind, vector) in [
        (MouseKind::WheelUp, Some((0, -1))),
        (MouseKind::WheelDown, Some((0, 1))),
        (MouseKind::WheelLeft, Some((-1, 0))),
        (MouseKind::WheelRight, Some((1, 0))),
        (MouseKind::Down, None),
    ] {
        assert_eq!(Mouse::new(kind, at).wheel(), vector, "{kind:?}");
    }

    // Key predicates.
    let key = |code, mods| Key { code, mods };
    let ctrl_s = key(KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert!(ctrl_s.ctrl() && ctrl_s.chord() && !ctrl_s.plain() && ctrl_s.ctrl_char('s'));
    assert!(!ctrl_s.is_char('s') && !ctrl_s.shift() && !ctrl_s.alt() && !ctrl_s.cmd());
    let alt_x = key(KeyCode::Char('x'), KeyModifiers::ALT);
    assert!(alt_x.alt() && alt_x.chord() && !alt_x.is_char('x'));
    let cmd_k = key(KeyCode::Char('k'), KeyModifiers::SUPER);
    assert!(cmd_k.cmd() && cmd_k.chord());
    let shift_tab = key(KeyCode::Tab, KeyModifiers::SHIFT);
    assert!(shift_tab.shift() && shift_tab.plain() && shift_tab.is(KeyCode::Tab));
    assert!(!shift_tab.chord());
    assert!(key(KeyCode::Char('q'), KeyModifiers::NONE).is_char('q'));
    assert!(!key(KeyCode::Char('q'), KeyModifiers::NONE).is(KeyCode::Enter));

    // Outcomes combine to the strongest one.
    assert!(!Outcome::Ignored.consumed());
    assert!(Outcome::Consumed.consumed() && Outcome::Changed.consumed());
    assert_eq!(Outcome::default(), Outcome::Ignored);
    assert_eq!(Outcome::Ignored.or(Outcome::Consumed), Outcome::Consumed);
    assert_eq!(Outcome::Consumed.or(Outcome::Changed), Outcome::Changed);
    assert_eq!(Outcome::Changed.or(Outcome::Ignored), Outcome::Changed);
    assert_eq!(Outcome::Ignored.or(Outcome::Ignored), Outcome::Ignored);

    // A decoded key reaches a real checkbox: space toggles, 'x' is ignored.
    let mut check = Checkbox::new(
        termrock::core::id::WidgetId::of("scenario.input.check"),
        "Notify me",
        false,
    );
    let Some(Input::Key(space)) = space else {
        panic!("space decodes to a key");
    };
    assert_eq!(check.on_key(&space), Outcome::Changed);
    assert!(check.checked);
    let Some(Input::Key(letter)) =
        Input::from_crossterm_with(press(KeyCode::Char('x'), KeyModifiers::NONE), other)
    else {
        panic!("x decodes to a key");
    };
    assert_eq!(check.on_key(&letter), Outcome::Ignored);
    let buf = paint(Rect::new(0, 0, 24, 1), |area, buf, ctx| {
        check.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert!(text_of(&buf).contains("Notify me"));
}

/// The keyboard protocol negotiation: flags, the escape sequences both ways,
/// the policy from the environment and how a session enters and leaves.
fn keyboard_negotiation() {
    assert_eq!(DISAMBIGUATE, 0b1);
    assert_eq!(REPORT_ALTERNATE_KEYS, 0b100);
    assert_eq!(FLAGS, DISAMBIGUATE | REPORT_ALTERNATE_KEYS);
    assert_eq!(QUERY, "\x1b[?u");
    assert_eq!(POP, "\x1b[<u");
    assert_eq!(keyboard::push(FLAGS), "\x1b[>5u");
    assert_eq!(keyboard::parse_flags_reply(b"\x1b[?5u"), Some(5));
    assert_eq!(keyboard::parse_flags_reply(b"\x1b[?u"), None);
    assert_eq!(keyboard::parse_flags_reply(b"\x1b[?5x"), None);
    assert_eq!(keyboard::parse_flags_reply(b"5u"), None);

    // Modifier parameters are one plus the bit set, in both directions.
    assert_eq!(keyboard::modifiers(1), KeyModifiers::NONE);
    assert_eq!(keyboard::modifiers(2), KeyModifiers::SHIFT);
    assert_eq!(keyboard::modifiers(3), KeyModifiers::ALT);
    assert_eq!(keyboard::modifiers(5), KeyModifiers::CONTROL);
    assert_eq!(keyboard::modifiers(9), KeyModifiers::SUPER);
    assert_eq!(keyboard::modifiers(0), KeyModifiers::NONE);
    for mods in [
        KeyModifiers::NONE,
        KeyModifiers::CONTROL,
        KeyModifiers::SHIFT | KeyModifiers::ALT,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT | KeyModifiers::SUPER,
    ] {
        assert_eq!(keyboard::modifiers(keyboard::modifier_param(mods)), mods);
    }
    assert_eq!(
        keyboard::modifier_param(KeyModifiers::CONTROL | KeyModifiers::SHIFT),
        6
    );

    // A chord encodes to CSI-u and decodes back to the same key.
    let ctrl_shift_a = Key {
        code: KeyCode::Char('A'),
        mods: KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    };
    assert_eq!(
        keyboard::encode(&ctrl_shift_a).as_deref(),
        Some("\x1b[97;6u")
    );
    let plain_a = Key {
        code: KeyCode::Char('a'),
        mods: KeyModifiers::NONE,
    };
    assert_eq!(keyboard::encode(&plain_a).as_deref(), Some("\x1b[97u"));
    assert_eq!(
        keyboard::encode(&Key {
            code: KeyCode::Up,
            mods: KeyModifiers::NONE
        }),
        None,
        "only characters have a CSI-u code"
    );
    assert_eq!(
        keyboard::decode(b"\x1b[97;6u"),
        Some(Key {
            code: KeyCode::Char('a'),
            mods: KeyModifiers::CONTROL | KeyModifiers::SHIFT
        })
    );
    for (seq, code) in [
        (&b"\x1b[27u"[..], KeyCode::Esc),
        (b"\x1b[13u", KeyCode::Enter),
        (b"\x1b[9u", KeyCode::Tab),
        (b"\x1b[127u", KeyCode::Backspace),
        (b"\x1b[107:75;5u", KeyCode::Char('k')),
    ] {
        assert_eq!(keyboard::decode(seq).map(|key| key.code), Some(code));
    }
    assert_eq!(keyboard::decode(b"\x1b[?5u"), None, "a reply is not a key");
    assert_eq!(keyboard::decode(b"\x1b[>5u"), None, "a push is not a key");
    assert_eq!(keyboard::decode(b"97u"), None);

    // The policy comes from the environment and defaults to detection.
    assert_eq!(KeyboardProtocol::ENV, "JUNIE_TUI_KEYBOARD_PROTOCOL");
    assert_eq!(KeyboardProtocol::default(), KeyboardProtocol::Auto);
    for (value, expected) in [
        ("on", KeyboardProtocol::On),
        ("kitty", KeyboardProtocol::On),
        ("1", KeyboardProtocol::On),
        ("off", KeyboardProtocol::Off),
        ("Legacy", KeyboardProtocol::Off),
        ("0", KeyboardProtocol::Off),
        ("whatever", KeyboardProtocol::Auto),
    ] {
        let policy = KeyboardProtocol::from_env(|name| {
            (name == KeyboardProtocol::ENV).then(|| value.to_owned())
        });
        assert_eq!(policy, expected, "{value}");
    }
    assert_eq!(KeyboardProtocol::from_env(|_| None), KeyboardProtocol::Auto);
    assert_eq!(
        KeyboardProtocol::current(),
        KeyboardProtocol::from_env(|name| std::env::var(name).ok())
    );

    // A session pushes the flags once, pops them once and can resume.
    let mut state = ProtocolState::default();
    assert_eq!(state.enter(KeyboardProtocol::Off, || true), None);
    assert!(!state.enabled && !state.pushed);
    assert_eq!(
        state.enter(KeyboardProtocol::Auto, || false),
        None,
        "Auto asks the terminal and it said no"
    );
    assert_eq!(
        state.enter(KeyboardProtocol::Auto, || true),
        Some(keyboard::push(FLAGS))
    );
    assert!(state.enabled && state.pushed);
    assert_eq!(state.resume(), None, "already pushed");
    assert_eq!(state.leave(), Some(POP));
    assert_eq!(state.leave(), None, "nothing left to pop");
    assert_eq!(
        state.resume(),
        Some(keyboard::push(FLAGS)),
        "the terminal came back: push again"
    );
    assert_eq!(
        state.enter(KeyboardProtocol::On, || false),
        None,
        "On while already pushed pushes nothing more"
    );

    // Alt keys: the policy from the environment, per platform and terminal.
    assert_eq!(AltKeys::ENV, "JUNIE_TUI_ALT_KEYS");
    let alt_env = |platform, pairs: &'static [(&'static str, &'static str)]| {
        AltKeys::from_env(platform, env_of(pairs))
    };
    for value in ["on", "1", "true", "yes"] {
        assert_eq!(
            AltKeys::from_env(ClientPlatform::MacOs, |name| {
                (name == AltKeys::ENV).then(|| value.to_owned())
            }),
            AltKeys::Delivered
        );
    }
    for value in ["off", "0", "false", "no"] {
        assert_eq!(
            AltKeys::from_env(ClientPlatform::Other, |name| {
                (name == AltKeys::ENV).then(|| value.to_owned())
            }),
            AltKeys::Refused
        );
    }
    assert_eq!(alt_env(ClientPlatform::Other, &[]), AltKeys::Delivered);
    for program in AltKeys::OPTION_TYPES_TERM_PROGRAMS {
        let keys = AltKeys::from_env(ClientPlatform::MacOs, |name| {
            (name == "TERM_PROGRAM").then(|| program.to_owned())
        });
        assert_eq!(keys, AltKeys::UntilSeen, "{program} types characters");
    }
    assert_eq!(
        alt_env(ClientPlatform::MacOs, &[("LC_TERMINAL", "iTerm2")]),
        AltKeys::UntilSeen
    );
    for program in AltKeys::OPTION_IS_ALT_TERM_PROGRAMS {
        let keys = AltKeys::from_env(ClientPlatform::MacOs, |name| {
            (name == "TERM_PROGRAM").then(|| program.to_uppercase())
        });
        assert_eq!(keys, AltKeys::Delivered, "{program} sends Alt");
    }
    assert_eq!(
        alt_env(ClientPlatform::MacOs, &[("TERM_PROGRAM", "ghostty")]),
        AltKeys::WithProtocol
    );
    match AltKeys::current() {
        AltKeys::Delivered => assert!(keyboard::alt_delivered()),
        AltKeys::Refused => assert!(!keyboard::alt_delivered()),
        AltKeys::UntilSeen | AltKeys::WithProtocol => {}
    }

    // What the terminal can deliver decides which hint labels may show.
    assert!(
        !keyboard::active(),
        "no session was entered in this process"
    );
    assert!(keyboard::label_deliverable("Ctrl+S"));
    assert!(keyboard::label_deliverable("Alt+click"), "a pointer chord");
    assert!(!keyboard::label_deliverable("Shift+click"));
    assert!(!keyboard::label_deliverable("Shift+drag"));
    assert_eq!(
        keyboard::label_deliverable("Alt+X"),
        keyboard::alt_delivered()
    );
    assert_eq!(
        keyboard::label_deliverable("Ctrl+Alt+X"),
        keyboard::alt_delivered()
    );
    assert_eq!(
        keyboard::label_deliverable("Cmd+K"),
        keyboard::super_delivered()
    );
    assert_eq!(
        keyboard::label_deliverable("⌘K"),
        keyboard::super_delivered()
    );
}

/// Reasons and controls: the length limits, how a fixed control refuses and
/// the refusal a real checkbox raises.
fn refusal_reasons() {
    clear_refusal();
    assert_eq!(Reason::TEXT_MAX, 60);
    assert_eq!(Reason::SHORT_MAX, 20);
    let reason = Reason::new("Locked by your administrator").short("Locked");
    assert_eq!(reason.short_or_text(), "Locked");
    assert_eq!(Reason::new("Read only").short_or_text(), "Read only");
    assert!(reason.within_limits());
    assert!(!Reason::new(&"x".repeat(Reason::TEXT_MAX + 1)).within_limits());
    assert!(
        !Reason::new("Short enough")
            .short(&"y".repeat(Reason::SHORT_MAX + 1))
            .within_limits()
    );
    assert!(reason.text_fits(28) && !reason.text_fits(27));
    assert!(reason.short_fits(6) && !reason.short_fits(5));
    assert!(
        !Reason::new("Locked by your administrator").short_fits(80),
        "no short form never fits"
    );

    let live = Control::default();
    assert_eq!(live, Control::Live);
    assert!(!live.is_fixed() && live.reason().is_none());
    assert!(!live.refuse());
    assert_eq!(take_refusal(), None, "a live control raises nothing");

    let fixed = Control::fixed(reason.clone());
    assert_eq!(fixed, Control::Fixed(reason.clone()));
    assert!(fixed.is_fixed());
    assert_eq!(fixed.reason(), Some(&reason));
    assert!(fixed.refuse());
    assert_eq!(take_refusal(), Some(reason.clone()), "the reason is raised");
    assert_eq!(take_refusal(), None, "taking empties the slot");
    refusal::raise(&reason);
    clear_refusal();
    assert_eq!(take_refusal(), None, "clearing drops a raised reason");

    // A real checkbox refuses its toggle and raises the reason.
    let id = termrock::core::id::WidgetId::of("scenario.refusal.check");
    let mut locked = Checkbox::new(id, "Telemetry", false).fixed(reason.clone());
    let space = Key {
        code: KeyCode::Char(' '),
        mods: KeyModifiers::NONE,
    };
    assert_eq!(locked.on_key(&space), Outcome::Consumed);
    assert!(!locked.checked, "a fixed control does not change");
    assert_eq!(take_refusal(), Some(reason));
    let mut open = Checkbox::new(id, "Telemetry", false).control(Control::Live);
    assert_eq!(open.on_key(&space), Outcome::Changed);
    assert!(open.checked);
    assert_eq!(take_refusal(), None);
    let buf = paint(Rect::new(0, 0, 24, 1), |area, buf, ctx| {
        locked.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert!(text_of(&buf).contains("Telemetry"));
}

/// The hit registry resolves a screen position to the region that owns it:
/// stacking, parts, scroll-only and hover-only regions, barriers, backdrops,
/// scrollbar tracks, release acceptance and the shift a scroll leaves behind.
fn hit_registry_routing() {
    const BASE: WidgetId = WidgetId::of("scenario.hit.base");
    const TOP: WidgetId = WidgetId::of("scenario.hit.top");
    const SPARE: WidgetId = WidgetId::of("scenario.hit.spare");
    const OWNER: WidgetId = WidgetId::of("scenario.hit.owner");
    const BACK: WidgetId = WidgetId::of("scenario.hit.back");
    let at = Position::new;

    // A real checkbox registers the region a click lands on.
    let check_id = WidgetId::of("scenario.hit.check");
    let mut check = Checkbox::new(check_id, "Remember me", false);
    let painted = paint_hits(Rect::new(0, 0, 30, 2), |area, buf, ctx| {
        check.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert_eq!(painted.hits.hit(at(2, 0)), Some(check_id));
    assert_eq!(painted.hits.hit(at(2, 1)), None, "the row below is empty");
    assert_eq!(painted.hits.area_of(check_id).map(|area| area.y), Some(0));

    // The topmost region wins; raising a region brings it back on top.
    let mut hits = HitRegistry::default();
    assert!(hits.is_empty());
    hits.register(BASE, Rect::new(0, 0, 20, 10));
    hits.register(TOP, Rect::new(5, 2, 6, 4));
    hits.register(SPARE, Rect::ZERO);
    assert_eq!(
        (hits.len(), hits.is_empty()),
        (2, false),
        "empty areas are dropped"
    );
    assert_eq!(hits.hit(at(6, 3)), Some(TOP));
    assert_eq!(hits.hit(at(1, 1)), Some(BASE));
    assert_eq!(hits.hit(at(30, 30)), None);
    let container = Hit {
        owner: TOP,
        part: PartRef::of(Part::CONTAINER),
    };
    assert_eq!(hits.resolve(at(6, 3)), Some(container));
    let region: &HitRegion = &hits.regions()[1];
    assert_eq!(region.hit(), container);
    assert_eq!(region.area, Rect::new(5, 2, 6, 4));
    assert!(!region.scroll_only && !region.hover_only && !region.backdrop && !region.pass_through);
    assert_eq!(hits.area_of(TOP), Some(Rect::new(5, 2, 6, 4)));
    assert_eq!(hits.area_of(SPARE), None);
    hits.raise(BASE);
    assert_eq!(hits.hit(at(6, 3)), Some(BASE), "BASE was raised over TOP");
    assert_eq!(hits.len(), 3, "a raise repeats the region on top");

    // Parts carry the item they belong to.
    let row = PartRef::item(Part::ROW, ItemKey::index(2));
    let row_id = hits.register_part(OWNER, row, Rect::new(0, 12, 10, 1));
    assert_eq!(row_id, OWNER.part(row));
    let row_hit = Hit {
        owner: OWNER,
        part: row,
    };
    assert_eq!(hits.resolve(at(1, 12)), Some(row_hit));
    assert_eq!(hits.part_of(row_id), Some(row_hit));
    let cell_id = WidgetId::of("scenario.hit.cell");
    hits.register_part_as(
        cell_id,
        OWNER,
        PartRef::of(Part::CELL),
        Rect::new(0, 13, 10, 1),
    );
    assert_eq!(hits.hit(at(1, 13)), Some(cell_id));
    assert_eq!(
        hits.resolve(at(1, 13)).map(|h| h.part.part),
        Some(Part::CELL)
    );
    let before = hits.len();
    hits.register_part_as(SPARE, OWNER, row, Rect::ZERO);
    assert_eq!(hits.len(), before, "an empty part is dropped");
    assert_eq!(hits.part_of(SPARE), None);

    // A region registered under goes beneath what was there; a barrier hides
    // everything registered before it.
    let mut hits = HitRegistry::default();
    hits.register(BASE, Rect::new(0, 0, 10, 10));
    hits.register_under(TOP, Rect::new(0, 0, 10, 10));
    assert_eq!(hits.hit(at(5, 5)), Some(BASE), "TOP sits under BASE");
    hits.register_under(SPARE, Rect::ZERO);
    assert_eq!(hits.len(), 2);
    hits.push_barrier();
    hits.register(OWNER, Rect::new(5, 5, 5, 5));
    hits.register_under(BACK, Rect::new(0, 0, 3, 3));
    assert_eq!(hits.hit(at(1, 1)), Some(BACK), "a layer's own under region");
    assert_eq!(hits.hit(at(5, 5)), Some(OWNER));
    assert_eq!(hits.hit(at(1, 8)), None, "the layer below is unreachable");

    // Scroll-only regions take the wheel but no click; hover-only regions
    // take the pointer but neither click nor wheel.
    let mut hits = HitRegistry::default();
    let area = Rect::new(0, 0, 10, 10);
    hits.register_scroll_under(SPARE, area);
    hits.register_scroll(BASE, area);
    assert_eq!(hits.hit(at(2, 2)), None);
    assert_eq!(hits.hover_at(at(2, 2)), None);
    assert_eq!(hits.hit_scroll(at(2, 2)), Some(BASE));
    assert_eq!(hits.scrollers(), vec![(SPARE, area), (BASE, area)]);
    assert!(hits.regions().iter().all(|r| r.scroll_only));
    hits.register(TOP, Rect::new(0, 0, 4, 4));
    hits.register_hover(OWNER, Rect::new(0, 0, 2, 2));
    assert_eq!(hits.hit(at(1, 1)), Some(TOP), "hover-only is not clickable");
    assert_eq!(hits.hover_at(at(1, 1)), Some(OWNER));
    assert_eq!(hits.hit_scroll(at(1, 1)), Some(TOP));
    assert_eq!(
        hits.scroll_stack(at(1, 1)),
        vec![TOP, BASE, SPARE],
        "every scrollable under the pointer, innermost first"
    );
    assert!(hits.regions().iter().any(|r| r.hover_only));

    // Surfaces, pointer shapes and release acceptance.
    let mut hits = HitRegistry::default();
    hits.register_surface(BASE, Rect::new(0, 0, 10, 10));
    hits.register(TOP, Rect::new(0, 0, 3, 3));
    assert!(hits.is_surface(BASE) && !hits.is_surface(TOP));
    assert!(hits.regions()[0].surface);
    assert_eq!(hits.resolve(at(8, 8)).map(|h| h.owner), Some(BASE));
    assert_eq!(hits.shape_of(BASE), None);
    hits.set_shape(BASE, PointerShape::Text);
    hits.set_shape(BASE, PointerShape::ColResize);
    assert_eq!(hits.shape_of(BASE), Some(PointerShape::ColResize));
    assert_eq!(hits.shape_of(TOP), None);
    assert!(!hits.accepts_release(TOP, ReleaseFrom::Press(BASE)));
    hits.accept_release(TOP, ReleaseFrom::Press(BASE));
    assert!(hits.accepts_release(TOP, ReleaseFrom::Press(BASE)));
    assert!(!hits.accepts_release(TOP, ReleaseFrom::Press(SPARE)));
    assert!(!hits.accepts_release(TOP, ReleaseFrom::Secondary));
    hits.accept_release(TOP, ReleaseFrom::Secondary);
    assert!(hits.accepts_release(TOP, ReleaseFrom::Secondary));

    // A scrollbar: track, thumb and the axis it moves along.
    let mut hits = HitRegistry::default();
    let track = Bar {
        id: BACK,
        owner: OWNER,
        axis: Axis::Vertical,
        track: Rect::new(19, 0, 1, 10),
    };
    hits.register_bar(track);
    hits.register_bar(Bar {
        track: Rect::ZERO,
        ..track
    });
    assert_eq!(hits.bars(), &[track], "an empty track is dropped");
    assert_eq!(hits.bar(BACK), Some(track));
    assert_eq!(hits.bar(SPARE), None);
    assert_eq!(hits.area_of(BACK), Some(track.track));
    hits.register_thumb(SPARE, OWNER, Rect::new(19, 0, 1, 2));
    assert_eq!(hits.len(), 1, "a thumb needs its bar");
    hits.register_thumb(BACK, OWNER, Rect::new(19, 2, 1, 3));
    let part_at = |hits: &HitRegistry, y| hits.resolve(at(19, y)).map(|h| h.part.part);
    assert_eq!(part_at(&hits, 3), Some(Part::THUMB));
    assert_eq!(part_at(&hits, 8), Some(Part::TRACK));
    let across = Bar {
        axis: Axis::Horizontal,
        track: Rect::new(0, 9, 19, 1),
        ..track
    };
    assert_ne!(across.axis, track.axis);
    hits.register_bar(across);
    assert_eq!(hits.bars().len(), 2);

    // Backdrops: a click outside the dialog names the backdrop and what lies
    // beneath it; a pass-through backdrop forwards that click.
    let mut hits = HitRegistry::default();
    hits.register(BASE, Rect::new(0, 0, 30, 10));
    hits.push_barrier();
    hits.register_backdrop(BACK, Rect::new(0, 0, 30, 10), false);
    hits.register(TOP, Rect::new(5, 2, 10, 5));
    assert_eq!(hits.backdrop_at(at(1, 1)), Some((BACK, Some(BASE))));
    assert_eq!(
        hits.backdrop_at(at(6, 3)),
        None,
        "the dialog is not a backdrop"
    );
    assert_eq!(hits.pass_through_target(BACK, at(1, 1)), None);
    let mut hits = HitRegistry::default();
    hits.register(BASE, Rect::new(0, 0, 30, 10));
    hits.push_barrier();
    hits.register_backdrop_of(BACK, OWNER, Rect::new(0, 0, 30, 10), true);
    assert_eq!(hits.pass_through_target(BACK, at(1, 1)), Some(BASE));
    assert_eq!(hits.pass_through_target(OWNER, at(1, 1)), None);
    assert_eq!(
        hits.resolve(at(1, 1)),
        Some(Hit {
            owner: OWNER,
            part: PartRef::of(Part::BACKDROP)
        })
    );
    assert!(hits.regions()[1].backdrop && hits.regions()[1].pass_through);

    // Autoscroll edges belong to the layer they were registered on.
    let mut hits = HitRegistry::default();
    hits.register_autoscroll(Rect::new(0, 0, 10, 5));
    hits.register_autoscroll(Rect::ZERO);
    assert_eq!(hits.autoscroll_at(at(1, 1)), Some(Rect::new(0, 0, 10, 5)));
    assert_eq!(hits.autoscroll_at(at(20, 20)), None);
    hits.push_barrier();
    assert_eq!(
        hits.autoscroll_at(at(1, 1)),
        None,
        "another layer is on top"
    );

    // Scrolling content moves the regions registered since the mark and
    // remembers the shift, so a screen position maps back to content space.
    let mut hits = HitRegistry::default();
    let mark: HitMark = hits.mark();
    hits.register(BASE, Rect::new(0, 10, 20, 10));
    hits.scroll_since(
        mark,
        Rect::new(0, 0, 20, 60),
        Rect::new(0, 5, 20, 10),
        at(0, 0),
    );
    assert_eq!(hits.shift_of(BASE), (0, 5));
    assert_eq!(hits.hit(at(3, 6)), Some(BASE));
    assert_eq!(hits.hit(at(3, 12)), None, "scrolled out of the window");
    assert_eq!(hits.local(BASE, at(3, 6)), at(3, 11));
    assert_eq!(hits.shift_of(SPARE), (0, 0));
    assert_eq!(hits.regions()[0].area, Rect::new(0, 5, 20, 5));
    hits.clip_since(0, Rect::new(0, 0, 5, 3));
    assert!(hits.is_empty(), "a region clipped to nothing is dropped");
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Cmd {
    Save,
    Quit,
    Palette,
}

impl Actions for Cmd {
    fn default_keymap() -> KeyMap<Self> {
        vec![
            (KeyBinding::ctrl('s'), Cmd::Save),
            (KeyBinding::char('q'), Cmd::Quit),
            (KeyBinding::cmd('k'), Cmd::Palette),
        ]
    }

    fn label(self) -> &'static str {
        match self {
            Cmd::Save => "Save",
            Cmd::Quit => "Quit",
            Cmd::Palette => "Palette",
        }
    }
}

fn typed(code: KeyCode, mods: KeyModifiers) -> Key {
    Key { code, mods }
}

/// Key bindings, their labels and how a keymap resolves a key: the capability
/// aware overloads decide from the capabilities passed in, the ambient ones
/// from the terminal this process runs in.
fn keymap_resolution() {
    // Constructors and the fields they set.
    let plain = KeyBinding::new(
        KeyCode::Char('x'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert_eq!(plain.code, KeyCode::Char('x'));
    assert_eq!(plain.mods, KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    assert!(!plain.alt_stand_in);
    assert_eq!(KeyBinding::key(KeyCode::F(5)).mods, KeyModifiers::NONE);
    assert_eq!(KeyBinding::char('q'), KeyBinding::key(KeyCode::Char('q')));
    assert_eq!(KeyBinding::ctrl('s').mods, KeyModifiers::CONTROL);
    assert_eq!(KeyBinding::alt('s').mods, KeyModifiers::ALT);
    assert_eq!(KeyBinding::cmd('s').mods, KeyModifiers::SUPER);
    assert_eq!(
        KeyBinding::ctrl('s').shift().mods,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT
    );
    assert!(KeyBinding::ctrl('b').alt_stand_in().alt_stand_in);

    // Shift folds into upper-case characters.
    assert_eq!(KeyBinding::char('T').normalized_mods(), KeyModifiers::SHIFT);
    assert_eq!(KeyBinding::char('t').normalized_mods(), KeyModifiers::NONE);
    assert_eq!(
        normalized_key_mods(&typed(KeyCode::Char('T'), KeyModifiers::CONTROL)),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT
    );
    assert_eq!(
        normalized_key_mods(&typed(KeyCode::Enter, KeyModifiers::NONE)),
        KeyModifiers::NONE
    );

    // Capability-aware overloads.
    let nothing = KeyboardCapabilities {
        alt: false,
        super_key: false,
    };
    let alt_only = KeyboardCapabilities {
        alt: true,
        super_key: false,
    };
    let everything = KeyboardCapabilities {
        alt: true,
        super_key: true,
    };
    assert!(KeyBinding::ctrl('s').deliverable_with(nothing));
    assert!(!KeyBinding::alt('s').deliverable_with(nothing));
    assert!(KeyBinding::alt('s').deliverable_with(alt_only));
    assert!(!KeyBinding::cmd('s').deliverable_with(alt_only));
    assert!(KeyBinding::cmd('s').deliverable_with(everything));
    let stand_in = KeyBinding::ctrl('x').alt_stand_in();
    assert!(
        stand_in.deliverable_with(nothing) && !stand_in.deliverable_with(alt_only),
        "a stand-in is offered only where the real Alt chord is not"
    );
    let ctrl_x = typed(KeyCode::Char('x'), KeyModifiers::CONTROL);
    assert!(stand_in.matches_with(&ctrl_x, nothing));
    assert!(!stand_in.matches_with(&ctrl_x, alt_only));
    assert!(KeyBinding::ctrl('x').matches_with(&ctrl_x, alt_only));
    assert!(!KeyBinding::ctrl('y').matches_with(&ctrl_x, nothing));
    assert!(
        !KeyBinding::char('q').matches_with(&typed(KeyCode::Char('q'), KeyModifiers::ALT), nothing),
        "modifiers must agree"
    );
    assert!(
        KeyBinding::char('q')
            .shift()
            .matches_with(&typed(KeyCode::Char('Q'), KeyModifiers::NONE), nothing),
        "an upper-case Q is Shift+q"
    );

    let alt_map: KeyMap<Cmd> = vec![
        (KeyBinding::alt('x'), Cmd::Palette),
        (KeyBinding::ctrl('x').alt_stand_in(), Cmd::Quit),
    ];
    let alt_x = typed(KeyCode::Char('x'), KeyModifiers::ALT);
    assert_eq!(lookup_with(&alt_map, &alt_x, alt_only), Some(Cmd::Palette));
    assert_eq!(lookup_with(&alt_map, &alt_x, nothing), None);
    assert_eq!(lookup_with(&alt_map, &ctrl_x, nothing), Some(Cmd::Quit));
    assert_eq!(lookup_with(&alt_map, &ctrl_x, alt_only), None);
    assert_eq!(
        binding_for(&alt_map, Cmd::Palette),
        Some(KeyBinding::alt('x'))
    );
    assert_eq!(binding_for(&alt_map, Cmd::Save), None);
    assert_eq!(
        deliverable_binding_for_with(&alt_map, Cmd::Palette, alt_only),
        Some(KeyBinding::alt('x'))
    );
    assert_eq!(
        deliverable_binding_for_with(&alt_map, Cmd::Palette, nothing),
        None
    );
    assert_eq!(
        deliverable_binding_for_with(&alt_map, Cmd::Quit, nothing),
        Some(stand_in)
    );

    // Ambient overloads agree with the capability-aware ones fed the
    // capabilities of this terminal.
    let here = KeyboardCapabilities::current();
    let map = Cmd::default_keymap();
    for binding in [
        KeyBinding::ctrl('s'),
        KeyBinding::alt('s'),
        KeyBinding::cmd('k'),
        stand_in,
    ] {
        assert_eq!(binding.deliverable(), binding.deliverable_with(here));
        assert_eq!(
            binding.matches(&ctrl_x),
            binding.matches_with(&ctrl_x, here)
        );
    }
    let ctrl_s = typed(KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert_eq!(lookup(&map, &ctrl_s), Some(Cmd::Save));
    assert_eq!(lookup(&map, &ctrl_s), lookup_with(&map, &ctrl_s, here));
    assert_eq!(
        lookup(&map, &typed(KeyCode::Char('Q'), KeyModifiers::SHIFT)),
        Some(Cmd::Quit),
        "Shift+q finds the binding for q"
    );
    assert_eq!(
        lookup(&map, &typed(KeyCode::Char('z'), KeyModifiers::NONE)),
        None
    );
    assert_eq!(
        deliverable_binding_for(&map, Cmd::Save),
        Some(KeyBinding::ctrl('s'))
    );
    assert_eq!(
        deliverable_binding_for(&map, Cmd::Palette).is_some(),
        KeyBinding::cmd('k').deliverable(),
        "Cmd+K is offered only where Super reaches the app"
    );

    // An unassigned Ctrl or Alt chord is reported, never a plain key.
    assert!(unassigned_chord(
        &typed(KeyCode::Char('z'), KeyModifiers::CONTROL),
        &map
    ));
    assert!(!unassigned_chord(&ctrl_s, &map), "Ctrl+S is bound");
    assert!(!unassigned_chord(
        &typed(KeyCode::Char('z'), KeyModifiers::NONE),
        &map
    ));
    assert!(!unassigned_chord(
        &typed(KeyCode::Enter, KeyModifiers::CONTROL),
        &map
    ));

    // Labels follow the platform.
    assert_eq!(KeyBinding::ctrl('s').label(), "Ctrl+S");
    assert_eq!(KeyBinding::cmd('k').label_on(ClientPlatform::MacOs), "⌘K");
    assert_eq!(
        KeyBinding::cmd('k').label_on(ClientPlatform::Other),
        "Super+K"
    );
    assert_eq!(
        KeyBinding::cmd('k').shift().label_on(ClientPlatform::MacOs),
        "⇧⌘K"
    );
    assert_eq!(
        KeyBinding::ctrl('a')
            .shift()
            .label_on(ClientPlatform::Other),
        "Ctrl+Shift+A"
    );
    for (binding, label) in [
        (KeyBinding::char(' '), "Space"),
        (KeyBinding::key(KeyCode::Enter), "Enter"),
        (KeyBinding::key(KeyCode::Esc), "Esc"),
        (KeyBinding::key(KeyCode::BackTab), "Shift+Tab"),
        (KeyBinding::key(KeyCode::Up), "↑"),
        (KeyBinding::key(KeyCode::PageDown), "PgDn"),
        (KeyBinding::key(KeyCode::F(5)), "F5"),
        (KeyBinding::alt('x'), "Alt+X"),
    ] {
        assert_eq!(binding.label_on(ClientPlatform::Other), label);
    }
    assert_eq!(
        KeyBinding::cmd('k').label(),
        KeyBinding::cmd('k').label_on(ClientPlatform::current())
    );
    assert_eq!(label_for(&map, Cmd::Save), Some("Ctrl+S".to_owned()));
    assert_eq!(label_for(&map, Cmd::Quit), Some("q".to_owned()));
    assert_eq!(hint_key(&map, Cmd::Save), Some("Ctrl+S"));
    assert!(
        std::ptr::eq(
            hint_key(&map, Cmd::Save).expect("bound"),
            hint_key(&map, Cmd::Save).expect("bound")
        ),
        "hint labels are interned"
    );
    assert_eq!(hint_key(&[], Cmd::Save), None);
    let first = intern("Scenario+Label".to_owned());
    assert!(std::ptr::eq(first, intern("Scenario+Label".to_owned())));
    assert_eq!(first, "Scenario+Label");

    // Select all has one binding per modifier family.
    assert_eq!(SELECT_ALL, [KeyBinding::cmd('a'), KeyBinding::ctrl('a')]);
    let select_all: Vec<_> = select_all_bindings(Cmd::Palette).collect();
    assert_eq!(
        select_all,
        vec![
            (KeyBinding::cmd('a'), Cmd::Palette),
            (KeyBinding::ctrl('a'), Cmd::Palette)
        ]
    );
    assert_eq!(Cmd::Quit.label(), "Quit");
}

/// Navigation key profiles: which keys move a real list, what each key means
/// and which hint labels the profile advertises.
fn nav_key_profiles() {
    assert_eq!(NavKeys::DEFAULT, NavKeys::DESKTOP);
    assert_eq!(NavKeys::NO_VIM, NavKeys::DESKTOP);
    let flags = |nav: NavKeys| (nav.arrows, nav.vim, nav.page, nav.home_end);
    assert_eq!(flags(NavKeys::DESKTOP), (true, false, true, true));
    assert_eq!(flags(NavKeys::VIM), (true, true, true, true));
    assert_eq!(NavKeys::DESKTOP.vim(true), NavKeys::VIM);
    assert!(!NavKeys::DESKTOP.page(false).page);
    assert!(!NavKeys::DESKTOP.home_end(false).home_end);
    assert!(!NavKeys::DESKTOP.arrows(false).arrows);

    let key = |code| typed(code, KeyModifiers::NONE);
    for (code, nav) in [
        (KeyCode::Up, Nav::Up),
        (KeyCode::Down, Nav::Down),
        (KeyCode::Left, Nav::Left),
        (KeyCode::Right, Nav::Right),
        (KeyCode::PageUp, Nav::PageUp),
        (KeyCode::PageDown, Nav::PageDown),
        (KeyCode::Home, Nav::Home),
        (KeyCode::End, Nav::End),
    ] {
        assert_eq!(NavKeys::DESKTOP.action(&key(code)), Some(nav), "{code:?}");
        assert_eq!(NavKeys::VIM.action(&key(code)), Some(nav), "{code:?}");
    }
    assert_eq!(NavKeys::DESKTOP.action(&key(KeyCode::Char('j'))), None);
    for (letter, nav) in [
        ('k', Nav::Up),
        ('j', Nav::Down),
        ('h', Nav::Left),
        ('l', Nav::Right),
        ('K', Nav::Up),
        ('g', Nav::Home),
        ('G', Nav::End),
    ] {
        assert_eq!(NavKeys::VIM.action(&key(KeyCode::Char(letter))), Some(nav));
    }
    assert_eq!(NavKeys::VIM.action(&key(KeyCode::Char('x'))), None);
    assert_eq!(
        NavKeys::VIM.action(&typed(KeyCode::Char('j'), KeyModifiers::CONTROL)),
        None,
        "a chord is never a vim motion"
    );
    let no_arrows = NavKeys::DESKTOP.arrows(false);
    assert_eq!(no_arrows.action(&key(KeyCode::Up)), None);
    assert_eq!(
        NavKeys::DESKTOP.page(false).action(&key(KeyCode::PageUp)),
        None
    );
    assert_eq!(
        NavKeys::DESKTOP.home_end(false).action(&key(KeyCode::Home)),
        None
    );
    assert!(NavKeys::is_vim_letter(&key(KeyCode::Char('H'))));
    assert!(!NavKeys::is_vim_letter(&key(KeyCode::Char('a'))));
    assert!(!NavKeys::is_vim_letter(&key(KeyCode::Up)));

    // Hints name the keys the profile really listens to.
    assert_eq!(NavKeys::DESKTOP.move_hint(false), Some("↑↓"));
    assert_eq!(NavKeys::DESKTOP.move_hint(true), Some("↑↓"));
    let vim_only = NavKeys::VIM.arrows(false);
    assert_eq!(vim_only.move_hint(false), Some("j k"));
    assert_eq!(
        vim_only.move_hint(true),
        None,
        "type-ahead owns the letters"
    );
    assert_eq!(NavKeys::DESKTOP.page_hint(), Some("PgUp PgDn"));
    assert_eq!(NavKeys::DESKTOP.page(false).page_hint(), None);
    assert_eq!(NavKeys::DESKTOP.ends_hint(false), Some("Home End"));
    assert_eq!(NavKeys::VIM.ends_hint(false), Some("g G"));
    assert_eq!(NavKeys::VIM.ends_hint(true), Some("Home End"));
    assert_eq!(NavKeys::DESKTOP.home_end(false).ends_hint(false), None);

    // A real list (type-ahead off, so letters are free): 'j' moves only under the vim profile.
    let rows = || {
        ["Alpha", "Beta", "Gamma"]
            .into_iter()
            .map(ListItem::new)
            .collect::<Vec<_>>()
    };
    let enter = key(KeyCode::Enter);
    let mut desktop = ListBox::new(
        WidgetId::of("scenario.nav.desktop"),
        rows(),
        SelectMode::Single,
    )
    .nav(NavKeys::DESKTOP)
    .type_ahead(false);
    let _ = desktop.on_key(&key(KeyCode::Char('j')));
    let _ = desktop.on_key(&enter);
    assert_eq!(
        desktop.chosen,
        Some(0),
        "j does nothing on the desktop profile"
    );
    let mut vim = ListBox::new(WidgetId::of("scenario.nav.vim"), rows(), SelectMode::Single)
        .nav(NavKeys::VIM)
        .type_ahead(false);
    assert!(vim.on_key(&key(KeyCode::Char('j'))).consumed());
    let _ = vim.on_key(&enter);
    assert_eq!(vim.chosen, Some(1), "j moved the cursor to Beta");
    let buf = paint(Rect::new(0, 0, 20, 3), |area, buf, ctx| {
        vim.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert!(text_of(&buf).contains("Beta"));
}

struct Footer;

impl Bindings for Footer {
    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::key(KeyBinding::ctrl('s'), "Save"),
            Binding::group("a–z", "Jump"),
            Binding::key(KeyBinding::key(KeyCode::Esc), String::from("Close")),
        ]
    }
}

/// Hint bindings: what a footer or help sheet lists, derived from the keymap
/// and the navigation profile, filtered by what the terminal can deliver.
fn hint_bindings() {
    let save = Binding::key(KeyBinding::ctrl('s'), "Save");
    assert_eq!(save.key, Some(KeyBinding::ctrl('s')));
    assert_eq!(save.label, "Ctrl+S");
    assert_eq!(save.action, "Save");
    let group = Binding::group("↑↓", "Move");
    assert_eq!(
        (group.key, group.label.as_ref(), group.action.as_ref()),
        (None, "↑↓", "Move")
    );
    assert_eq!(
        Footer.chords(),
        vec![KeyBinding::ctrl('s'), KeyBinding::key(KeyCode::Esc)],
        "a group has no chord of its own"
    );
    assert_eq!(Footer.bindings().len(), 3);

    let map = Cmd::default_keymap();
    let listed = keymap_bindings(&map, |cmd| cmd.label());
    let palette_listed = listed.iter().any(|b| b.action == "Palette");
    assert_eq!(palette_listed, KeyBinding::cmd('k').deliverable());
    assert!(
        listed
            .iter()
            .any(|b| b.action == "Save" && b.label == "Ctrl+S")
    );
    assert_eq!(
        actions_bindings(&map),
        listed,
        "labels come from the Actions impl"
    );

    // Navigation rows follow the profile.
    let labels = |bindings: Vec<Binding>| {
        bindings
            .into_iter()
            .map(|b| (b.label.into_owned(), b.action.into_owned()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        labels(nav_bindings(&NavKeys::DESKTOP, false)),
        [
            ("↑↓".to_owned(), "Move".to_owned()),
            ("PgUp PgDn".to_owned(), "Page".to_owned()),
            ("Home End".to_owned(), "First / last".to_owned()),
        ]
    );
    let typing = labels(nav_bindings(&NavKeys::DESKTOP, true));
    assert_eq!(
        typing.last(),
        Some(&("a–z".to_owned(), "Jump to a row by name".to_owned()))
    );
    assert!(
        labels(nav_bindings(
            &NavKeys::DESKTOP.arrows(false).page(false).home_end(false),
            false
        ))
        .is_empty()
    );

    let desktop = nav_hints(&NavKeys::DESKTOP, false, true);
    assert_eq!(
        desktop
            .iter()
            .map(|h| (h.key.as_ref(), h.action.as_ref()))
            .collect::<Vec<_>>(),
        [("↑↓", "Move"), ("Home End", "Ends")]
    );
    assert_eq!(desktop[1].priority, HintPriority::Normal);
    assert_eq!(nav_hints(&NavKeys::DESKTOP, false, false).len(), 1);
    let typing = nav_hints(&NavKeys::DESKTOP, true, false);
    assert_eq!(typing.last(), Some(&TYPE_TO_JUMP));
    assert_eq!(
        (TYPE_TO_JUMP.key.as_ref(), TYPE_TO_JUMP.action.as_ref()),
        ("Type", "to jump")
    );
    assert!(nav_hints(&NavKeys::DESKTOP.arrows(false), false, false).is_empty());
    assert_eq!(
        nav_hints(&NavKeys::VIM.arrows(false), false, true)
            .iter()
            .map(|h| h.key.as_ref())
            .collect::<Vec<_>>(),
        ["j k", "g G"]
    );

    // The hint context and a real widget that supplies hints from it.
    let focus = Some(WidgetId::of("scenario.hints.focus"));
    let cx = HintCx::with_environment(
        focus,
        ClientPlatform::MacOs,
        KeyboardCapabilities {
            alt: false,
            super_key: true,
        },
    );
    assert_eq!(cx.focus, focus);
    assert_eq!(cx.platform, ClientPlatform::MacOs);
    assert!(cx.keyboard.super_key && !cx.keyboard.alt);
    let ambient = HintCx::new(focus);
    assert_eq!(ambient.focus, focus);
    assert_eq!(ambient.platform, ClientPlatform::current());
    assert_eq!(ambient.keyboard, KeyboardCapabilities::current());
    assert_eq!(HintCx::default().focus, None);

    let mut dialog = ChoiceDialog::new(
        WidgetId::of("scenario.hints.dialog"),
        "Pick one",
        "Mode",
        &["Fast", "Safe"],
        0,
    );
    let hints = HasHints::hints(&dialog, &HintCx::new(None));
    let keys: Vec<_> = hints.iter().map(|h| h.key.as_ref()).collect();
    assert!(keys.contains(&"↑↓") && keys.contains(&"Tab") && keys.contains(&"Esc"));
    dialog.nav = NavKeys::DESKTOP.arrows(false);
    let hints = HasHints::hints(&dialog, &HintCx::new(None));
    assert!(
        hints.iter().all(|h| h.key != "↑↓"),
        "no arrows in the profile, no arrow hint"
    );
}

/// The input vocabulary: phases, dismissal reasons and the pointer and
/// wheel intents that carry them to a widget.
fn intent_vocabulary() {
    // Every phase round-trips through the pointer event that carries it.
    let id = WidgetId::of("scenario.intent.target");
    let mut hits = HitRegistry::default();
    let row = PartRef::item(Part::ROW, ItemKey::index(3));
    hits.register_part_as(id, id, row, Rect::new(0, 0, 10, 4));
    let pos = Position::new(2, 1);
    let phases = [
        Phase::Press,
        Phase::Release,
        Phase::Click,
        Phase::Secondary,
        Phase::Middle,
        Phase::DragStart,
        Phase::Drag,
        Phase::DragEnd,
    ];
    for phase in phases {
        let event = PointerEvent::new(&hits, phase, id, pos, 2, KeyModifiers::SHIFT);
        assert_eq!(event.phase(), Some(phase));
        assert!(event.is(phase));
        let Intent::Pointer {
            phase: carried,
            part,
            pos: at,
            count,
            mods,
        } = event.intent
        else {
            panic!("a pointer event carries a pointer intent");
        };
        assert_eq!(
            (carried, part, at, count, mods),
            (phase, row, pos, 2, KeyModifiers::SHIFT)
        );
        assert_eq!(event.intent.part(), Some(row));
        assert_eq!(event.intent.pos(), Some(pos));
    }
    let simple = Intent::pointer(Phase::Click, row, pos);
    assert_eq!(
        simple,
        Intent::Pointer {
            phase: Phase::Click,
            part: row,
            pos,
            count: 1,
            mods: KeyModifiers::NONE
        }
    );

    // A wheel intent names its axis, delta, part and position.
    let wheel = PointerEvent::wheel_on(&hits, id, Axis::Horizontal, 3, pos, KeyModifiers::NONE);
    let Intent::Wheel {
        axis,
        delta,
        part,
        pos: wheel_at,
        mods,
    } = wheel.intent
    else {
        panic!("a wheel event carries a wheel intent");
    };
    assert_eq!(
        (axis, delta, part, wheel_at, mods),
        (Axis::Horizontal, 3, row, pos, KeyModifiers::NONE)
    );
    assert_eq!(wheel.intent.part(), Some(row));
    assert_eq!(wheel.intent.pos(), Some(pos));

    // The dismiss policy decides which reasons close a layer.
    let reasons = [
        DismissReason::Esc,
        DismissReason::CloseButton,
        DismissReason::OutsideClick,
        DismissReason::FocusOut,
        DismissReason::Programmatic,
    ];
    let allowed = |policy: Dismiss| reasons.map(|reason| policy.allows(reason));
    assert_eq!(allowed(Dismiss::LIGHT), [true; 5]);
    assert_eq!(allowed(Dismiss::ESCAPE), [true, true, false, false, true]);
    assert_eq!(allowed(Dismiss::NEVER), [false, false, false, false, true]);
}

const CHECK: WidgetId = WidgetId::of("scenario.pointer.check");
const RADIO: WidgetId = WidgetId::of("scenario.pointer.radio");
const VIEW: WidgetId = WidgetId::of("scenario.pointer.view");
const BACKDROP: WidgetId = WidgetId::of("scenario.pointer.backdrop");

/// A host with real widgets: a checkbox, a radio group and a scroll view
/// (with its scrollbar). Every mouse event goes through `PointerRouter`.
struct Rig {
    hits: HitRegistry,
    router: PointerRouter,
    focus: Focus,
    ring: FocusRing,
    now: u64,
    modal: bool,
    edge: Option<Rect>,
    check: Checkbox,
    radio: RadioGroup,
    view: ScrollView,
    seen: Vec<PointerEvent>,
    outside: Vec<Position>,
}

impl PointerHost for Rig {
    fn pointer_parts(&mut self) -> PointerParts<'_> {
        PointerParts {
            router: &mut self.router,
            hits: &self.hits,
            focus: &mut self.focus,
            ring: &self.ring,
        }
    }

    fn layer_open(&self) -> bool {
        self.modal
    }

    fn now_ms(&self) -> u64 {
        self.now
    }

    fn deliver(&mut self, ev: &PointerEvent) -> Response<()> {
        self.seen.push(*ev);
        if let Some(response) = self.check.route(ev) {
            return response;
        }
        if let Some(response) = self.radio.route(ev) {
            return response;
        }
        self.view
            .pointer(ev)
            .unwrap_or_else(|| Outcome::Ignored.into())
    }

    fn outside(&mut self, pos: Position) -> Response<()> {
        self.outside.push(pos);
        Outcome::Ignored.into()
    }
}

impl Rig {
    fn new() -> Self {
        Self::with(PointerConfig::DEFAULT)
    }

    fn with(config: PointerConfig) -> Self {
        let mut rig = Self {
            hits: HitRegistry::default(),
            router: PointerRouter::new(config),
            focus: Focus::default(),
            ring: FocusRing::default(),
            now: 0,
            modal: false,
            edge: None,
            check: Checkbox::new(CHECK, "Remember me", false),
            radio: RadioGroup::new(RADIO, "Mode", &["Fast", "Safe"], 0),
            view: ScrollView::new(VIEW).axes(Axes::VERTICAL),
            seen: Vec::new(),
            outside: Vec::new(),
        };
        rig.render();
        rig
    }

    fn render(&mut self) {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 14));
        self.hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        {
            let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut self.hits, &mut ring);
            let bg = ctx.theme.canvas;
            self.check
                .render(Rect::new(0, 0, 30, 1), &mut buf, &mut ctx, bg);
            self.radio
                .render(Rect::new(0, 2, 30, 3), &mut buf, &mut ctx, bg);
            self.view.render(
                Rect::new(0, 6, 30, 6),
                Size::new(28, 30),
                &mut buf,
                &mut ctx,
                |_, _, _| {},
            );
            if let Some(edge) = self.edge {
                ctx.autoscroll(edge);
            }
        }
        if self.modal {
            self.hits.push_barrier();
            self.hits
                .register_backdrop(BACKDROP, Rect::new(0, 0, 40, 14), false);
        }
        self.ring = ring;
    }

    fn mouse(&mut self, kind: MouseKind, x: u16, y: u16) -> Outcome {
        let out = PointerRouter::route(self, Mouse::new(kind, Position::new(x, y)));
        self.render();
        out
    }

    fn tick(&mut self) -> Outcome {
        let out = PointerRouter::tick(self);
        self.render();
        out
    }

    fn click(&mut self, x: u16, y: u16) {
        self.mouse(MouseKind::Down, x, y);
        self.mouse(MouseKind::Up, x, y);
    }

    fn last(&self) -> &PointerEvent {
        self.seen.last().expect("an event reached the host")
    }

    fn phases(&self) -> Vec<Phase> {
        self.seen.iter().filter_map(PointerEvent::phase).collect()
    }
}

/// Mouse events routed through real widgets: press, click, release, double
/// clicks, dead space, secondary clicks under a modal and focus policy.
fn pointer_click_routing() {
    let at = Position::new;

    // Hover follows the pointer; a second move in the same cell changes nothing.
    let mut rig = Rig::new();
    assert!(!rig.router.reports_motion());
    assert_eq!(rig.mouse(MouseKind::Move, 2, 0), Outcome::Changed);
    assert_eq!(rig.router.hover, Some(CHECK));
    assert!(rig.router.reports_motion());
    assert_eq!(rig.mouse(MouseKind::Move, 2, 0), Outcome::Ignored);

    // Press focuses and reports the press; release over the same widget is a click.
    assert_eq!(rig.mouse(MouseKind::Down, 2, 0), Outcome::Changed);
    let press = *rig.last();
    assert!(press.is(Phase::Press) && press.on(CHECK));
    assert_eq!(
        (
            press.owner(),
            press.part(),
            press.pos(),
            press.count(),
            press.mods()
        ),
        (
            CHECK,
            PartRef::of(Part::CONTAINER),
            at(2, 0),
            1,
            KeyModifiers::NONE
        )
    );
    assert_eq!(
        rig.focus.current(),
        Some(CHECK),
        "focus on press is the default"
    );
    assert_eq!(rig.router.pressed, Some(CHECK));
    assert!(rig.router.pointer_focus);
    assert!(!rig.check.checked);
    assert_eq!(rig.mouse(MouseKind::Up, 2, 0), Outcome::Changed);
    assert!(rig.check.checked, "the click toggled the checkbox");
    assert!(rig.last().is(Phase::Click));
    assert_eq!(rig.router.pressed, None);

    // A click flashes its widget until the flash expires on a tick.
    assert_eq!(rig.router.flashing(0), Some(CHECK));
    assert!(rig.router.has_flash());
    rig.now = 139;
    assert_eq!(rig.tick(), Outcome::Ignored, "still flashing");
    assert!(rig.router.has_flash());
    rig.now = 140;
    assert_eq!(rig.tick(), Outcome::Changed, "the flash ended");
    assert!(!rig.router.has_flash());
    assert_eq!(rig.tick(), Outcome::Ignored);

    // Clicks in quick succession on one spot count up; a slow or distant one starts over.
    let mut rig = Rig::new();
    let mut counts = Vec::new();
    for now in [1000, 1100, 1200] {
        rig.now = now;
        rig.click(2, 0);
        counts.push(rig.last().count());
    }
    assert_eq!(counts, [1, 2, 3]);
    rig.now = 1300;
    rig.click(8, 0);
    assert_eq!(rig.last().count(), 1, "two cells away is not the same spot");
    rig.now = 1900;
    rig.click(8, 0);
    assert_eq!(rig.last().count(), 1, "too slow for a double click");
    rig.now = 1950;
    assert_eq!(rig.router.press_count(CHECK, at(8, 0), 1950), 2);
    rig.router.reset_clicks();
    assert_eq!(rig.router.press_count(CHECK, at(8, 0), 1950), 1);

    // Releasing elsewhere cancels the click and tells the pressed widget.
    let mut rig = Rig::new();
    rig.mouse(MouseKind::Down, 2, 0);
    rig.mouse(MouseKind::Drag, 2, 3);
    assert!(
        rig.router.hover.is_some(),
        "the drag moved the hover onto the radio group"
    );
    assert_ne!(rig.router.hover, Some(CHECK));
    rig.mouse(MouseKind::Up, 2, 3);
    assert_eq!(
        rig.phases(),
        [Phase::Press, Phase::Drag, Phase::DragEnd, Phase::Release]
    );
    assert!(!rig.check.checked, "a cancelled click toggles nothing");
    let mut rig = Rig::new();
    rig.mouse(MouseKind::Down, 2, 0);
    rig.mouse(MouseKind::Up, 38, 13);
    assert_eq!(rig.phases(), [Phase::Press, Phase::Release]);
    assert!(rig.outside.is_empty());

    // Dead space: a click on nothing ends an edit; a release with no press is outside.
    let mut rig = Rig::new();
    rig.focus.focus(CHECK);
    rig.focus.take_changes();
    assert_eq!(rig.mouse(MouseKind::Down, 38, 13), Outcome::Ignored);
    assert_eq!(
        rig.focus.take_changes(),
        vec![FocusChange {
            from: CHECK,
            to: Some(CHECK)
        }],
        "the edit on the focused widget was ended"
    );
    assert_eq!(rig.mouse(MouseKind::Up, 38, 13), Outcome::Changed);
    assert_eq!(rig.outside, vec![at(38, 13)]);
    let mut rig = Rig::with(PointerConfig::DEFAULT.end_edit_on_dead_space(false));
    rig.focus.focus(CHECK);
    rig.focus.take_changes();
    rig.mouse(MouseKind::Down, 38, 13);
    assert!(
        rig.focus.take_changes().is_empty(),
        "the policy keeps the edit open"
    );

    // Focus on release instead of press.
    assert_eq!(PointerConfig::default().focus_on, FocusOn::Press);
    let mut rig = Rig::with(PointerConfig::DEFAULT.focus_on(FocusOn::Release));
    rig.mouse(MouseKind::Down, 2, 0);
    assert_eq!(rig.focus.current(), None);
    rig.mouse(MouseKind::Up, 2, 0);
    assert_eq!(rig.focus.current(), Some(CHECK));

    // A radio option is addressed by its row part; a middle click is delivered as such.
    let mut rig = Rig::new();
    rig.click(2, 4);
    assert_eq!(rig.radio.selected, 1, "the second option was clicked");
    let click = *rig.last();
    assert_eq!(click.index_of(Part::ROW), Some(1));
    assert_eq!(click.index_of(Part::CELL), None);
    assert!(click.on(RADIO) && !click.on(CHECK));
    assert_eq!(rig.mouse(MouseKind::Middle, 2, 0), Outcome::Ignored);
    assert!(rig.last().is(Phase::Middle));
    let seen = rig.seen.len();
    assert_eq!(rig.mouse(MouseKind::Middle, 38, 13), Outcome::Ignored);
    assert_eq!(
        rig.seen.len(),
        seen,
        "a middle click on nothing reaches no widget"
    );

    // A secondary click focuses what it hits, unless told not to.
    let mut rig = Rig::new();
    let outcome = rig.mouse(MouseKind::Secondary, 2, 0);
    assert_eq!(outcome, Outcome::Ignored);
    assert!(rig.last().is(Phase::Secondary));
    assert_eq!(rig.focus.current(), Some(CHECK));
    assert_eq!(rig.mouse(MouseKind::Secondary, 38, 13), Outcome::Ignored);
    let mut rig = Rig::with(PointerConfig::DEFAULT.secondary_focuses(false));
    rig.mouse(MouseKind::Secondary, 2, 0);
    assert_eq!(rig.focus.current(), None);
    let hits = &rig.hits;
    assert_eq!(
        rig.router.route_secondary(hits, at(2, 0)),
        SecondaryRoute::Target(CHECK)
    );
    assert_eq!(
        rig.router.route_secondary(hits, at(38, 13)),
        SecondaryRoute::Nothing
    );

    // Under a modal, a secondary click outside names the backdrop and,
    // by policy, reopens beneath it, closes, or is swallowed.
    assert_eq!(
        PointerConfig::default().secondary_outside,
        SecondaryOutside::Reopen
    );
    for (policy, route, delivered) in [
        (
            SecondaryOutside::Reopen,
            SecondaryRoute::Outside {
                backdrop: BACKDROP,
                reopen: Some(CHECK),
            },
            vec![Phase::Click, Phase::Secondary],
        ),
        (
            SecondaryOutside::Close,
            SecondaryRoute::Outside {
                backdrop: BACKDROP,
                reopen: None,
            },
            vec![Phase::Click],
        ),
        (SecondaryOutside::Ignore, SecondaryRoute::Swallowed, vec![]),
    ] {
        let mut rig = Rig::with(PointerConfig::DEFAULT.secondary_outside(policy));
        rig.modal = true;
        rig.render();
        let hits = &rig.hits;
        assert_eq!(
            rig.router.route_secondary(hits, at(2, 0)),
            route,
            "{policy:?}"
        );
        let outcome = rig.mouse(MouseKind::Secondary, 2, 0);
        assert_eq!(rig.phases(), delivered, "{policy:?}");
        if policy == SecondaryOutside::Ignore {
            assert_eq!(outcome, Outcome::Consumed);
        }
        assert_eq!(
            rig.focus.current(),
            None,
            "a modal keeps focus where it was"
        );
    }

    // A modal swallows presses outside everything and reports backdrop clicks.
    let mut rig = Rig::new();
    rig.modal = true;
    rig.render();
    assert_eq!(rig.mouse(MouseKind::Down, 60, 30), Outcome::Consumed);
    rig.click(38, 13);
    assert!(rig.last().is(Phase::Click) && rig.last().on(BACKDROP));
    assert_eq!(rig.last().part(), PartRef::of(Part::BACKDROP));
}

/// Wheel, scrollbar, track repeat and autoscroll gestures on a real scroll view.
fn pointer_scroll_gestures() {
    let at = Position::new;

    // The wheel scrolls the view under the pointer by the configured lines.
    let mut rig = Rig::new();
    assert_eq!(rig.view.offset(), (0, 0));
    assert_eq!(rig.router.wheel_delta(MouseKind::WheelDown), 3);
    assert_eq!(rig.router.wheel_delta(MouseKind::WheelRight), 3);
    assert_eq!(rig.router.wheel_delta(MouseKind::WheelUp), -3);
    assert_eq!(rig.router.wheel_delta(MouseKind::WheelLeft), -3);
    rig.mouse(MouseKind::Move, 2, 0);
    assert_eq!(rig.router.hover, Some(CHECK));
    assert_eq!(rig.mouse(MouseKind::WheelDown, 5, 8), Outcome::Changed);
    assert_eq!(rig.view.offset(), (0, 3));
    let wheel = *rig.last();
    assert_eq!(wheel.wheel(), Some((Axis::Vertical, 3)));
    assert!(wheel.on(VIEW) && !wheel.is(Phase::Press));
    assert_eq!(wheel.phase(), None);
    assert_eq!(wheel.count(), 0);
    assert_eq!(wheel.mods(), KeyModifiers::NONE);
    assert_eq!(rig.mouse(MouseKind::WheelUp, 5, 8), Outcome::Changed);
    assert_eq!(rig.view.offset(), (0, 0));
    assert_eq!(rig.last().wheel(), Some((Axis::Vertical, -3)));
    let seen = rig.seen.len();
    assert_eq!(rig.mouse(MouseKind::WheelDown, 38, 13), Outcome::Ignored);
    assert_eq!(rig.seen.len(), seen, "nothing scrollable under the pointer");
    rig.mouse(MouseKind::WheelRight, 5, 8);
    assert_eq!(rig.last().wheel(), Some((Axis::Horizontal, 3)));

    // After a scroll the hover is refreshed to what is now under the pointer.
    let mut rig = Rig::new();
    rig.mouse(MouseKind::Move, 2, 0);
    rig.mouse(MouseKind::WheelDown, 5, 8);
    let hits = &rig.hits;
    rig.router.refresh_hover(hits);
    assert_eq!(rig.router.hover, rig.hits.hover_at(at(5, 8)));
    assert_ne!(rig.router.hover, Some(CHECK));
    let mut rig = Rig::with(
        PointerConfig::DEFAULT
            .hover_refresh_after_scroll(false)
            .wheel_lines(5),
    );
    rig.mouse(MouseKind::Move, 2, 0);
    rig.mouse(MouseKind::WheelDown, 5, 8);
    assert_eq!(rig.view.offset(), (0, 5), "five lines per wheel step");
    let hits = &rig.hits;
    rig.router.refresh_hover(hits);
    assert_eq!(rig.router.hover, Some(CHECK), "the refresh is switched off");

    // The scrollbar: grab the thumb, drag, release.
    let mut rig = Rig::new();
    let bar = rig.hits.bars()[0];
    let thumb = rig
        .hits
        .regions()
        .iter()
        .find(|r| r.part.part == Part::THUMB)
        .map(|r| r.area)
        .expect("an overflowing view has a thumb");
    assert!(bar.track.contains(thumb.as_position()));
    assert_eq!(rig.router.grabbed(), None);
    rig.mouse(MouseKind::Down, thumb.x, thumb.y);
    assert_eq!(rig.router.grabbed().map(|b| b.track), Some(bar.track));
    assert!(rig.last().bar.is_some_and(|b| b.owner == VIEW));
    rig.mouse(MouseKind::Drag, thumb.x, thumb.y + 3);
    assert!(rig.view.offset().1 > 0, "dragging the thumb scrolls");
    rig.mouse(MouseKind::Up, thumb.x, thumb.y + 3);
    assert_eq!(rig.router.grabbed(), None, "the grab ends with the press");
    let moved = rig.view.offset().1;

    // Holding the track below the thumb repeats the page step after a delay.
    let mut rig = Rig::new();
    let bar = rig.hits.bars()[0];
    let (x, y) = (bar.track.x, bar.track.bottom() - 1);
    rig.mouse(MouseKind::Down, x, y);
    let first = rig.view.offset().1;
    assert!(first > 0, "the press paged the view");
    assert!(rig.router.track_repeating());
    rig.now = 0;
    assert_eq!(
        rig.tick(),
        Outcome::Ignored,
        "the delay starts at the first tick"
    );
    rig.now = 299;
    assert_eq!(rig.tick(), Outcome::Ignored);
    assert_eq!(rig.view.offset().1, first);
    rig.now = 300;
    assert_ne!(rig.tick(), Outcome::Ignored);
    assert!(rig.view.offset().1 > first, "the repeat paged again");
    rig.mouse(MouseKind::Up, x, y);
    assert!(!rig.router.track_repeating());
    assert!(moved > 0);
    let mut rig = Rig::with(PointerConfig::DEFAULT.track_repeat(false));
    let bar = rig.hits.bars()[0];
    rig.mouse(MouseKind::Down, bar.track.x, bar.track.bottom() - 1);
    assert!(!rig.router.track_repeating());
    let mut rig = Rig::with(PointerConfig::DEFAULT.track_repeat_ms(50, 10));
    let bar = rig.hits.bars()[0];
    rig.mouse(MouseKind::Down, bar.track.x, bar.track.bottom() - 1);
    let first = rig.view.offset().1;
    rig.now = 0;
    rig.tick();
    rig.now = 49;
    rig.tick();
    assert_eq!(rig.view.offset().1, first);
    rig.now = 50;
    rig.tick();
    let second = rig.view.offset().1;
    assert!(second > first, "the short delay elapsed");
    rig.now = 59;
    rig.tick();
    assert_eq!(rig.view.offset().1, second);
    rig.now = 60;
    rig.tick();
    assert!(
        rig.view.offset().1 > second,
        "the repeat interval is ten ms"
    );

    // Autoscroll: dragging out of an edge keeps delivering drags on ticks.
    let mut rig = Rig::new();
    rig.edge = Some(Rect::new(0, 0, 30, 1));
    rig.render();
    rig.mouse(MouseKind::Down, 2, 0);
    assert!(
        !rig.router.autoscrolling(),
        "the pointer is still inside the edge"
    );
    rig.mouse(MouseKind::Drag, 2, 5);
    assert!(rig.router.autoscrolling());
    let drags = |rig: &Rig| rig.phases().iter().filter(|p| **p == Phase::Drag).count();
    let before = drags(&rig);
    rig.now = 0;
    rig.tick();
    assert_eq!(drags(&rig), before + 1, "a tick delivers a drag");
    rig.now = 49;
    rig.tick();
    assert_eq!(drags(&rig), before + 1, "not before the interval");
    rig.now = 50;
    rig.tick();
    assert_eq!(drags(&rig), before + 2);
    rig.mouse(MouseKind::Up, 2, 5);
    assert!(!rig.router.autoscrolling());
    let mut rig = Rig::with(PointerConfig::DEFAULT.autoscroll(false));
    rig.edge = Some(Rect::new(0, 0, 30, 1));
    rig.render();
    rig.mouse(MouseKind::Down, 2, 0);
    rig.mouse(MouseKind::Drag, 2, 5);
    assert!(!rig.router.autoscrolling(), "autoscroll is switched off");
    let mut rig = Rig::with(PointerConfig::DEFAULT.autoscroll_ms(10));
    rig.edge = Some(Rect::new(0, 0, 30, 1));
    rig.render();
    rig.mouse(MouseKind::Down, 2, 0);
    rig.mouse(MouseKind::Drag, 2, 5);
    let before = drags(&rig);
    rig.now = 0;
    rig.tick();
    rig.now = 10;
    rig.tick();
    assert_eq!(drags(&rig), before + 2, "a ten ms interval");

    // Wheel and bar events drive any scrollable through `scroll_input`.
    let mut rig = Rig::new();
    let wheel = PointerEvent::wheel_on(
        &rig.hits,
        VIEW,
        Axis::Vertical,
        2,
        at(1, 7),
        KeyModifiers::NONE,
    );
    let response = scroll_input(&mut rig.view, &wheel).expect("a wheel event scrolls");
    assert_eq!(response.outcome(), Outcome::Changed);
    assert_eq!(rig.view.offset(), (0, 2));
    let click = PointerEvent::new(
        &rig.hits,
        Phase::Click,
        CHECK,
        at(1, 0),
        1,
        KeyModifiers::NONE,
    );
    assert!(
        scroll_input(&mut rig.view, &click).is_none(),
        "not a scroll event"
    );
    let bar = rig.hits.bars()[0];
    let press = PointerEvent::new(
        &rig.hits,
        Phase::Press,
        bar.id,
        at(bar.track.x, bar.track.bottom() - 1),
        1,
        KeyModifiers::NONE,
    );
    assert!(
        scroll_input(&mut rig.view, &press).is_some(),
        "a press on the bar is a scroll gesture"
    );
    let drag_start = PointerEvent::new(
        &rig.hits,
        Phase::DragStart,
        bar.id,
        at(bar.track.x, bar.track.bottom() - 1),
        1,
        KeyModifiers::NONE,
    );
    assert!(
        scroll_input(&mut rig.view, &drag_start).is_some(),
        "the start of a drag on the bar is a scroll gesture"
    );
    let moved = PointerEvent::new(
        &rig.hits,
        Phase::Middle,
        bar.id,
        at(bar.track.x, bar.track.bottom() - 1),
        1,
        KeyModifiers::NONE,
    );
    assert!(
        scroll_input(&mut rig.view, &moved).is_none(),
        "a middle click on the bar is not a scroll gesture"
    );
}

/// Pointer state and configuration driven directly against a hit registry.
fn pointer_state_policies() {
    const FIRST: WidgetId = WidgetId::of("scenario.state.first");
    const SECOND: WidgetId = WidgetId::of("scenario.state.second");
    const SURFACE: WidgetId = WidgetId::of("scenario.state.surface");
    const TRACK: WidgetId = WidgetId::of("scenario.state.track");
    let at = Position::new;

    // The defaults, and what each builder changes.
    let default = PointerConfig::DEFAULT;
    assert_eq!(PointerConfig::default(), default);
    assert_eq!(
        (
            default.focus_on,
            default.double_click_ms,
            default.double_click_slop,
            default.wheel_lines
        ),
        (FocusOn::Press, 500, 1, 3)
    );
    assert_eq!(
        (
            default.flash_ms,
            default.autoscroll_ms,
            default.track_repeat_ms
        ),
        (140, 50, 50)
    );
    assert!(default.hover_refresh_after_scroll && default.secondary_focuses);
    assert!(default.end_edit_on_dead_space && default.autoscroll && default.track_repeat);
    assert!(default.pointer_shapes && default.click_skips_animation);
    assert_eq!(default.caret_shape, CaretShape::Bar);
    let tuned = default
        .focus_on(FocusOn::Release)
        .double_click_ms(300)
        .double_click_slop(3)
        .wheel_lines(5)
        .hover_refresh_after_scroll(false)
        .flash_ms(60)
        .secondary_outside(SecondaryOutside::Close)
        .secondary_focuses(false)
        .pointer_shapes(false)
        .caret_shape(CaretShape::Block)
        .end_edit_on_dead_space(false)
        .click_skips_animation(false)
        .autoscroll(false)
        .autoscroll_ms(20)
        .track_repeat(false)
        .track_repeat_ms(100, 10);
    assert_eq!(
        (
            tuned.focus_on,
            tuned.double_click_ms,
            tuned.double_click_slop,
            tuned.wheel_lines
        ),
        (FocusOn::Release, 300, 3, 5)
    );
    assert_eq!((tuned.flash_ms, tuned.autoscroll_ms), (60, 20));
    assert_eq!(
        (tuned.track_repeat_delay_ms, tuned.track_repeat_ms),
        (100, 10)
    );
    assert_eq!(tuned.secondary_outside, SecondaryOutside::Close);
    assert!(!tuned.hover_refresh_after_scroll && !tuned.secondary_focuses);
    assert!(!tuned.end_edit_on_dead_space && !tuned.autoscroll && !tuned.track_repeat);
    assert!(!tuned.click_skips_animation && !tuned.pointer_shapes);
    assert_eq!(tuned.caret_shape, CaretShape::Block);

    let mut hits = HitRegistry::default();
    hits.register(FIRST, Rect::new(0, 0, 10, 3));
    hits.register(SECOND, Rect::new(0, 4, 10, 3));
    hits.set_shape(FIRST, PointerShape::Pointer);

    // Shapes follow the config and the hovered widget.
    let mut state = PointerState::new(default);
    state.on_move(&hits, at(1, 1));
    let shapes = shapes_for(&state, &hits);
    assert_eq!(shapes.pointer, Some(PointerShape::Pointer));
    assert_eq!(shapes.caret, Some(CaretShape::Bar));
    let mut quiet = PointerState::new(tuned);
    quiet.on_move(&hits, at(1, 1));
    let shapes = shapes_for(&quiet, &hits);
    assert_eq!(shapes.pointer, None, "pointer shapes are off");
    assert_eq!(shapes.caret, Some(CaretShape::Block));

    // Moving, hover time and suppression.
    let mut state = PointerState::new(default);
    assert!(!state.reports_motion());
    assert_eq!(state.on_move(&hits, at(1, 1)), Outcome::Changed);
    assert!(state.reports_motion());
    assert_eq!((state.hover, state.pos), (Some(FIRST), Some(at(1, 1))));
    assert_eq!(state.on_move(&hits, at(2, 1)), Outcome::Ignored);
    assert_eq!(state.hover_ms(100), 0, "the clock starts at the first look");
    assert_eq!(state.hover_ms(350), 250);
    assert!(state.suppress_hover());
    assert_eq!(state.hover_ms(400), 0, "a suppressed hover has no age");
    assert_eq!(
        state.on_move(&hits, at(2, 1)),
        Outcome::Changed,
        "moving restores hover"
    );
    state.focused_by_click();
    assert!(state.pointer_focus);
    assert!(
        state.suppress_hover(),
        "a key press after a click is a visual change"
    );
    assert!(!state.pointer_focus);

    // Press, drag, release, with acceptance of a release elsewhere.
    let mut state = PointerState::new(default);
    assert_eq!(state.on_down(&hits, at(1, 1)), Some(FIRST));
    assert_eq!((state.pressed, state.hover), (Some(FIRST), Some(FIRST)));
    assert_eq!(state.on_drag(&hits, at(1, 5)), Some(FIRST));
    assert_eq!(state.hover, Some(SECOND));
    assert_eq!(state.on_up(&hits, at(1, 5)), (Some(SECOND), None));
    assert_eq!(state.release(Some(SECOND)), Release::Cancelled);
    assert_eq!(state.pressed, None);
    state.press(FIRST);
    assert_eq!(state.pressed, Some(FIRST));
    assert_eq!(state.release(None), Release::Cancelled);
    assert_eq!(state.release(None), Release::Outside, "nothing was pressed");
    state.press(FIRST);
    state.on_up(&hits, at(1, 1));
    assert_eq!(state.release(Some(FIRST)), Release::Click(FIRST));
    hits.accept_release(SECOND, ReleaseFrom::Press(FIRST));
    state.press(FIRST);
    state.on_up(&hits, at(1, 5));
    assert_eq!(
        state.release(Some(SECOND)),
        Release::Click(SECOND),
        "SECOND accepts a release from FIRST's press"
    );

    // Click counting: window and slop come from the config.
    let mut state = PointerState::new(tuned);
    assert_eq!(state.press_count(FIRST, at(1, 1), 0), 1);
    assert_eq!(state.count_click(FIRST, at(1, 1), 0), 1);
    assert_eq!(
        state.count_click(FIRST, at(3, 1), 299),
        2,
        "slop is three cells"
    );
    assert_eq!(state.press_count(FIRST, at(3, 1), 400), 3);
    assert_eq!(state.press_count(FIRST, at(3, 1), 600), 1, "past 300 ms");
    assert_eq!(
        state.press_count(SECOND, at(3, 1), 400),
        1,
        "another widget"
    );
    assert_eq!(state.press_count(FIRST, at(7, 1), 400), 1, "too far");
    state.reset_clicks();
    assert_eq!(state.press_count(FIRST, at(3, 1), 400), 1);

    // Flashing lasts the configured time.
    let mut state = PointerState::new(default);
    assert!(!state.has_flash());
    state.flash(FIRST, 1000);
    assert!(state.has_flash());
    assert_eq!(state.flashing(1139), Some(FIRST));
    assert_eq!(state.flashing(1140), None);
    assert!(!state.expire_flash(1139));
    assert!(state.expire_flash(1140));
    assert!(!state.has_flash());

    // Secondary: press, drag, release over a widget that accepts it.
    let mut state = PointerState::new(default);
    assert_eq!(state.on_secondary(&hits, at(1, 1)), Some(FIRST));
    assert_eq!(state.pressed, None, "a secondary press is not a press");
    assert_eq!(state.on_secondary_drag(&hits, at(1, 5)), Outcome::Changed);
    assert_eq!(state.on_secondary_drag(&hits, at(2, 5)), Outcome::Ignored);
    hits.accept_release(SECOND, ReleaseFrom::Secondary);
    assert_eq!(state.on_secondary_up(&hits, at(1, 5)), Some(SECOND));
    state.on_secondary(&hits, at(1, 5));
    assert_eq!(
        state.on_secondary_up(&hits, at(1, 5)),
        None,
        "released where it began"
    );
    assert_eq!(
        state.on_secondary_up(&hits, at(1, 5)),
        None,
        "no secondary press is open"
    );
    assert_eq!(
        state.route_secondary(&hits, at(1, 5)),
        SecondaryRoute::Target(SECOND)
    );
    assert_eq!(
        state.route_secondary(&hits, at(30, 30)),
        SecondaryRoute::Nothing
    );

    // Surfaces and dead space end an edit only when the policy says so.
    hits.register_surface(SURFACE, Rect::new(0, 8, 10, 2));
    let mut focus = Focus::default();
    focus.focus(FIRST);
    focus.take_changes();
    let state = PointerState::new(default);
    assert!(
        !state.surface_press(&hits, FIRST, &mut focus),
        "not a surface"
    );
    assert!(focus.take_changes().is_empty());
    assert!(state.surface_press(&hits, SURFACE, &mut focus));
    assert_eq!(
        focus.take_changes().len(),
        1,
        "the surface press ended the edit"
    );
    PointerState::new(tuned).dead_space(&mut focus);
    assert!(focus.take_changes().is_empty());
    state.dead_space(&mut focus);
    assert_eq!(focus.take_changes().len(), 1);

    // Wheel deltas and the hover refresh a scroll asks for.
    let mut state = PointerState::new(tuned);
    assert_eq!(state.wheel_delta(MouseKind::WheelUp), -5);
    assert_eq!(state.wheel_delta(MouseKind::WheelRight), 5);
    state.on_move(&hits, at(1, 1));
    state.scrolled(at(1, 5), Outcome::Changed);
    state.refresh_hover(&hits);
    assert_eq!(state.hover, Some(FIRST), "tuned config does not refresh");
    let mut state = PointerState::new(default);
    state.on_move(&hits, at(1, 1));
    state.scrolled(at(1, 5), Outcome::Consumed);
    state.refresh_hover(&hits);
    assert_eq!(state.hover, Some(FIRST), "only a change refreshes");
    state.scrolled(at(1, 5), Outcome::Changed);
    state.refresh_hover(&hits);
    assert_eq!(state.hover, Some(SECOND));

    // Autoscroll and track repeat come from edges and bars in the registry.
    let mut edges = HitRegistry::default();
    edges.register(FIRST, Rect::new(0, 0, 10, 3));
    edges.register_autoscroll(Rect::new(0, 0, 10, 3));
    let mut state = PointerState::new(default);
    state.on_down(&edges, at(1, 1));
    assert!(!state.autoscrolling());
    state.on_drag(&edges, at(1, 6));
    state.pos = Some(at(1, 6));
    assert!(state.autoscrolling());
    assert_eq!(state.autoscroll_due(0), Some((FIRST, at(1, 6))));
    assert_eq!(state.autoscroll_due(49), None);
    assert_eq!(state.autoscroll_due(50), Some((FIRST, at(1, 6))));
    state.release(None);
    assert_eq!(
        state.autoscroll_due(100),
        None,
        "released: nothing to scroll"
    );

    let mut bars = HitRegistry::default();
    bars.register_bar(Bar {
        id: TRACK,
        owner: SECOND,
        axis: Axis::Vertical,
        track: Rect::new(9, 0, 1, 8),
    });
    let mut state = PointerState::new(default);
    assert_eq!(state.on_down(&bars, at(9, 6)), Some(TRACK));
    assert_eq!(state.grabbed().map(|b| b.id), Some(TRACK));
    assert!(state.track_repeating());
    assert_eq!(state.track_repeat_due(0), None, "the delay starts now");
    assert_eq!(state.track_repeat_due(299), None);
    assert_eq!(state.track_repeat_due(300), Some(at(9, 6)));
    assert_eq!(state.track_repeat_due(349), None);
    assert_eq!(state.track_repeat_due(350), Some(at(9, 6)));
    state.release(Some(TRACK));
    assert_eq!(state.track_repeat_due(400), None);
    assert!(!state.track_repeating());
}

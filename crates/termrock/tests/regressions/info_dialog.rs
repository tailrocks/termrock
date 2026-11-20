use crate::common::*;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use termrock::core::focus::Focus;
use termrock::core::focus::FocusRing;
use termrock::core::hit::HitRegistry;
use termrock::core::id::WidgetId;
use termrock::theme::{ColorLevel, Theme};
use termrock::ui::ctx::{Interaction, RenderCtx};
use termrock::widgets::info_dialog::InfoDialog;
use termrock::widgets::props::Prop;

const ID: WidgetId = WidgetId::of("t.info");

fn dialog(screen: Rect) -> Buffer {
    let mut props: Vec<Prop> = (0..10)
        .map(|n| Prop::new(&format!("prop{n:02}"), format!("value {n:02}")))
        .collect();
    let long = "a very long note that needs several rows to be read whole ".repeat(6);
    props.push(Prop::new("note", long).wrap());
    let mut info = InfoDialog::new(ID, "Info", props);
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(screen);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    info.render(screen, &mut buf, &mut ctx);
    buf
}

#[test]
fn a_wrapped_value_below_the_window_does_not_shrink_the_rows_shown() {
    let screen = Rect::new(0, 0, 80, 18);
    let buf = dialog(screen);
    let text = (0..screen.height)
        .map(|y| row(&buf, y))
        .collect::<Vec<_>>()
        .join("\n");
    let shown = (0..10)
        .filter(|n| text.contains(&format!("prop{n:02}")))
        .count();
    assert!(
        shown == 10,
        "the window fills the dialog, only {shown} props shown:\n{text}"
    );
}

fn lines_of(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height).map(|y| row(buf, y)).collect()
}

#[test]
fn walking_the_keyboard_onto_a_wrapped_last_prop_keeps_the_cursor_drawn() {
    let screen = Rect::new(0, 0, 80, 14);
    let mut props: Vec<Prop> = (0..9)
        .map(|n| Prop::new(&format!("prop{n:02}"), format!("value {n:02}")))
        .collect();
    let long = "a very long note that needs several rows to be read whole ".repeat(4);
    props.push(Prop::new("note", long).wrap());
    let mut info = InfoDialog::new(ID, "Info", props);
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut focus = Focus::default();
    focus.focus(info.initial_focus());
    let paint = |info: &mut InfoDialog, focus: &Focus| {
        let mut buf = Buffer::empty(screen);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let interaction = Interaction {
            focus: focus.current(),
            ..Default::default()
        };
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        info.render(screen, &mut buf, &mut ctx);
        (buf, ring)
    };
    let (_, mut ring) = paint(&mut info, &focus);
    for step in 1..=9 {
        info.on_key(&key(KeyCode::Down), &mut focus, &ring);
        let (buf, next) = paint(&mut info, &focus);
        ring = next;
        let lines = lines_of(&buf);
        let label = if step == 9 {
            "note".to_owned()
        } else {
            format!("prop{step:02}")
        };
        assert!(
            lines.iter().any(|l| l.contains('▎') && l.contains(&label)),
            "step {step}: the marker sits on `{label}`:\n{}",
            lines.join("\n")
        );
    }
    let text = lines_of(&paint(&mut info, &focus).0).join("\n");
    assert!(
        text.contains("read whole"),
        "the wrapped value of the last prop is drawn:\n{text}"
    );
}

mod dismiss {
    use super::*;
    use termrock::core::edit::EditEntry;
    use termrock::core::event::Outcome;
    use termrock::core::hints::{HasHints, HintCx};
    use termrock::ui::layer::{Layer, LayerCx, LayerTag, ShellLayers, Taken};
    use termrock::ui::layers::LayerResult;
    use termrock::ui::popup::Dismiss;
    use termrock::widgets::choice_dialog::ChoiceDialog;
    use termrock::widgets::form_dialog::{FormAction, FormDialog, FormField};
    use termrock::widgets::info_dialog::InfoResult;
    use termrock::widgets::input::TextInput;
    use termrock::widgets::select::Select;

    const CHOICE: WidgetId = WidgetId::of("t.choice");
    const FORM: WidgetId = WidgetId::of("t.form");

    fn choice(dismiss: Dismiss) -> ChoiceDialog {
        ChoiceDialog::new(CHOICE, "Pick", "Channel", &["Stable", "Beta"], 0).dismiss(dismiss)
    }

    fn info(dismiss: Dismiss) -> InfoDialog {
        InfoDialog::new(ID, "Info", vec![Prop::new("Host", "db")]).dismiss(dismiss)
    }

    fn form(dismiss: Dismiss) -> FormDialog {
        FormDialog::new(
            FORM,
            "Form",
            vec![
                FormField::input(
                    "name",
                    TextInput::new(FORM.sub("name"), "Name")
                        .value("x")
                        .edit_entry(EditEntry::Explicit),
                ),
                FormField::select(
                    "driver",
                    Select::new(FORM.sub("driver"), "Driver", &["A", "B"], 0),
                ),
            ],
        )
        .dismiss(dismiss)
    }

    fn escape() -> termrock::core::event::Key {
        key(KeyCode::Esc)
    }

    fn focused(id: WidgetId) -> Focus {
        let mut focus = Focus::default();
        focus.focus(id);
        focus
    }

    #[test]
    fn choice_escape_policy_preserves_cancel_contract() {
        let ring = FocusRing::default();
        for id in [CHOICE.sub("radio"), CHOICE.sub("cancel"), CHOICE.sub("ok")] {
            let mut never = choice(Dismiss::NEVER);
            let o = never.on_key(&escape(), &mut focused(id), &ring);
            assert_eq!(o, Outcome::Consumed, "refused Esc on {id:?}");
            assert_eq!(never.result, None, "refused Esc leaves no result");
            assert_eq!(never.fired, None, "refused Esc fires no button");
        }
        let mut allowed = choice(Dismiss::ESCAPE);
        let o = allowed.on_key(&escape(), &mut focused(CHOICE.sub("radio")), &ring);
        assert_eq!(o, Outcome::Changed);
        assert_eq!(allowed.result, Some(None));
        assert_eq!(allowed.fired, Some(allowed.cancel_index));
    }

    #[test]
    fn explicit_choice_cancel_survives_a_refused_escape_policy() {
        let ring = FocusRing::default();
        let mut d = choice(Dismiss::NEVER);
        let mut focus = focused(CHOICE.sub("cancel"));
        d.on_key(&key(KeyCode::Enter), &mut focus, &ring);
        assert_eq!(d.result, Some(None));
        assert_eq!(d.fired, Some(d.cancel_index));
    }

    #[test]
    fn info_escape_policy_keeps_explicit_close() {
        let ring = FocusRing::default();
        let mut never = info(Dismiss::NEVER);
        let mut focus = focused(never.initial_focus());
        assert_eq!(
            never.on_key(&escape(), &mut focus, &ring),
            Outcome::Consumed
        );
        assert_eq!(never.result, None, "refused Esc writes no result");

        let mut focus = focused(never.close.id);
        never.on_key(&key(KeyCode::Enter), &mut focus, &ring);
        assert_eq!(never.result, Some(InfoResult::Closed), "Close button works");

        let mut allowed = info(Dismiss::ESCAPE);
        let mut focus = focused(allowed.initial_focus());
        assert_eq!(
            allowed.on_key(&escape(), &mut focus, &ring),
            Outcome::Changed
        );
        assert_eq!(allowed.result, Some(InfoResult::Closed));
    }

    #[test]
    fn form_idle_escape_respects_policy() {
        let ring = FocusRing::default();
        for id in [FORM.sub("cancel"), FORM.sub("save"), FORM.sub("driver")] {
            let mut never = form(Dismiss::NEVER);
            let o = never.on_key(&escape(), &mut focused(id), &ring);
            assert_eq!(o, Outcome::Consumed, "refused Esc on {id:?}");
            assert!(never.events.is_empty(), "no FormAction on refusal");
        }
        let mut allowed = form(Dismiss::ESCAPE);
        let o = allowed.on_key(&escape(), &mut focused(FORM.sub("save")), &ring);
        assert_eq!(o, Outcome::Changed);
        assert_eq!(allowed.events, vec![FormAction::Cancel]);
    }

    #[test]
    fn form_child_escape_precedes_dialog_policy() {
        let ring = FocusRing::default();
        for (dismiss, cancels) in [(Dismiss::NEVER, false), (Dismiss::ESCAPE, true)] {
            let mut d = form(dismiss);
            let mut focus = focused(FORM.sub("name"));
            d.on_key(&key(KeyCode::Enter), &mut focus, &ring);
            assert!(d.is_editing(), "Enter begins editing");
            d.on_key(&key(KeyCode::Char('y')), &mut focus, &ring);
            d.on_key(&escape(), &mut focus, &ring);
            assert!(!d.is_editing(), "first Esc cancels the dirty edit");
            assert!(d.events.is_empty(), "first Esc never reaches the form");
            let o = d.on_key(&escape(), &mut focus, &ring);
            assert_eq!(d.events.len(), usize::from(cancels));
            assert_eq!(o == Outcome::Changed, cancels);
        }
    }

    #[test]
    fn form_clean_always_on_edit_escape_respects_policy() {
        let ring = FocusRing::default();
        for (dismiss, cancels) in [(Dismiss::NEVER, false), (Dismiss::ESCAPE, true)] {
            let mut d = FormDialog::new(
                FORM,
                "Form",
                vec![FormField::input(
                    "name",
                    TextInput::new(FORM.sub("name"), "Name").value("x"),
                )],
            )
            .dismiss(dismiss);
            let mut focus = focused(FORM.sub("name"));
            d.on_key(&key(KeyCode::Enter), &mut focus, &ring);
            assert!(d.is_editing());
            d.events.clear();
            d.on_key(&escape(), &mut focus, &ring);
            assert_eq!(d.events.len(), usize::from(cancels));
        }
    }

    #[test]
    fn form_open_select_escape_closes_popup_before_dialog_policy() {
        let ring = FocusRing::default();
        let mut d = form(Dismiss::NEVER);
        let mut focus = focused(FORM.sub("driver"));
        d.on_key(&key(KeyCode::Enter), &mut focus, &ring);
        assert!(
            matches!(&d.fields[1].kind, termrock::widgets::form_dialog::FormFieldKind::Select(s) if s.open)
        );
        d.on_key(&escape(), &mut focus, &ring);
        assert!(
            matches!(&d.fields[1].kind, termrock::widgets::form_dialog::FormFieldKind::Select(s) if !s.open),
            "Esc closes the popup"
        );
        assert!(d.events.is_empty());
    }

    #[test]
    fn dialog_refused_escape_does_not_erase_prior_result() {
        let ring = FocusRing::default();
        let mut d = choice(Dismiss::NEVER);
        let mut focus = focused(CHOICE.sub("ok"));
        d.on_key(&key(KeyCode::Enter), &mut focus, &ring);
        let before = (d.result, d.fired);
        assert_eq!(before.0, Some(Some(0)));
        d.on_key(&escape(), &mut focus, &ring);
        assert_eq!((d.result, d.fired), before);

        let mut i = info(Dismiss::NEVER);
        let mut focus = focused(i.close.id);
        i.on_key(&key(KeyCode::Enter), &mut focus, &ring);
        i.on_key(&escape(), &mut focus, &ring);
        assert_eq!(i.result, Some(InfoResult::Closed));
    }

    type Stack = ShellLayers<LayerResult>;

    fn open(layer: Box<dyn Layer<LayerResult>>, focus: &mut Focus) -> Stack {
        let mut layers = Stack::default();
        layers.open(layer, LayerTag::new("t"), (), focus);
        layers
    }

    fn press_escape(layers: &mut Stack, focus: &mut Focus) -> Outcome {
        let ring = FocusRing::default();
        let mut cx = LayerCx {
            focus,
            ring: &ring,
            app: &(),
        };
        layers
            .top_mut()
            .expect("a layer is open")
            .layer
            .on_key(&escape(), &mut cx)
    }

    #[test]
    fn dialog_layer_escape_retains_or_removes_stack() {
        type Make = fn(Dismiss) -> Box<dyn Layer<LayerResult>>;
        let makers: [(&str, Make, LayerResult); 3] = [
            ("choice", |d| Box::new(choice(d)), LayerResult::Choice(None)),
            (
                "info",
                |d| Box::new(info(d)),
                LayerResult::Info(InfoResult::Closed),
            ),
            (
                "form",
                |d| Box::new(form(d)),
                LayerResult::Form(FormAction::Cancel),
            ),
        ];
        for (name, make, expected) in makers {
            let mut focus = Focus::default();
            let mut layers = open(make(Dismiss::NEVER), &mut focus);
            let o = press_escape(&mut layers, &mut focus);
            assert_ne!(o, Outcome::Ignored, "{name}: refused Esc is consumed");
            assert!(layers.take(&mut focus).is_none(), "{name}: no result");
            assert_eq!(layers.len(), 1, "{name}: layer retained");

            let mut focus = Focus::default();
            let mut layers = open(make(Dismiss::ESCAPE), &mut focus);
            press_escape(&mut layers, &mut focus);
            match layers.take(&mut focus) {
                Some(Taken::Closed(_, result)) => assert_eq!(result, expected, "{name}"),
                _ => panic!("{name}: allowed Esc closes the layer"),
            }
            assert!(layers.is_empty(), "{name}: removed exactly once");
            assert!(layers.take(&mut focus).is_none(), "{name}: one result only");
        }
    }

    fn close_drawn(render: impl FnOnce(Rect, &mut Buffer, &mut RenderCtx)) -> bool {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let screen = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(screen);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        render(screen, &mut buf, &mut ctx);
        hits.regions()
            .iter()
            .any(|r| r.id == termrock::ui::modal::CLOSE)
    }

    #[test]
    fn dialog_escape_hints_match_policy() {
        let cx = HintCx::new(None);
        let has_esc =
            |hints: Vec<termrock::widgets::keyhint::Hint>| hints.iter().any(|h| h.key == "Esc");
        for (dismiss, shows) in [
            (Dismiss::NEVER, false),
            (Dismiss::ESCAPE, true),
            (Dismiss::LIGHT, true),
        ] {
            let mut c = choice(dismiss);
            let mut i = info(dismiss);
            let mut f = form(dismiss);
            assert_eq!(has_esc(HasHints::hints(&c, &cx)), shows, "choice hint");
            assert_eq!(has_esc(HasHints::hints(&i, &cx)), shows, "info hint");
            assert_eq!(has_esc(HasHints::hints(&f, &cx)), shows, "form hint");
            let chrome = dismiss.shows_close();
            assert_eq!(close_drawn(|s, b, x| c.render(s, b, x)), chrome, "choice ×");
            assert_eq!(close_drawn(|s, b, x| i.render(s, b, x)), chrome, "info ×");
            assert_eq!(close_drawn(|s, b, x| f.render(s, b, x)), chrome, "form ×");
        }
    }
}

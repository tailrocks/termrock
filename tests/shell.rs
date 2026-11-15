mod common;

use common::*;
use junie_tui::clock::TestRequest;
use junie_tui::core::event::{Input, Outcome};
use junie_tui::core::refusal::{clear_refusal, raise, take_refusal};
use junie_tui::theme::Tone;
use ratatui::crossterm::event::KeyCode;

#[test]
fn the_shell_clears_an_untaken_refusal_at_the_end_of_each_input() {
    for (name, input) in every_input() {
        clear_refusal();
        let mut host = Host::new(Act::Raise(reason()));
        host.input(input);
        assert_eq!(take_refusal(), None, "{name}");
    }
}

#[test]
fn the_shell_clears_a_stale_refusal_before_each_input() {
    for (name, input) in every_input() {
        clear_refusal();
        raise(&reason());
        let mut host = Host::new(Act::Nothing);
        host.input(input);
        assert_eq!(host.seen_at_entry, Some(None), "{name}");
    }
}

#[test]
fn a_status_posted_by_a_consumed_key_is_a_change() {
    let mut host = Host::new(Act::Status("saved", Tone::Secondary));
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Changed);
    assert_eq!(host.status(), Some(("saved".to_owned(), Tone::Secondary)));
}

#[test]
fn a_key_that_posts_nothing_is_not_a_change() {
    let mut host = Host::new(Act::Nothing);
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Ignored);
}

#[test]
fn an_error_cleared_by_the_input_is_a_change() {
    let mut host = Host::new(Act::Nothing);
    host.set_status("write failed", Tone::Error);
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Changed);
    assert_eq!(host.status(), None);
}

#[test]
fn an_identical_status_posted_again_is_not_a_change() {
    let mut host = Host::new(Act::Status("saved", Tone::Secondary));
    host.set_status("saved", Tone::Secondary);
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Consumed);
}

#[test]
fn the_same_text_in_another_tone_is_a_change() {
    let mut host = Host::new(Act::Status("saved", Tone::Warning));
    host.set_status("saved", Tone::Secondary);
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Changed);
    assert_eq!(host.status(), Some(("saved".to_owned(), Tone::Warning)));
}

#[test]
fn the_same_tone_with_other_text_is_a_change() {
    let mut host = Host::new(Act::Status("second", Tone::Secondary));
    host.set_status("first", Tone::Secondary);
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Changed);
    assert_eq!(host.status(), Some(("second".to_owned(), Tone::Secondary)));
}

#[test]
fn a_fence_is_acknowledged_once_and_never_reaches_the_app() {
    junie_tui::shell::clipboard::take_output();
    let mut host = Host::new(Act::Nothing);
    host.test_requests(true);

    let out = host.input(Input::Paste(TestRequest::FENCE.to_owned()));

    assert_eq!(out, Outcome::Ignored);
    assert_eq!(host.inputs, 0, "the app never sees a fence");
    assert_eq!(
        junie_tui::shell::clipboard::take_output(),
        [TestRequest::FENCE_ACK],
        "one acknowledgement per fence"
    );
}

#[test]
fn a_clock_step_reaches_the_app_as_a_step_and_not_as_a_paste() {
    junie_tui::shell::clipboard::take_output();
    let mut host = Host::new(Act::Nothing);
    host.test_requests(true);

    host.input(Input::Paste(TestRequest::advance(250)));
    host.input(Input::Paste(TestRequest::advance(40)));

    assert_eq!(host.stepped, [250, 40]);
    assert_eq!(host.inputs, 0);
}

#[test]
fn test_requests_are_plain_pastes_unless_the_shell_accepts_them() {
    junie_tui::shell::clipboard::take_output();
    let mut host = Host::new(Act::Nothing);
    host.test_requests(false);

    host.input(Input::Paste(TestRequest::FENCE.to_owned()));
    host.input(Input::Paste(TestRequest::advance(250)));

    assert_eq!(host.inputs, 2, "both reach the app as text");
    assert!(host.stepped.is_empty());
    assert_eq!(
        junie_tui::shell::clipboard::take_output(),
        Vec::<String>::new()
    );
}

#[test]
fn a_stop_handle_ends_the_loop_from_another_thread() {
    use junie_tui::runtime::Application;

    let host = Host::new(Act::Nothing);
    assert!(!host.should_quit());

    let stop = host.stop_handle();
    std::thread::spawn(move || stop.stop())
        .join()
        .expect("the stopping thread");

    assert!(
        host.should_quit(),
        "the loop reads should_quit after a stop"
    );
}

mod layer_focus {
    use junie_tui::core::event::{Key, Outcome};
    use junie_tui::core::focus::{Focus, FocusChange, FocusRing};
    use junie_tui::core::id::WidgetId;
    use junie_tui::ui::ctx::RenderCtx;
    use junie_tui::ui::layer::{
        DismissReason, Dismissed, Layer, LayerCx, LayerOut, LayerSpec, LayerTag, ShellLayers,
    };
    use junie_tui::ui::modal::ModalStack;
    use junie_tui::ui::popup::Dismiss;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    const A: WidgetId = WidgetId::of("lf.a");
    const B: WidgetId = WidgetId::of("lf.b");
    const C: WidgetId = WidgetId::of("lf.c");
    const D: WidgetId = WidgetId::of("lf.d");
    const E: WidgetId = WidgetId::of("lf.e");

    struct Probe {
        spec: LayerSpec,
        out: Option<LayerOut<()>>,
    }

    impl Probe {
        fn boxed(spec: LayerSpec) -> Box<dyn Layer<(), ()>> {
            Box::new(Self { spec, out: None })
        }
    }

    impl Layer<(), ()> for Probe {
        fn spec(&self) -> LayerSpec {
            self.spec
        }
        fn render(&mut self, _: Rect, _: &mut Buffer, _: &mut RenderCtx, _: &()) {}
        fn on_key(&mut self, _: &Key, _: &mut LayerCx<'_, ()>) -> Outcome {
            Outcome::Ignored
        }
        fn on_dismiss(&mut self, _: DismissReason) {}
        fn take_result(&mut self) -> Option<LayerOut<()>> {
            self.out.take()
        }
    }

    type Layers = ShellLayers<(), ()>;

    fn modal(initial: WidgetId, restore: bool) -> Box<dyn Layer<(), ()>> {
        Probe::boxed(
            LayerSpec::modal(Dismiss::LIGHT)
                .initial_focus(initial)
                .restore_focus(restore),
        )
    }

    fn focused(id: WidgetId) -> Focus {
        let mut focus = Focus::default();
        focus.set(Some(id));
        focus.take_changes();
        focus
    }

    fn finish(layers: &mut Layers) {
        layers.top_as_mut::<Probe>().expect("probe on top").out = Some(LayerOut::Close(()));
    }

    fn close_by_result(layers: &mut Layers, focus: &mut Focus) {
        finish(layers);
        layers.take(focus).expect("the layer closes");
    }

    fn ring_of(ids: &[WidgetId]) -> FocusRing {
        let mut ring = FocusRing::default();
        for id in ids {
            ring.register(*id);
        }
        ring
    }

    #[test]
    fn layer_no_restore_keeps_action_focus() {
        for (restore, after) in [(false, C), (true, A)] {
            let mut layers = Layers::new();
            let mut focus = focused(A);
            layers.open(modal(B, restore), LayerTag::new("t"), (), &mut focus);
            assert_eq!(focus.current(), Some(B));

            focus.set(Some(C));
            close_by_result(&mut layers, &mut focus);

            assert_eq!(focus.current(), Some(after), "restore_focus = {restore}");
        }
    }

    #[test]
    fn layer_no_restore_dismiss_and_clear() {
        for (restore, after) in [(false, C), (true, A)] {
            let mut layers = Layers::new();
            let mut focus = focused(A);
            layers.open(modal(B, restore), LayerTag::new("t"), (), &mut focus);
            focus.set(Some(C));
            let closed = layers.dismiss(DismissReason::Esc, &mut focus);
            assert!(matches!(closed, Dismissed::Closed(..)));
            assert_eq!(focus.current(), Some(after), "dismiss, restore = {restore}");

            let mut layers = Layers::new();
            let mut focus = focused(A);
            layers.open(modal(B, restore), LayerTag::new("t"), (), &mut focus);
            focus.set(Some(C));
            layers.clear(&mut focus);
            assert!(layers.is_empty());
            assert_eq!(focus.current(), Some(after), "clear, restore = {restore}");
        }
    }

    #[test]
    fn nested_clear_matches_top_down_pops_without_intermediate_changes() {
        let flag_sets = [
            ([true, false, true], A),
            ([false, true, false], B),
            ([false, false, false], E),
            ([true, true, true], A),
            ([true, false, false], A),
            ([false, false, true], C),
        ];
        for (flags, destination) in flag_sets {
            let open_all = |layers: &mut Layers, focus: &mut Focus| {
                for (initial, restore) in [B, C, D].into_iter().zip(flags) {
                    layers.open(modal(initial, restore), LayerTag::new("t"), (), focus);
                }
                focus.set(Some(E));
                focus.take_changes();
            };

            let mut popped = Layers::new();
            let mut popped_focus = focused(A);
            open_all(&mut popped, &mut popped_focus);
            while popped.pop(&mut popped_focus).is_some() {}

            let mut layers = Layers::new();
            let mut focus = focused(A);
            open_all(&mut layers, &mut focus);
            layers.clear(&mut focus);

            assert_eq!(focus.current(), popped_focus.current(), "flags {flags:?}");
            assert_eq!(focus.current(), Some(destination), "flags {flags:?}");
            let changes = focus.take_changes();
            let expected: Vec<FocusChange> = if focus.current() == Some(E) {
                Vec::new()
            } else {
                vec![FocusChange {
                    from: E,
                    to: focus.current(),
                }]
            };
            assert_eq!(changes, expected, "flags {flags:?}");
        }
    }

    #[test]
    fn tooltip_without_initial_focus_preserves_owner() {
        let mut layers = Layers::new();
        let mut focus = focused(A);

        layers.open(
            Probe::boxed(LayerSpec::tooltip()),
            LayerTag::new("tip"),
            (),
            &mut focus,
        );
        assert_eq!(focus.current(), Some(A), "opening the tooltip keeps focus");
        assert!(focus.take_changes().is_empty(), "no transient blur on open");

        close_by_result(&mut layers, &mut focus);
        assert_eq!(focus.current(), Some(A));
        assert!(focus.take_changes().is_empty(), "no blur on close");
    }

    #[test]
    fn tooltip_close_keeps_a_focus_the_owner_moved() {
        let mut layers = Layers::new();
        let mut focus = focused(A);
        layers.open(
            Probe::boxed(LayerSpec::tooltip()),
            LayerTag::new("tip"),
            (),
            &mut focus,
        );
        focus.set(Some(C));
        let closed = layers.dismiss(DismissReason::Esc, &mut focus);
        assert!(matches!(closed, Dismissed::Closed(..)));
        assert_eq!(focus.current(), Some(C));
    }

    #[test]
    fn modal_without_initial_focus_uses_ring_fallback() {
        let mut layers = Layers::new();
        let mut focus = focused(A);

        layers.open(
            Probe::boxed(LayerSpec::modal(Dismiss::LIGHT)),
            LayerTag::new("m"),
            (),
            &mut focus,
        );
        assert_eq!(focus.current(), None, "an inert layer clears the old focus");

        let ring = ring_of(&[B, C]);
        focus.ensure_valid(&ring);
        assert_eq!(focus.current(), Some(B), "the ring picks its first control");

        close_by_result(&mut layers, &mut focus);
        assert_eq!(focus.current(), Some(A), "the modal default restores");
    }

    #[test]
    fn retain_invalidates_removed_layer_return_targets() {
        let mut layers = Layers::new();
        let mut focus = focused(A);
        layers.open(modal(B, true), LayerTag::new("low"), (), &mut focus);
        layers.open(modal(C, true), LayerTag::new("top"), (), &mut focus);
        assert_eq!(focus.current(), Some(C));

        layers.retain(|e| e.tag.kind != "low");
        assert_eq!(layers.len(), 1);
        assert_eq!(focus.current(), Some(C), "retain leaves focus alone");

        close_by_result(&mut layers, &mut focus);
        assert_eq!(
            focus.current(),
            None,
            "the top layer's saved target (inside the removed layer) is not revived"
        );
        let ring = ring_of(&[A]);
        focus.ensure_valid(&ring);
        assert_eq!(
            focus.current(),
            Some(A),
            "shared fallback picks a valid control"
        );
    }

    #[test]
    fn retain_keeps_return_targets_when_nothing_below_was_removed() {
        let mut layers = Layers::new();
        let mut focus = focused(A);
        layers.open(modal(B, true), LayerTag::new("low"), (), &mut focus);
        layers.open(modal(C, true), LayerTag::new("top"), (), &mut focus);

        layers.retain(|e| e.tag.kind != "top");
        close_by_result(&mut layers, &mut focus);
        assert_eq!(focus.current(), Some(A));
    }

    #[test]
    fn legacy_modal_push_preserves_default_restore() {
        let mut stack: ModalStack<u8> = ModalStack::new();
        let mut focus = focused(A);

        stack.push(1, &mut focus, Some(B));
        assert_eq!(focus.current(), Some(B));
        stack.push(2, &mut focus, None);
        assert_eq!(focus.current(), None);

        assert_eq!(stack.pop(&mut focus), Some(2));
        assert_eq!(focus.current(), Some(B));
        focus.set(Some(C));
        stack.clear(&mut focus);
        assert_eq!(focus.current(), Some(A));
    }

    #[test]
    fn push_with_focus_policy_separates_open_from_close() {
        use junie_tui::ui::modal::FocusOnOpen;

        let mut stack: ModalStack<u8> = ModalStack::new();
        let mut focus = focused(A);

        stack.push_with_focus_policy(1, &mut focus, FocusOnOpen::Preserve, true);
        assert_eq!(focus.current(), Some(A), "Preserve leaves focus alone");
        focus.set(Some(B));
        stack.pop(&mut focus);
        assert_eq!(
            focus.current(),
            Some(A),
            "restore = true returns to the opener"
        );

        stack.push_with_focus_policy(2, &mut focus, FocusOnOpen::Set(Some(C)), false);
        assert_eq!(focus.current(), Some(C));
        focus.set(Some(D));
        stack.pop(&mut focus);
        assert_eq!(
            focus.current(),
            Some(D),
            "restore = false keeps the moved focus"
        );
    }
}

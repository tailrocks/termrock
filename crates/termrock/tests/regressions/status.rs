use crate::common::*;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Position;
use termrock::clock::{Moment, TestRequest};
use termrock::core::event::{External, Input, Mouse, MouseKind, Outcome};
use termrock::core::focus::{Focus, FocusRing};
use termrock::shell::{Cx, Request, StatusLine, background};
use termrock::theme::Tone;

fn mouse(kind: MouseKind) -> Input {
    Input::Mouse(Mouse::new(kind, Position::new(1, 1)))
}

#[test]
fn a_sticky_status_is_neutral_and_does_not_expire() {
    let mut line = StatusLine::<()>::new();
    line.sticky("/Users/me/project");
    assert!(line.is_sticky());
    assert_eq!(line.get(), Some(("/Users/me/project", Tone::Secondary)));
    assert!(!line.expire(Moment::from_millis(60_000)));
    assert_eq!(line.text(), Some("/Users/me/project"));

    line.set("saved", Tone::Secondary);
    assert!(!line.is_sticky());
    assert!(line.expire(Moment::from_millis(65_000)));
    assert_eq!(line.text(), None);
}

#[test]
fn a_sticky_status_clears_on_the_next_key_paste_or_press() {
    let clearing = [
        ("key", Input::Key(key(KeyCode::Char('x')))),
        ("paste", Input::Paste("text".into())),
        ("press", mouse(MouseKind::Down)),
        ("secondary press", mouse(MouseKind::Secondary)),
        ("middle press", mouse(MouseKind::Middle)),
    ];
    for (name, input) in clearing {
        let mut line = StatusLine::<()>::new();
        line.sticky("path");
        assert!(line.on_input(&input), "{name}");
        assert_eq!(line.text(), None, "{name}");
        assert!(!line.is_sticky(), "{name}");
    }
}

#[test]
fn a_sticky_status_stays_through_ticks_resizes_and_pointer_moves() {
    let staying = [
        ("tick", Input::Tick),
        ("resize", Input::Resize(100, 30)),
        ("external", Input::External(External::new(1_u8))),
        ("move", mouse(MouseKind::Move)),
        ("release", mouse(MouseKind::Up)),
    ];
    for (name, input) in staying {
        let mut line = StatusLine::<()>::new();
        line.sticky("path");
        assert!(!line.on_input(&input), "{name}");
        assert_eq!(line.text(), Some("path"), "{name}");
    }
}

#[test]
fn a_sticky_status_posted_by_a_key_survives_that_key() {
    let mut host = Host::new(Act::Sticky("path"));
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Changed);
    assert_eq!(host.status(), Some(("path".to_owned(), Tone::Secondary)));
    host.act = Act::Nothing;
    assert_eq!(host.press(KeyCode::Char('y')), Outcome::Changed);
    assert_eq!(host.status(), None);
}

#[test]
fn a_page_in_the_background_may_keep_a_sticky_status() {
    let mut focus = Focus::default();
    let ring = FocusRing::default();
    let mut cx: Cx<'_, (), (), ()> = Cx::new(&mut focus, &ring);
    cx.sticky("path");
    let requests: Vec<_> = background(cx.take()).collect();
    assert!(matches!(requests.as_slice(), [Request::Sticky(text)] if text == "path"));
}

#[test]
fn a_clock_step_request_does_not_clear_an_error_status_like_operator_input() {
    let mut host = Host::new(Act::Nothing);
    host.test_requests(true);
    host.set_status("save failed", Tone::Error);

    host.input(Input::Paste(TestRequest::advance(250)));
    assert_eq!(
        host.status(),
        Some(("save failed".to_owned(), Tone::Error)),
        "a step of the test clock is not the operator reading the error"
    );

    host.input(Input::Paste("typed".into()));
    assert_eq!(host.status(), None, "a real paste still clears it");
}

#[test]
fn a_clock_step_request_does_not_clear_a_sticky_status() {
    let mut host = Host::new(Act::Nothing);
    host.test_requests(true);
    host.sticky("/Users/me/project");

    host.input(Input::Paste(TestRequest::advance(250)));

    assert_eq!(
        host.status(),
        Some(("/Users/me/project".to_owned(), Tone::Secondary))
    );
}

mod payload {
    use std::time::Duration;

    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use termrock::core::event::{Input, Outcome};
    use termrock::shell::{App, Shell, handle};
    use termrock::theme::{ColorLevel, Theme, Tone};
    use termrock::ui::ctx::RenderCtx;
    use termrock::widgets::too_small::TooSmall;

    pub struct Host {
        shell: Shell<u8>,
        pub next: Option<(&'static str, Tone, u8)>,
    }

    impl Host {
        pub fn new() -> Self {
            Self {
                shell: Shell::new(),
                next: None,
            }
        }

        pub fn show(&mut self, text: &str, tone: Tone, payload: u8) {
            self.shell.status.set_with(text, tone, payload);
        }

        pub fn input(&mut self, input: Input) -> Outcome {
            handle(self, input)
        }

        pub fn payload(&self) -> Option<u8> {
            self.shell.status.payload().copied()
        }
    }

    impl App for Host {
        type Status = u8;

        fn shell(&self) -> &Shell<u8> {
            &self.shell
        }

        fn shell_mut(&mut self) -> &mut Shell<u8> {
            &mut self.shell
        }

        fn theme(&self) -> Theme {
            Theme::for_level(ColorLevel::TrueColor)
        }

        fn too_small(&self) -> TooSmall<'_> {
            TooSmall::new("host")
        }

        fn now_ms(&self) -> u64 {
            0
        }

        fn tick(&self) -> u64 {
            0
        }

        fn tick_interval(&self) -> Duration {
            Duration::from_millis(100)
        }

        fn draw(&mut self, _area: Rect, _buf: &mut Buffer, _ctx: &mut RenderCtx) {}

        fn on_input(&mut self, _input: Input) -> Outcome {
            if let Some((text, tone, payload)) = self.next.take() {
                self.shell.status.set_with(text, tone, payload);
            }
            Outcome::Ignored
        }
    }
}

#[test]
fn an_input_that_changes_only_the_payload_of_the_status_redraws() {
    let mut host = payload::Host::new();
    host.show("Copied", Tone::Secondary, 1);

    host.next = Some(("Copied", Tone::Secondary, 2));
    assert_eq!(host.input(Input::Tick), Outcome::Changed);
    assert_eq!(host.payload(), Some(2));

    assert_eq!(host.input(Input::Tick), Outcome::Ignored);
}

#[test]
fn a_reposted_status_with_the_same_payload_is_not_a_change() {
    let mut host = payload::Host::new();
    host.show("Copied", Tone::Secondary, 1);

    host.next = Some(("Copied", Tone::Secondary, 1));
    assert_eq!(host.input(Input::Tick), Outcome::Ignored);
    assert_eq!(host.payload(), Some(1));
}

#[test]
fn two_status_lines_are_equal_when_they_show_the_same_thing() {
    let fresh = StatusLine::<u8>::new();
    let mut used = StatusLine::<u8>::new();
    used.set_with("x", Tone::Secondary, 1);
    assert!(used.clear());
    assert_ne!(used.revision(), fresh.revision());
    assert!(used == fresh, "the revision counts changes, not state");

    let mut a = StatusLine::<u8>::new();
    let mut b = StatusLine::<u8>::new();
    a.set_with("x", Tone::Secondary, 1);
    b.set_with("x", Tone::Secondary, 2);
    assert!(a != b, "the payload is part of what is shown");
}

#[test]
fn a_notice_shows_when_nothing_is_on_screen() {
    let mut line = StatusLine::<()>::new();
    line.notice_at("index unreadable", Tone::Error, Moment::from_millis(10));
    assert_eq!(line.get(), Some(("index unreadable", Tone::Error)));
}

#[test]
fn a_notice_never_replaces_the_message_on_screen() {
    type Post = fn(&mut StatusLine<()>);
    let shown: [(&str, Post); 4] = [
        ("status", |line| {
            line.set("Discarded 1 change", Tone::Secondary)
        }),
        ("error", |line| line.error("Discarded 1 change")),
        ("sticky", |line| line.sticky("Discarded 1 change")),
        ("progress", |line| {
            line.progress(termrock::shell::JobKey::new("stop"), "Discarded 1 change")
        }),
    ];
    for (name, post) in shown {
        let mut line = StatusLine::<()>::new();
        post(&mut line);
        let revision = line.revision();
        line.notice_at("Refresh failed", Tone::Error, Moment::from_millis(100));
        assert_eq!(line.text(), Some("Discarded 1 change"), "{name}");
        assert_eq!(line.revision(), revision, "{name}: nothing visible changed");
    }
}

#[test]
fn a_notice_replaces_an_earlier_notice() {
    let mut line = StatusLine::<()>::new();
    line.notice("Another client is entering", Tone::Secondary);
    line.notice("Refresh failed", Tone::Error);
    assert_eq!(line.get(), Some(("Refresh failed", Tone::Error)));
}

#[test]
fn a_regular_message_replaces_a_notice() {
    let mut line = StatusLine::<()>::new();
    line.notice("Refresh failed", Tone::Error);
    line.set("Discarded 1 change", Tone::Secondary);
    line.notice("Refresh failed", Tone::Error);
    assert_eq!(line.text(), Some("Discarded 1 change"));
}

#[test]
fn a_dropped_notice_does_not_come_back_when_the_message_expires() {
    let mut line = StatusLine::<()>::new();
    line.set_at("saved", Tone::Secondary, Moment::ZERO);
    line.notice_at("synced", Tone::Secondary, Moment::from_millis(1));
    assert!(line.expire(Moment::from_millis(5_000)));
    assert_eq!(line.text(), None);
    line.notice_at("synced", Tone::Secondary, Moment::from_millis(5_001));
    assert_eq!(line.text(), Some("synced"));
}

#[test]
fn a_notice_request_is_allowed_from_a_screen_that_is_not_shown() {
    let mut focus = Focus::default();
    let ring = FocusRing::default();
    let mut cx: Cx<'_, ()> = Cx::new(&mut focus, &ring);
    cx.notice("Refresh failed", Tone::Error);
    let kept: Vec<_> = background(cx.take()).collect();
    assert!(
        matches!(kept.as_slice(), [Request::Notice(text, Tone::Error)] if text == "Refresh failed")
    );
}

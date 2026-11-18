use termrock_core::event::Outcome;
use termrock_core::focus::{Focus, FocusRing};
use termrock_core::hit::HitRegistry;
use termrock_core::id::WidgetId;
use termrock_core::pointer::{
    PointerConfig, PointerEvent, PointerHost, PointerParts, PointerRouter,
};
use termrock_core::response::Response;

const BUTTON: WidgetId = WidgetId::of("t.flash.button");

struct Host {
    router: PointerRouter,
    hits: HitRegistry,
    focus: Focus,
    ring: FocusRing,
    now: u64,
}

impl Host {
    fn new() -> Self {
        Self {
            router: PointerRouter::new(PointerConfig::DEFAULT),
            hits: HitRegistry::default(),
            focus: Focus::default(),
            ring: FocusRing::default(),
            now: 0,
        }
    }
}

impl PointerHost for Host {
    fn pointer_parts(&mut self) -> PointerParts<'_> {
        PointerParts {
            router: &mut self.router,
            hits: &self.hits,
            focus: &mut self.focus,
            ring: &self.ring,
        }
    }

    fn layer_open(&self) -> bool {
        false
    }

    fn now_ms(&self) -> u64 {
        self.now
    }

    fn deliver(&mut self, _ev: &PointerEvent) -> Response<()> {
        Response::ignored()
    }
}

#[test]
fn pointer_tick_repaints_once_the_press_flash_has_expired() {
    let mut host = Host::new();
    host.router.flash(BUTTON, host.now);
    assert!(host.router.has_flash(), "the press starts a flash");

    host.now = PointerConfig::DEFAULT.flash_ms - 1;
    assert_eq!(
        PointerRouter::tick(&mut host),
        Outcome::Ignored,
        "the flash is still inside its window"
    );
    assert!(host.router.has_flash());

    host.now = PointerConfig::DEFAULT.flash_ms;
    assert_eq!(
        PointerRouter::tick(&mut host),
        Outcome::Changed,
        "a tick past the deadline repaints the frame without the flash"
    );
    assert!(!host.router.has_flash(), "the expired flash is dropped");
    assert_eq!(
        PointerRouter::tick(&mut host),
        Outcome::Ignored,
        "an expired flash does not repaint again"
    );
}

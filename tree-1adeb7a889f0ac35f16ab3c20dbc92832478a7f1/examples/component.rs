mod support;
use termrock::runtime::Component;
struct Screen;
impl Component<(), ()> for Screen {
    fn handle_event(&mut self, (): &()) -> Option<()> {
        Some(())
    }
}
fn main() {
    let _screen = Screen;
    support::render();
}

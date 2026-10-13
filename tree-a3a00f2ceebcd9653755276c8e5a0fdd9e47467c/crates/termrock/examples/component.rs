mod support;
use termrock::runtime::{Component, UpdateResult};
struct Screen;
impl Component<(), ()> for Screen {
    type Effect = ();
    fn event(&mut self, (): ()) -> Option<()> {
        Some(())
    }
    fn update(&mut self, (): ()) -> UpdateResult<()> {
        UpdateResult::redraw()
    }
}
fn main() {
    let _screen = Screen;
    support::render();
}

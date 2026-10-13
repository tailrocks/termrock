mod support;
fn main() -> std::io::Result<()> {
    let mut session = termrock::crossterm::Session::enter(
        std::io::stdout(),
        termrock::crossterm::SessionOptions {
            alternate_screen: false,
            mouse_capture: false,
            bracketed_paste: false,
            raw_mode: false,
        },
    )?;
    support::render();
    session.restore()
}

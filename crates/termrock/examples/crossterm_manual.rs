mod support;
fn main() {
    let _backend = termrock::crossterm::CrosstermBackend::new(std::io::stdout());
    support::render();
}

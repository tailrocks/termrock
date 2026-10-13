mod support;
#[derive(Default)]
struct Store {
    selected: usize,
}
fn main() {
    let store = Store::default();
    assert_eq!(store.selected, 0);
    support::render();
}

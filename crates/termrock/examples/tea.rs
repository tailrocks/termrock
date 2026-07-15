mod support;
enum Message {
    Select,
}
fn main() {
    let _message = Message::Select;
    support::render();
}

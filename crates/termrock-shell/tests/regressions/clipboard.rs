use std::sync::Arc;

use junie_tui::clock::Moment;
use junie_tui::core::edit::FieldKeys;
use junie_tui::core::event::Key;
use junie_tui::core::id::WidgetId;
use junie_tui::shell::{Clipboard, Copied, NativeClipboard, Shell};
use junie_tui::widgets::input::TextInput;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

#[derive(Debug)]
struct Sink(bool);

impl NativeClipboard for Sink {
    fn write(&self, _text: &str) -> bool {
        self.0
    }
}

const TEXT: &str = "the settlement retry loop so failed batches";

#[test]
fn a_native_write_overrides_an_osc52_refusal() {
    let chars = TEXT.chars().count();
    let too_large = || Clipboard::new().limit(8);
    let unsupported = || Clipboard::new().osc52(false);

    assert_eq!(
        too_large().native(Arc::new(Sink(true))).copy(TEXT),
        Copied::Copied { chars }
    );
    assert_eq!(
        too_large().native(Arc::new(Sink(false))).copy(TEXT),
        Copied::TooLarge {
            bytes: TEXT.len(),
            limit: 8
        }
    );
    assert_eq!(
        unsupported().native(Arc::new(Sink(true))).copy(TEXT),
        Copied::Copied { chars }
    );
    assert_eq!(
        unsupported().native(Arc::new(Sink(false))).copy(TEXT),
        Copied::Unsupported
    );
}

#[derive(Debug, Default)]
struct Counting(std::sync::atomic::AtomicUsize);

impl NativeClipboard for Counting {
    fn write(&self, _text: &str) -> bool {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        true
    }
}

fn queued() -> Vec<String> {
    junie_tui::shell::clipboard::take_output()
}

#[test]
fn the_limit_counts_decoded_bytes_like_the_pane_policy() {
    assert_eq!(Clipboard::DEFAULT_LIMIT, 100_000);
    let at_limit = "x".repeat(100_000);
    let over = "x".repeat(100_001);
    let clipboard = Clipboard::new();

    assert_eq!(
        clipboard.copy(&"x".repeat(80_000)),
        Copied::Copied { chars: 80_000 },
        "80,000 decoded bytes are 106,668 encoded; the pane policy accepts them"
    );
    assert_eq!(clipboard.copy(&at_limit), Copied::Copied { chars: 100_000 });
    let refused = clipboard.copy(&over);
    assert!(
        matches!(refused, Copied::TooLarge { limit: 100_000, .. }),
        "{refused:?}"
    );
    assert_eq!(
        Clipboard::report(&refused, &over, None).0,
        "Not copied · 100001 bytes exceed the terminal's 100000 byte OSC 52 limit"
    );
    queued();
}

#[test]
fn osc52_is_queued_only_when_the_native_write_did_not_land() {
    queued();
    let native = Arc::new(Counting::default());
    let landed = Clipboard::new().native(native.clone()).copy(TEXT);
    assert_eq!(
        landed,
        Copied::Copied {
            chars: TEXT.chars().count()
        }
    );
    assert_eq!(native.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(
        queued(),
        Vec::<String>::new(),
        "no OSC 52 after a native write"
    );

    let missed = Clipboard::new().native(Arc::new(Sink(false))).copy(TEXT);
    assert_eq!(
        missed,
        Copied::Requested {
            chars: TEXT.chars().count()
        }
    );
    assert_eq!(
        queued().len(),
        1,
        "a failed native write falls back to OSC 52"
    );

    Clipboard::new().copy(TEXT);
    assert_eq!(queued().len(), 1, "no native sink: OSC 52 only");
}

fn cmd(c: char) -> Key {
    Key {
        code: KeyCode::Char(c),
        mods: KeyModifiers::SUPER,
    }
}

fn copy_selection(sink: Arc<dyn NativeClipboard>) {
    let mut shell = Shell::<()>::new();
    shell.set_clipboard(Clipboard::new().native(sink));
    let mut field = TextInput::new(WidgetId::of("t.field"), "Name").value("alpha");
    field.on_key(&cmd('a'));
    field.on_key(&cmd('c'));
}

#[test]
fn a_field_copy_reaches_the_native_clipboard_once_and_queues_no_osc52() {
    queued();
    let native = Arc::new(Counting::default());
    copy_selection(native.clone());
    assert_eq!(native.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(queued(), Vec::<String>::new());
}

#[test]
fn a_field_copy_falls_back_to_one_osc52_when_the_native_write_fails() {
    queued();
    copy_selection(Arc::new(Sink(false)));
    assert_eq!(queued().len(), 1);
}

#[test]
fn a_shell_copy_with_the_hook_installed_queues_no_osc52_after_a_native_write() {
    queued();
    let native = Arc::new(Counting::default());
    let mut shell = Shell::<()>::new();
    shell.set_clipboard(Clipboard::new().native(native.clone()));
    shell.copy_at(TEXT, None, Moment::ZERO);
    assert_eq!(native.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(queued(), Vec::<String>::new());
}

#[derive(Debug, Default)]
struct Recording(std::sync::Mutex<Vec<String>>);

impl NativeClipboard for Recording {
    fn write(&self, text: &str) -> bool {
        self.0.lock().expect("writes").push(text.to_owned());
        true
    }
}

fn cmd_on(field: &mut TextInput, keys: &[char]) -> Vec<String> {
    let sink = Arc::new(Recording::default());
    let mut shell = Shell::<()>::new();
    shell.set_clipboard(Clipboard::new().native(sink.clone()));
    queued();
    for c in keys {
        field.on_key(&cmd(*c));
    }
    let writes = sink.0.lock().expect("writes").clone();
    assert_eq!(queued(), Vec::<String>::new(), "no OSC 52");
    writes
}

fn masked_secret_after(second: char) {
    let mut field = TextInput::new(WidgetId::of("t.secret"), "Key")
        .value("sk-secret")
        .masked();
    let writes = cmd_on(&mut field, &['a', second]);
    assert_eq!(writes, Vec::<String>::new(), "Cmd+A, Cmd+{second}");
    assert_eq!(field.text(), "sk-secret", "Cmd+{second} must not cut");
}

#[test]
fn a_masked_field_copies_nothing_to_the_clipboard() {
    masked_secret_after('c');
}

#[test]
fn a_masked_field_cuts_nothing_to_the_clipboard() {
    masked_secret_after('x');
}

#[test]
fn an_unmasked_field_still_copies_and_cuts() {
    let mut copied = TextInput::new(WidgetId::of("t.plain"), "Name").value("alpha");
    assert_eq!(cmd_on(&mut copied, &['a', 'c']), ["alpha"]);
    let mut cut = TextInput::new(WidgetId::of("t.plain"), "Name").value("alpha");
    assert_eq!(cmd_on(&mut cut, &['a', 'x']), ["alpha"]);
    assert_eq!(cut.text(), "");
}

fn ctrl_w() -> Key {
    Key {
        code: KeyCode::Char('w'),
        mods: KeyModifiers::CONTROL,
    }
}

#[test]
fn ctrl_w_in_a_masked_field_deletes_the_word_before_the_cursor_and_copies_nothing() {
    let mut field = TextInput::new(WidgetId::of("t.secret"), "Key").masked();
    for c in "sk-one two".chars() {
        field.on_key(&Key {
            code: KeyCode::Char(c),
            mods: KeyModifiers::NONE,
        });
    }
    let sink = Arc::new(Recording::default());
    let mut shell = Shell::<()>::new();
    shell.set_clipboard(Clipboard::new().native(sink.clone()));
    queued();
    field.on_key(&ctrl_w());
    assert_eq!(field.text(), "sk-one ");
    field.on_key(&ctrl_w());
    assert_eq!(field.text(), "sk-");
    assert_eq!(sink.0.lock().expect("writes").clone(), Vec::<String>::new());
    assert_eq!(queued(), Vec::<String>::new(), "no OSC 52");
}

#[test]
fn ctrl_w_over_a_selection_in_a_masked_field_deletes_it_and_copies_nothing() {
    let mut field = TextInput::new(WidgetId::of("t.secret"), "Key")
        .value("sk-secret")
        .masked();
    let sink = Arc::new(Recording::default());
    let mut shell = Shell::<()>::new();
    shell.set_clipboard(Clipboard::new().native(sink.clone()));
    queued();
    field.on_key(&cmd('a'));
    field.on_key(&ctrl_w());
    assert_eq!(field.text(), "");
    assert_eq!(sink.0.lock().expect("writes").clone(), Vec::<String>::new());
    assert_eq!(queued(), Vec::<String>::new(), "no OSC 52");
}

fn ctrl(c: char) -> Key {
    Key {
        code: KeyCode::Char(c),
        mods: KeyModifiers::CONTROL,
    }
}

#[test]
fn ctrl_x_in_a_masked_desktop_field_neither_copies_nor_cuts() {
    let mut field = TextInput::new(WidgetId::of("t.secret"), "Key")
        .value("sk-secret")
        .masked()
        .field_keys(FieldKeys::Desktop);
    let sink = Arc::new(Recording::default());
    let mut shell = Shell::<()>::new();
    shell.set_clipboard(Clipboard::new().native(sink.clone()));
    queued();
    field.on_key(&ctrl('a'));
    field.on_key(&ctrl('x'));
    assert_eq!(field.text(), "sk-secret");
    assert_eq!(sink.0.lock().expect("writes").clone(), Vec::<String>::new());
    assert_eq!(queued(), Vec::<String>::new(), "no OSC 52");
}

#[test]
fn ctrl_x_in_an_unmasked_desktop_field_cuts_to_the_clipboard() {
    let mut field = TextInput::new(WidgetId::of("t.plain"), "Name")
        .value("alpha")
        .field_keys(FieldKeys::Desktop);
    let sink = Arc::new(Recording::default());
    let mut shell = Shell::<()>::new();
    shell.set_clipboard(Clipboard::new().native(sink.clone()));
    queued();
    field.on_key(&ctrl('a'));
    field.on_key(&ctrl('x'));
    assert_eq!(field.text(), "");
    assert_eq!(sink.0.lock().expect("writes").clone(), ["alpha"]);
}

#[test]
fn a_dropped_shell_leaves_no_clipboard_hook_behind() {
    queued();
    let native = Arc::new(Counting::default());
    let mut shell = Shell::<()>::new();
    shell.set_clipboard(Clipboard::new().native(native.clone()));
    junie_tui::core::edit::clipboard::copy(TEXT);
    assert_eq!(native.0.load(std::sync::atomic::Ordering::SeqCst), 1);

    drop(shell);
    junie_tui::core::edit::clipboard::copy(TEXT);

    assert_eq!(
        native.0.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the next owner of this thread does not write through the old app's clipboard"
    );
    assert_eq!(queued(), Vec::<String>::new());
}

#[test]
fn dropping_one_shell_keeps_the_clipboard_hook_of_the_shell_that_replaced_it() {
    queued();
    let first_sink = Arc::new(Counting::default());
    let second_sink = Arc::new(Counting::default());
    let mut first = Shell::<()>::new();
    first.set_clipboard(Clipboard::new().native(first_sink.clone()));
    let mut second = Shell::<()>::new();
    second.set_clipboard(Clipboard::new().native(second_sink.clone()));

    drop(first);
    junie_tui::core::edit::clipboard::copy(TEXT);

    let count = |sink: &Counting| sink.0.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(count(&second_sink), 1, "the live shell still owns the hook");
    assert_eq!(count(&first_sink), 0);

    drop(second);
    junie_tui::core::edit::clipboard::copy(TEXT);
    assert_eq!(count(&second_sink), 1, "its own drop still removes it");
}

#[test]
fn dropping_a_shell_keeps_the_hook_of_another_shell_given_a_clone_of_the_same_clipboard() {
    queued();
    let sink = Arc::new(Counting::default());
    let configured = Clipboard::new().native(sink.clone());
    let mut first = Shell::<()>::new();
    first.set_clipboard(configured.clone());
    let mut second = Shell::<()>::new();
    second.set_clipboard(configured.clone());

    drop(first);
    junie_tui::core::edit::clipboard::copy(TEXT);

    let count = || sink.0.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(count(), 1, "the live shell still owns the hook");

    drop(second);
    junie_tui::core::edit::clipboard::copy(TEXT);
    assert_eq!(count(), 1, "its own drop still removes it");
}

#[test]
fn installing_the_same_clipboard_twice_leaves_one_live_hook() {
    queued();
    let sink = Arc::new(Counting::default());
    let configured = Clipboard::new().native(sink.clone());
    let mut shell = Shell::<()>::new();
    shell.set_clipboard(configured.clone());
    shell.set_clipboard(configured.clone());

    junie_tui::core::edit::clipboard::copy(TEXT);

    assert_eq!(sink.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    drop(shell);
    junie_tui::core::edit::clipboard::copy(TEXT);
    assert_eq!(sink.0.load(std::sync::atomic::Ordering::SeqCst), 1);
}

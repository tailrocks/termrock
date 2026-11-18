use super::event::ClientPlatform;
use super::keymap::{Actions, KeyBinding, KeyMap};
use termrock_text::buffer::TextBuffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditEntry {
    #[default]
    Immediate,

    TypeToEdit,

    Explicit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusSelect {
    #[default]
    SelectAll,

    CaretEnd,

    CaretStart,
}

impl FocusSelect {
    pub fn apply(self, buf: &mut TextBuffer) {
        match self {
            FocusSelect::SelectAll => buf.select_all(),
            FocusSelect::CaretEnd => {
                buf.move_doc_end(false);
                buf.clear_selection();
            }
            FocusSelect::CaretStart => {
                buf.move_doc_start(false);
                buf.clear_selection();
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKeys {
    Emacs,

    Desktop,
}

impl FieldKeys {
    pub const fn for_platform(platform: ClientPlatform) -> Self {
        match platform {
            ClientPlatform::MacOs => Self::Emacs,
            ClientPlatform::Other => Self::Desktop,
        }
    }

    pub fn text_keymap(self) -> KeyMap<TextCmd> {
        let mut map: KeyMap<TextCmd> = CMD_TEXT_KEYS.to_vec();
        match self {
            Self::Emacs => map.extend([
                (KeyBinding::ctrl('l'), TextCmd::SelectAll),
                (KeyBinding::alt('w'), TextCmd::Copy),
                (KeyBinding::ctrl('w'), TextCmd::Cut),
                (KeyBinding::ctrl('y'), TextCmd::Paste),
            ]),
            Self::Desktop => map.extend([
                (KeyBinding::ctrl('a'), TextCmd::SelectAll),
                (KeyBinding::ctrl('c'), TextCmd::Copy),
                (KeyBinding::ctrl('x'), TextCmd::Cut),
                (KeyBinding::ctrl('w'), TextCmd::Cut),
                (KeyBinding::ctrl('v'), TextCmd::Paste),
                (KeyBinding::ctrl('y'), TextCmd::Redo),
            ]),
        }
        map.extend([
            (KeyBinding::ctrl('z').shift(), TextCmd::Redo),
            (KeyBinding::ctrl('Z'), TextCmd::Redo),
            (KeyBinding::ctrl('z'), TextCmd::Undo),
        ]);
        map
    }
}

pub const CMD_TEXT_KEYS: [(KeyBinding, TextCmd); 15] = {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};
    const CMD: KeyModifiers = KeyModifiers::SUPER;
    const CMD_SHIFT: KeyModifiers = KeyModifiers::SUPER.union(KeyModifiers::SHIFT);
    [
        (KeyBinding::cmd('a'), TextCmd::SelectAll),
        (KeyBinding::cmd('c'), TextCmd::Copy),
        (KeyBinding::cmd('x'), TextCmd::Cut),
        (KeyBinding::cmd('v'), TextCmd::Paste),
        (KeyBinding::cmd('z').shift(), TextCmd::Redo),
        (KeyBinding::cmd('Z'), TextCmd::Redo),
        (KeyBinding::cmd('z'), TextCmd::Undo),
        (KeyBinding::new(KeyCode::Left, CMD), TextCmd::LineStart),
        (KeyBinding::new(KeyCode::Right, CMD), TextCmd::LineEnd),
        (KeyBinding::new(KeyCode::Up, CMD), TextCmd::DocStart),
        (KeyBinding::new(KeyCode::Down, CMD), TextCmd::DocEnd),
        (
            KeyBinding::new(KeyCode::Left, CMD_SHIFT),
            TextCmd::SelectToLineStart,
        ),
        (
            KeyBinding::new(KeyCode::Right, CMD_SHIFT),
            TextCmd::SelectToLineEnd,
        ),
        (
            KeyBinding::new(KeyCode::Up, CMD_SHIFT),
            TextCmd::SelectToDocStart,
        ),
        (
            KeyBinding::new(KeyCode::Down, CMD_SHIFT),
            TextCmd::SelectToDocEnd,
        ),
    ]
};

impl Default for FieldKeys {
    fn default() -> Self {
        Self::for_platform(ClientPlatform::current())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextCmd {
    SelectAll,

    Copy,

    Cut,

    Paste,

    Undo,

    Redo,

    LineStart,

    LineEnd,

    DocStart,

    DocEnd,

    SelectToLineStart,

    SelectToLineEnd,

    SelectToDocStart,

    SelectToDocEnd,
}

impl Actions for TextCmd {
    fn label(self) -> &'static str {
        match self {
            TextCmd::SelectAll => "Select all",
            TextCmd::Copy => "Copy",
            TextCmd::Cut => "Cut",
            TextCmd::Paste => "Paste",
            TextCmd::Undo => "Undo",
            TextCmd::Redo => "Redo",
            TextCmd::LineStart => "Line start",
            TextCmd::LineEnd => "Line end",
            TextCmd::DocStart => "Start of text",
            TextCmd::DocEnd => "End of text",
            TextCmd::SelectToLineStart => "Select to line start",
            TextCmd::SelectToLineEnd => "Select to line end",
            TextCmd::SelectToDocStart => "Select to start",
            TextCmd::SelectToDocEnd => "Select to end",
        }
    }

    fn default_keymap() -> KeyMap<Self> {
        FieldKeys::default().text_keymap()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Snap {
    text: String,
    cursor: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditKind {
    #[default]
    Typing,

    Deleting,

    Other,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UndoStack {
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    run: Option<EditKind>,
}

impl UndoStack {
    pub const LIMIT: usize = 200;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, buf: &TextBuffer, kind: EditKind) {
        self.redo.clear();
        let folds = kind != EditKind::Other && self.run == Some(kind);
        if !folds {
            termrock_metrics::record_work!(PayloadBytesCloned, buf.text().len());
            self.undo.push(Snap {
                text: buf.text().to_owned(),
                cursor: buf.cursor_offset(),
            });
            if self.undo.len() > Self::LIMIT {
                self.undo.remove(0);
            }
        }
        self.run = (kind != EditKind::Other).then_some(kind);
    }

    pub fn seal(&mut self) {
        self.run = None;
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.run = None;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, buf: &mut TextBuffer) -> bool {
        let Some(s) = self.undo.pop() else {
            return false;
        };
        termrock_metrics::record_work!(PayloadBytesCloned, buf.text().len());
        self.redo.push(Snap {
            text: buf.text().to_owned(),
            cursor: buf.cursor_offset(),
        });
        restore(buf, s);
        self.run = None;
        true
    }

    pub fn redo(&mut self, buf: &mut TextBuffer) -> bool {
        let Some(s) = self.redo.pop() else {
            return false;
        };
        termrock_metrics::record_work!(PayloadBytesCloned, buf.text().len());
        self.undo.push(Snap {
            text: buf.text().to_owned(),
            cursor: buf.cursor_offset(),
        });
        restore(buf, s);
        self.run = None;
        true
    }
}

fn restore(buf: &mut TextBuffer, s: Snap) {
    buf.set_text(s.text);
    let at = s.cursor.min(buf.text().len());
    buf.select_range(at, at);
    buf.clear_selection();
}

pub mod clipboard {
    use std::cell::RefCell;

    type Hook = (u64, Box<dyn Fn(&str)>);

    thread_local! {
        static TEXT: RefCell<Option<String>> = const { RefCell::new(None) };
        static HOOK: RefCell<Option<Hook>> = const { RefCell::new(None) };
    }

    #[doc(hidden)]
    pub fn store(text: &str) {
        TEXT.with(|t| *t.borrow_mut() = Some(text.to_owned()));
    }

    pub fn copy(text: &str) {
        store(text);
        HOOK.with(|h| {
            if let Some((_, f)) = h.borrow().as_ref() {
                f(text);
            }
        });
    }

    pub fn text() -> Option<String> {
        TEXT.with(|t| t.borrow().clone())
    }

    pub fn set_hook(owner: u64, hook: impl Fn(&str) + 'static) {
        HOOK.with(|h| *h.borrow_mut() = Some((owner, Box::new(hook))));
    }

    pub fn clear_hook(owner: u64) {
        HOOK.with(|h| {
            let mut slot = h.borrow_mut();
            if slot.as_ref().is_some_and(|(o, _)| *o == owner) {
                *slot = None;
            }
        });
    }
}

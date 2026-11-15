use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::core::event::{Input, Key, Outcome};
use crate::core::keymap::{KeyBinding, KeyMap, lookup};
use crate::core::list::route_key;
use crate::ui::leave::{KeyContext, LeaveKey, QuitKeys};

use super::App;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShellAction<C = ()> {
    Quit,

    Help,

    MenuBar,

    NextPage,

    PrevPage,

    FocusNext,

    FocusPrev,

    Back,

    App(C),
}

#[derive(Debug, Clone)]
pub struct ShellKeys<C = ()> {
    pub quit: QuitKeys,

    pub priority: KeyMap<ShellAction<C>>,

    pub keys: KeyMap<ShellAction<C>>,

    pub fallback: KeyMap<ShellAction<C>>,

    pub idle_only: Vec<ShellAction<C>>,

    pub esc_note: Option<String>,

    pub dirty_letter_note: Option<String>,
}

impl<C> Default for ShellKeys<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> ShellKeys<C> {
    pub fn new() -> Self {
        Self {
            quit: QuitKeys::DEFAULT,
            priority: vec![],
            keys: vec![
                (KeyBinding::key(KeyCode::F(1)), ShellAction::Help),
                (KeyBinding::key(KeyCode::F(10)), ShellAction::MenuBar),
                (
                    KeyBinding::new(KeyCode::F(2), KeyModifiers::CONTROL),
                    ShellAction::MenuBar,
                ),
                (
                    KeyBinding::new(KeyCode::PageDown, KeyModifiers::CONTROL),
                    ShellAction::NextPage,
                ),
                (
                    KeyBinding::new(KeyCode::PageUp, KeyModifiers::CONTROL),
                    ShellAction::PrevPage,
                ),
            ],
            fallback: vec![
                (KeyBinding::char('?'), ShellAction::Help),
                (KeyBinding::key(KeyCode::Tab), ShellAction::FocusNext),
                (
                    KeyBinding::new(KeyCode::BackTab, KeyModifiers::SHIFT),
                    ShellAction::FocusPrev,
                ),
                (KeyBinding::key(KeyCode::BackTab), ShellAction::FocusPrev),
                (KeyBinding::key(KeyCode::Esc), ShellAction::Back),
            ],
            idle_only: vec![],
            esc_note: Some("Esc never quits · Ctrl+Q quits".into()),
            dirty_letter_note: Some("Unsaved changes · Ctrl+Q asks before quitting".into()),
        }
    }

    pub fn quit(mut self, quit: QuitKeys) -> Self {
        self.quit = quit;
        self
    }

    pub fn bind(mut self, key: impl Into<KeyBinding>, action: ShellAction<C>) -> Self {
        self.keys.insert(0, (key.into(), action));
        self
    }

    pub fn bind_priority(mut self, key: impl Into<KeyBinding>, action: ShellAction<C>) -> Self {
        self.priority.insert(0, (key.into(), action));
        self
    }

    pub fn bind_fallback(mut self, key: impl Into<KeyBinding>, action: ShellAction<C>) -> Self {
        self.fallback.insert(0, (key.into(), action));
        self
    }

    pub fn unbind(mut self, key: impl Into<KeyBinding>) -> Self {
        let key = key.into();
        self.priority.retain(|(b, _)| *b != key);
        self.keys.retain(|(b, _)| *b != key);
        self.fallback.retain(|(b, _)| *b != key);
        self
    }

    pub fn idle_only(mut self, action: ShellAction<C>) -> Self {
        self.idle_only.push(action);
        self
    }

    pub fn esc_note(mut self, note: Option<&str>) -> Self {
        self.esc_note = note.map(Into::into);
        self
    }

    pub fn help_section(
        &self,
        title: &str,
        app: impl Fn(C) -> Option<&'static str>,
    ) -> crate::widgets::help::HelpSection
    where
        C: Copy,
    {
        use crate::core::hints::Binding;
        let label = |a: ShellAction<C>| -> Option<&'static str> {
            Some(match a {
                ShellAction::Quit => "Quit",
                ShellAction::Help => "This reference",
                ShellAction::MenuBar => "Menu bar",
                ShellAction::NextPage => "Next page",
                ShellAction::PrevPage => "Previous page",
                ShellAction::FocusNext => "Next control",
                ShellAction::FocusPrev => "Previous control",
                ShellAction::Back => "Back",
                ShellAction::App(c) => return app(c),
            })
        };
        let mut rows: Vec<Binding> = self
            .priority
            .iter()
            .chain(&self.keys)
            .chain(&self.fallback)
            .filter(|(k, _)| k.deliverable())
            .filter_map(|&(k, a)| label(a).map(|l| Binding::key(k, l)))
            .collect();
        let mut quit = vec![];
        if self.quit.ctrl_q {
            quit.push(Binding::key(KeyBinding::ctrl('q'), "Quit"));
        }
        if let Some(c) = self.quit.letter {
            quit.push(Binding::key(KeyBinding::char(c), "Quit"));
        }
        if self.quit.esc_quits {
            quit.push(Binding::key(KeyBinding::key(KeyCode::Esc), "Quit"));
        }
        if self.quit.ctrl_c {
            quit.push(Binding::key(KeyBinding::ctrl('c'), "Copy, else quit"));
        }
        rows.extend(quit);
        crate::widgets::help::HelpSection::new(title).bindings(&rows)
    }

    pub fn dirty_letter_note(mut self, note: Option<&str>) -> Self {
        self.dirty_letter_note = note.map(Into::into);
        self
    }
}

pub trait KeyHost: App {
    type Cmd: Copy + PartialEq + 'static;

    fn shell_keys(&self) -> &ShellKeys<Self::Cmd>;

    fn layer_key(&mut self, _key: &Key) -> Option<Outcome> {
        None
    }

    fn menu_key(&mut self, _key: &Key) -> Option<Outcome> {
        None
    }

    fn capturing(&self) -> bool {
        false
    }

    fn text_focus(&self) -> bool {
        self.capturing()
    }

    fn interrupt(&mut self) -> Outcome {
        Outcome::Ignored
    }

    fn dirty(&mut self) -> bool {
        false
    }

    fn selection(&self) -> bool {
        let s = self.shell();
        s.focus.current().is_some_and(|f| s.ring.has_selection(f))
    }

    fn shell_action(&mut self, _action: ShellAction<Self::Cmd>, _key: &Key) -> Outcome {
        Outcome::Ignored
    }

    fn chrome_key(&mut self, _key: &Key) -> Outcome {
        Outcome::Ignored
    }

    fn page_key(&mut self, key: &Key) -> Outcome;

    fn esc_top(&mut self) -> Outcome {
        Outcome::Ignored
    }

    fn layer_paste(&mut self, _text: &str) -> Option<Outcome> {
        None
    }

    fn page_paste(&mut self, _text: &str) -> Outcome {
        Outcome::Ignored
    }

    fn request_quit(&mut self) -> Outcome;

    fn note(&mut self, _text: &str) {}
}

pub struct KeyRouter;

impl KeyRouter {
    pub fn input<H: KeyHost + ?Sized>(host: &mut H, input: Input) -> Outcome {
        match input {
            Input::Key(key) => Self::route(host, &key),
            Input::Paste(text) => Self::paste(host, &text),
            _ => Outcome::Ignored,
        }
    }

    pub fn paste<H: KeyHost + ?Sized>(host: &mut H, text: &str) -> Outcome {
        if host.shell().too_small {
            return Outcome::Consumed;
        }
        if let Some(out) = host.layer_paste(text) {
            return out.or(Outcome::Consumed);
        }
        host.page_paste(text)
    }

    pub fn route<H: KeyHost + ?Sized>(host: &mut H, key: &Key) -> Outcome {
        let interaction_changed = host.shell_mut().pointer.suppress_hover();
        let command = Self::replay(host, key);
        command.or(if interaction_changed {
            Outcome::Changed
        } else {
            Outcome::Ignored
        })
    }

    pub fn replay<H: KeyHost + ?Sized>(host: &mut H, key: &Key) -> Outcome {
        route_key(key, || Self::once(host, key))
    }

    fn once<H: KeyHost + ?Sized>(host: &mut H, key: &Key) -> Outcome {
        let quit = host.shell_keys().quit;
        if host.shell().too_small {
            let letter = quit.letter.is_some_and(|c| key.is_char(c));
            if letter || quit.classify(key, KeyContext::default()) == Some(LeaveKey::Quit) {
                return host.request_quit().or(Outcome::Consumed);
            }
            return Outcome::Consumed;
        }
        if let Some(out) = host.layer_key(key) {
            if out.consumed() {
                return out;
            }
            if key.chord() && quit.classify(key, KeyContext::default()) == Some(LeaveKey::Quit) {
                return host.request_quit();
            }
            return Outcome::Consumed;
        }
        if let Some(out) = host.menu_key(key) {
            return out;
        }
        let capture = host.capturing();
        let text = capture || host.text_focus();
        if let Some(action) = Self::bound(host.shell_keys(), &host.shell_keys().priority, key, text)
        {
            let out = Self::act(host, action, key);
            if out.consumed() {
                return out;
            }
        }
        if capture {
            let out = host.page_key(key);
            if out.consumed() {
                return out;
            }
        }

        let copy = quit.ctrl_c && key.ctrl_char('c') && host.selection();
        if quit.ctrl_c && key.ctrl_char('c') && !copy {
            let out = host.interrupt();
            if out.consumed() {
                return out;
            }
        }
        if key.chord() {
            let kc = KeyContext {
                text_focus: text,
                dirty: false,
                selection: copy,
            };
            if quit.classify(key, kc) == Some(LeaveKey::Quit) {
                return host.request_quit();
            }
        }
        if let Some(action) = Self::bound(host.shell_keys(), &host.shell_keys().keys, key, text) {
            let out = Self::act(host, action, key);
            if out.consumed() {
                return out;
            }
        }
        if !capture {
            let out = host.chrome_key(key);
            if out.consumed() {
                return out;
            }
            let activating = matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) && key.plain();
            let out = host.page_key(key);
            if out.consumed() {
                if activating
                    && !text
                    && out == Outcome::Changed
                    && let Some(f) = host.shell().focus.current()
                {
                    let now = host.now_ms();
                    host.shell_mut().pointer.flash(f, now);
                }
                return out;
            }
            if let Some(c) = quit.letter
                && key.is_char(c)
                && !text
            {
                let dirty = host.dirty();
                let kc = KeyContext {
                    text_focus: false,
                    dirty,
                    selection: false,
                };
                if quit.classify(key, kc) == Some(LeaveKey::Quit) {
                    return host.request_quit();
                }
                if dirty && let Some(note) = host.shell_keys().dirty_letter_note.clone() {
                    host.note(&note);
                    return Outcome::Changed;
                }
            }
        }
        if let Some(action) = Self::bound(host.shell_keys(), &host.shell_keys().fallback, key, text)
        {
            return Self::act(host, action, key);
        }
        Outcome::Ignored
    }

    fn bound<C: Copy + PartialEq>(
        keys: &ShellKeys<C>,
        map: &[(KeyBinding, ShellAction<C>)],
        key: &Key,
        capture: bool,
    ) -> Option<ShellAction<C>> {
        let action = lookup(map, key)?;
        let printable = matches!(key.code, KeyCode::Char(_)) && !key.chord();
        let quiet = capture && (printable || keys.idle_only.contains(&action));
        (!quiet).then_some(action)
    }

    fn act<H: KeyHost + ?Sized>(host: &mut H, action: ShellAction<H::Cmd>, key: &Key) -> Outcome {
        match action {
            ShellAction::FocusNext => {
                host.shell_mut().focus_next();
                Outcome::Changed
            }
            ShellAction::FocusPrev => {
                host.shell_mut().focus_prev();
                Outcome::Changed
            }
            ShellAction::Quit => host.request_quit(),
            ShellAction::Back => {
                let out = host.esc_top();
                if out.consumed() {
                    return out;
                }
                let quit = host.shell_keys().quit;
                if quit.classify(key, KeyContext::default()) == Some(LeaveKey::Quit) {
                    return host.request_quit();
                }
                match host.shell_keys().esc_note.clone() {
                    Some(note) => {
                        host.note(&note);
                        Outcome::Changed
                    }
                    None => Outcome::Consumed,
                }
            }
            other => host.shell_action(other, key),
        }
    }
}

use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Position;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Outcome {
    #[default]
    Ignored,

    Consumed,

    Changed,
}

impl Outcome {
    pub fn consumed(self) -> bool {
        !matches!(self, Outcome::Ignored)
    }

    pub fn or(self, other: Outcome) -> Outcome {
        match (self, other) {
            (Outcome::Changed, _) | (_, Outcome::Changed) => Outcome::Changed,
            (Outcome::Consumed, _) | (_, Outcome::Consumed) => Outcome::Consumed,
            _ => Outcome::Ignored,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Key(Key),
    Mouse(Mouse),
    Resize(u16, u16),
    Paste(String),
    Tick,
    External(External),
}

#[derive(Clone)]
pub struct External(std::sync::Arc<dyn std::any::Any + Send + Sync>);

impl External {
    pub fn new<T: std::any::Any + Send + Sync>(message: T) -> Self {
        Self(std::sync::Arc::new(message))
    }

    pub fn downcast_ref<T: std::any::Any>(&self) -> Option<&T> {
        self.0.downcast_ref()
    }
}

impl std::fmt::Debug for External {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("External").finish_non_exhaustive()
    }
}

impl PartialEq for External {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for External {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

impl Key {
    pub fn ctrl(&self) -> bool {
        self.mods.contains(KeyModifiers::CONTROL)
    }
    pub fn shift(&self) -> bool {
        self.mods.contains(KeyModifiers::SHIFT)
    }
    pub fn alt(&self) -> bool {
        self.mods.contains(KeyModifiers::ALT)
    }

    pub fn cmd(&self) -> bool {
        self.mods.contains(KeyModifiers::SUPER)
    }

    pub fn chord(&self) -> bool {
        self.ctrl() || self.alt() || self.cmd()
    }
    pub fn plain(&self) -> bool {
        self.mods.difference(KeyModifiers::SHIFT).is_empty()
    }
    pub fn is(&self, code: KeyCode) -> bool {
        self.code == code && self.plain()
    }
    pub fn is_char(&self, c: char) -> bool {
        self.code == KeyCode::Char(c) && self.plain()
    }
    pub fn ctrl_char(&self, c: char) -> bool {
        self.code == KeyCode::Char(c) && self.ctrl()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseKind {
    Move,
    Down,
    Up,
    Drag,

    Secondary,

    SecondaryDrag,

    SecondaryUp,

    Middle,
    WheelUp,
    WheelDown,
    WheelLeft,
    WheelRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mouse {
    pub kind: MouseKind,
    pub pos: Position,

    pub mods: KeyModifiers,
}

impl Mouse {
    pub const fn new(kind: MouseKind, pos: Position) -> Self {
        Self {
            kind,
            pos,
            mods: KeyModifiers::NONE,
        }
    }

    pub const fn with_mods(mut self, mods: KeyModifiers) -> Self {
        self.mods = mods;
        self
    }

    pub fn shift(&self) -> bool {
        self.mods.contains(KeyModifiers::SHIFT)
    }
    pub fn ctrl(&self) -> bool {
        self.mods.contains(KeyModifiers::CONTROL)
    }
    pub fn alt(&self) -> bool {
        self.mods.contains(KeyModifiers::ALT)
    }

    pub fn wheel(&self) -> Option<(i32, i32)> {
        match self.kind {
            MouseKind::WheelUp => Some((0, -1)),
            MouseKind::WheelDown => Some((0, 1)),
            MouseKind::WheelLeft => Some((-1, 0)),
            MouseKind::WheelRight => Some((1, 0)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputConfig {
    pub shift_wheel_horizontal: bool,

    pub ctrl_click_secondary: bool,

    pub legacy_ctrl_punctuation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientPlatform {
    MacOs,

    Other,
}

impl ClientPlatform {
    pub const OVERRIDE_ENV: &'static str = "JUNIE_TUI_CLIENT_PLATFORM";

    pub const MACOS_TERM_PROGRAMS: [&'static str; 2] = ["Apple_Terminal", "iTerm.app"];

    pub const fn build_target() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Other
        }
    }

    pub fn from_env(get: impl Fn(&str) -> Option<String>) -> Self {
        if let Some(v) = get(Self::OVERRIDE_ENV) {
            match v.trim().to_ascii_lowercase().as_str() {
                "macos" | "mac" | "darwin" => return Self::MacOs,
                "other" | "linux" | "windows" => return Self::Other,
                _ => {}
            }
        }
        if get("LC_TERMINAL").is_some_and(|v| v == "iTerm2") {
            return Self::MacOs;
        }
        if get("TERM_PROGRAM").is_some_and(|v| Self::MACOS_TERM_PROGRAMS.contains(&v.as_str())) {
            return Self::MacOs;
        }
        Self::build_target()
    }

    pub fn current() -> Self {
        #[cfg(feature = "work-metrics")]
        if let Some(pinned) = pin::PINNED.with(std::cell::Cell::get) {
            return pinned;
        }
        static CURRENT: std::sync::OnceLock<ClientPlatform> = std::sync::OnceLock::new();
        *CURRENT.get_or_init(|| Self::from_env(|k| std::env::var(k).ok()))
    }

    pub const fn is_macos(self) -> bool {
        matches!(self, Self::MacOs)
    }
}

/// Fixes the platform that [`ClientPlatform::current`] reports on one thread.
/// Key labels and hints differ by platform, so a work count that includes them
/// would otherwise depend on the host and on `JUNIE_TUI_CLIENT_PLATFORM`.
#[cfg(feature = "work-metrics")]
mod pin {
    use std::cell::Cell;

    use super::ClientPlatform;

    thread_local! {
        pub(super) static PINNED: Cell<Option<ClientPlatform>> = const { Cell::new(None) };
    }

    pub(crate) struct PlatformPin(Option<ClientPlatform>);

    impl PlatformPin {
        pub(crate) fn new(platform: ClientPlatform) -> Self {
            Self(PINNED.with(|p| p.replace(Some(platform))))
        }
    }

    impl Drop for PlatformPin {
        fn drop(&mut self) {
            PINNED.with(|p| p.set(self.0));
        }
    }
}

#[cfg(feature = "work-metrics")]
pub(crate) use pin::PlatformPin;

impl InputConfig {
    pub const fn for_platform(platform: ClientPlatform) -> Self {
        Self {
            shift_wheel_horizontal: true,
            ctrl_click_secondary: platform.is_macos(),
            legacy_ctrl_punctuation: true,
        }
    }
}

impl Default for InputConfig {
    fn default() -> Self {
        Self::for_platform(ClientPlatform::current())
    }
}

impl Input {
    pub fn from_crossterm(ev: Event) -> Option<Self> {
        Self::from_crossterm_with(ev, InputConfig::default())
    }

    pub fn from_crossterm_with(ev: Event, config: InputConfig) -> Option<Self> {
        let ev = match ev {
            Event::Mouse(mut m)
                if config.shift_wheel_horizontal && m.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                m.kind = match m.kind {
                    MouseEventKind::ScrollUp => MouseEventKind::ScrollLeft,
                    MouseEventKind::ScrollDown => MouseEventKind::ScrollRight,
                    k => k,
                };
                Event::Mouse(m)
            }
            ev => ev,
        };
        match ev {
            Event::Key(KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            }) => {
                let code = match code {
                    KeyCode::Char(c @ '4'..='7')
                        if config.legacy_ctrl_punctuation
                            && modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        KeyCode::Char(match c {
                            '4' => '\\',
                            '5' => ']',
                            '6' => '^',
                            _ => '_',
                        })
                    }
                    code => code,
                };
                Some(Input::Key(Key {
                    code,
                    mods: modifiers,
                }))
            }
            Event::Mouse(MouseEvent {
                kind,
                column,
                row,
                modifiers,
            }) => {
                let kind = match kind {
                    MouseEventKind::Down(MouseButton::Left)
                        if config.ctrl_click_secondary
                            && modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        MouseKind::Secondary
                    }

                    MouseEventKind::Drag(MouseButton::Left)
                        if config.ctrl_click_secondary
                            && modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        MouseKind::SecondaryDrag
                    }
                    MouseEventKind::Up(MouseButton::Left)
                        if config.ctrl_click_secondary
                            && modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        MouseKind::SecondaryUp
                    }
                    MouseEventKind::Moved => MouseKind::Move,
                    MouseEventKind::Down(MouseButton::Left) => MouseKind::Down,
                    MouseEventKind::Up(MouseButton::Left) => MouseKind::Up,
                    MouseEventKind::Drag(MouseButton::Left) => MouseKind::Drag,
                    MouseEventKind::Down(MouseButton::Right) => MouseKind::Secondary,
                    MouseEventKind::Drag(MouseButton::Right) => MouseKind::SecondaryDrag,
                    MouseEventKind::Up(MouseButton::Right) => MouseKind::SecondaryUp,
                    MouseEventKind::Down(MouseButton::Middle) => MouseKind::Middle,
                    MouseEventKind::ScrollUp => MouseKind::WheelUp,
                    MouseEventKind::ScrollDown => MouseKind::WheelDown,

                    MouseEventKind::ScrollLeft => MouseKind::WheelLeft,
                    MouseEventKind::ScrollRight => MouseKind::WheelRight,
                    _ => return None,
                };
                Some(Input::Mouse(
                    Mouse::new(kind, Position::new(column, row)).with_mods(modifiers),
                ))
            }
            Event::Resize(w, h) => Some(Input::Resize(w, h)),
            Event::Paste(s) => Some(Input::Paste(s)),
            _ => None,
        }
    }
}

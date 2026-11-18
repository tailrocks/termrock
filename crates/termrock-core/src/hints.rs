use std::borrow::Cow;

use ratatui::crossterm::event::KeyModifiers;

use crate::event::{ClientPlatform, Key};
use crate::id::WidgetId;
use crate::keyboard::label_deliverable;
use crate::keymap::{Actions, KeyBinding, KeyboardCapabilities, NavKeys};

/// Keys a hint bar always keeps, implemented by the theme's hint bar style.
///
/// `Hint::is_pinned` reads the list through this trait so that `core` does not
/// name `theme`. It is `#[doc(hidden)]` so that `theme`, another crate, can
/// implement it without it joining the declared API.
#[doc(hidden)]
pub trait PinnedKeys {
    fn pinned_keys(&self) -> &[&str];
}

pub const UP_DOWN: &str = "↑↓";
pub const LEFT_RIGHT: &str = "←→";
pub const ARROWS: &str = "↑↓←→";
pub const DELETE: &str = "Delete";

pub fn range(from: u32, to: u32) -> String {
    format!("{from}–{to}")
}

pub fn normalize_key(key: &str) -> Cow<'_, str> {
    const ARROW: [char; 4] = ['↑', '↓', '←', '→'];
    if key == "Del" {
        return Cow::Borrowed(DELETE);
    }
    let chars: Vec<char> = key.chars().collect();
    let mut out = String::with_capacity(key.len());
    let mut changed = false;
    for (i, &c) in chars.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| chars[j]);
        let next = chars.get(i + 1).copied();
        if c == ' '
            && prev.is_some_and(|p| ARROW.contains(&p))
            && next.is_some_and(|n| ARROW.contains(&n))
        {
            changed = true;
            continue;
        }
        if c == '-'
            && prev.is_some_and(|p| p.is_ascii_digit())
            && next.is_some_and(|n| n.is_ascii_digit())
        {
            out.push('–');
            changed = true;
            continue;
        }
        out.push(c);
    }
    if changed {
        Cow::Owned(out)
    } else {
        Cow::Borrowed(key)
    }
}

pub fn modifier_click(mods: KeyModifiers, platform: ClientPlatform) -> Option<&'static str> {
    const MAC: [(KeyModifiers, &str); 3] = [
        (KeyModifiers::ALT, "Option+click"),
        (KeyModifiers::SUPER, "Cmd+click"),
        (KeyModifiers::CONTROL, "Ctrl+click"),
    ];
    const OTHER: [(KeyModifiers, &str); 3] = [
        (KeyModifiers::CONTROL, "Ctrl+click"),
        (KeyModifiers::ALT, "Alt+click"),
        (KeyModifiers::SUPER, "Super+click"),
    ];
    let order = if platform.is_macos() { &MAC } else { &OTHER };
    order
        .iter()
        .find(|(m, _)| mods.contains(*m))
        .map(|&(_, label)| label)
}

pub const fn toggle_hint(
    key: &'static str,
    on: bool,
    when_on: &'static str,
    when_off: &'static str,
) -> Hint {
    hint(key, if on { when_on } else { when_off })
}

pub const EDIT_FIELD: [Hint; 3] = [
    hint("Enter", "Apply"),
    hint("Esc", "Cancel"),
    hint("Tab", "Next field"),
];

pub const CONTEXT_MENU: Hint = hint("Shift+F10", "Menu");

pub const EDIT_TEXT: [Hint; 2] = [hint("Tab", "Next field"), hint("Esc", "Done")];

pub const EDIT_CODE: [Hint; 3] = [
    hint("Tab", "Indent"),
    hint("F6", "Leave"),
    hint("Esc", "Done"),
];

pub const EDIT_CELL: [Hint; 3] = [
    hint("Enter", "Apply"),
    hint("Esc", "Cancel"),
    hint("Tab", "Next cell"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub key: Cow<'static, str>,
    pub action: Cow<'static, str>,

    pub priority: HintPriority,

    pub click_key: Option<Key>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HintPriority {
    #[default]
    Auto,

    Normal,

    Pinned,
}

impl Hint {
    pub fn new(key: impl Into<Cow<'static, str>>, action: impl Into<Cow<'static, str>>) -> Self {
        Self {
            key: key.into(),
            action: action.into(),
            priority: HintPriority::Auto,
            click_key: None,
        }
    }

    pub fn click_key(mut self, key: Key) -> Self {
        self.click_key = Some(key);
        self
    }

    pub fn priority(mut self, priority: HintPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn pinned(self) -> Self {
        self.priority(HintPriority::Pinned)
    }

    pub fn is_pinned(&self, style: &impl PinnedKeys) -> bool {
        match self.priority {
            HintPriority::Pinned => true,
            HintPriority::Normal => false,
            HintPriority::Auto => style.pinned_keys().contains(&self.key.as_ref()),
        }
    }
}

pub fn deliverable_line(line: &str) -> std::borrow::Cow<'_, str> {
    let keep = |part: &&str| label_deliverable(part.split(' ').next().unwrap_or_default());
    if line.split(" · ").all(|p| keep(&p)) {
        return std::borrow::Cow::Borrowed(line);
    }
    std::borrow::Cow::Owned(
        line.split(" · ")
            .filter(keep)
            .collect::<Vec<_>>()
            .join(" · "),
    )
}

pub const fn hint(key: &'static str, action: &'static str) -> Hint {
    Hint {
        key: Cow::Borrowed(key),
        action: Cow::Borrowed(action),
        priority: HintPriority::Auto,
        click_key: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HintCx {
    pub focus: Option<WidgetId>,
    pub platform: ClientPlatform,
    pub keyboard: KeyboardCapabilities,
}

impl Default for HintCx {
    fn default() -> Self {
        Self::current(None)
    }
}

impl HintCx {
    pub const fn with_environment(
        focus: Option<WidgetId>,
        platform: ClientPlatform,
        keyboard: KeyboardCapabilities,
    ) -> Self {
        Self {
            focus,
            platform,
            keyboard,
        }
    }

    pub fn current(focus: Option<WidgetId>) -> Self {
        Self {
            focus,
            platform: ClientPlatform::current(),
            keyboard: KeyboardCapabilities::current(),
        }
    }

    pub fn new(focus: Option<WidgetId>) -> Self {
        Self::current(focus)
    }
}

pub trait HasHints {
    fn hints(&self, cx: &HintCx) -> Vec<Hint>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub key: Option<KeyBinding>,

    pub label: Cow<'static, str>,

    pub action: Cow<'static, str>,
}

impl Binding {
    pub fn key(key: KeyBinding, action: impl Into<Cow<'static, str>>) -> Self {
        Self {
            key: Some(key),
            label: key.label().into(),
            action: action.into(),
        }
    }

    pub fn group(label: &'static str, action: impl Into<Cow<'static, str>>) -> Self {
        Self {
            key: None,
            label: Cow::Borrowed(label),
            action: action.into(),
        }
    }
}

pub trait Bindings {
    fn bindings(&self) -> Vec<Binding>;

    fn chords(&self) -> Vec<KeyBinding> {
        self.bindings().into_iter().filter_map(|b| b.key).collect()
    }
}

pub fn keymap_bindings<A: Copy>(
    map: &[(KeyBinding, A)],
    label: impl Fn(A) -> &'static str,
) -> Vec<Binding> {
    map.iter()
        .filter(|(k, _)| k.deliverable())
        .map(|&(k, a)| Binding::key(k, label(a)))
        .collect()
}

pub fn actions_bindings<A: Actions>(map: &[(KeyBinding, A)]) -> Vec<Binding> {
    keymap_bindings(map, A::label)
}

pub fn nav_bindings(nav: &NavKeys, type_ahead: bool) -> Vec<Binding> {
    let mut v = vec![];
    if let Some(k) = nav.move_hint(type_ahead) {
        v.push(Binding::group(k, "Move"));
    }
    if let Some(k) = nav.page_hint() {
        v.push(Binding::group(k, "Page"));
    }
    if let Some(k) = nav.ends_hint(type_ahead) {
        v.push(Binding::group(k, "First / last"));
    }
    if type_ahead {
        v.push(Binding::group("a–z", "Jump to a row by name"));
    }
    v
}

pub const TYPE_TO_JUMP: Hint = Hint {
    key: Cow::Borrowed("Type"),
    action: Cow::Borrowed("to jump"),
    priority: HintPriority::Normal,
    click_key: None,
};

pub fn nav_hints(nav: &NavKeys, type_ahead: bool, ends: bool) -> Vec<Hint> {
    let mut v = Vec::new();
    if let Some(k) = nav.move_hint(type_ahead) {
        v.push(Hint::new(k, "Move"));
    }
    if ends && let Some(k) = nav.ends_hint(type_ahead) {
        v.push(Hint::new(k, "Ends").priority(HintPriority::Normal));
    }
    if type_ahead {
        v.push(TYPE_TO_JUMP);
    }
    v
}

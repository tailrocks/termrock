use std::io::Write;
use std::sync::atomic::{AtomicU8, Ordering};

use super::hit::HitRegistry;
use super::pointer::PointerState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum PointerShape {
    #[default]
    Default,

    Text,

    Pointer,

    ColResize,

    RowResize,
}

impl PointerShape {
    pub const fn css_name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Text => "text",
            Self::Pointer => "pointer",
            Self::ColResize => "col-resize",
            Self::RowResize => "row-resize",
        }
    }

    pub fn osc22(self) -> String {
        format!("\x1b]22;{}\x1b\\", self.css_name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum CaretShape {
    #[default]
    Bar,

    Block,

    Underline,

    Terminal,
}

impl CaretShape {
    pub const fn decscusr(self) -> Option<&'static str> {
        match self {
            Self::Block => Some("\x1b[2 q"),
            Self::Underline => Some("\x1b[4 q"),
            Self::Bar => Some("\x1b[6 q"),
            Self::Terminal => None,
        }
    }
}

pub const CARET_RESET: &str = "\x1b[0 q";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShapeSupport {
    pub pointer: bool,

    pub caret: bool,
}

impl ShapeSupport {
    pub const NONE: Self = Self {
        pointer: false,
        caret: false,
    };

    pub fn from_env(get: impl Fn(&str) -> Option<String>) -> Self {
        let term = get("TERM").unwrap_or_default().to_ascii_lowercase();
        let program = get("TERM_PROGRAM").unwrap_or_default().to_ascii_lowercase();
        let multiplexed = term.starts_with("tmux")
            || term.starts_with("screen")
            || program == "tmux"
            || get("TMUX").is_some();
        let modern = term.contains("kitty")
            || term.contains("ghostty")
            || term.starts_with("foot")
            || term.starts_with("wezterm")
            || program == "wezterm"
            || program == "ghostty"
            || get("KITTY_WINDOW_ID").is_some();
        let pointer = modern && !multiplexed;
        let caret = modern
            || term.starts_with("xterm")
            || term.starts_with("tmux")
            || term.starts_with("screen")
            || term.starts_with("alacritty")
            || term.starts_with("rxvt-unicode")
            || term.starts_with("vte");
        Self { pointer, caret }
    }

    pub fn current() -> Self {
        static CURRENT: std::sync::OnceLock<ShapeSupport> = std::sync::OnceLock::new();
        *CURRENT.get_or_init(|| Self::from_env(|k| std::env::var(k).ok()))
    }
}

pub fn under(pointer: &PointerState, hits: &HitRegistry) -> PointerShape {
    pointer
        .pressed
        .or(pointer.hover)
        .and_then(|id| hits.shape_of(id))
        .unwrap_or_default()
}

pub fn shapes_for(pointer: &PointerState, hits: &HitRegistry) -> Shapes {
    Shapes {
        pointer: pointer.config.pointer_shapes.then(|| under(pointer, hits)),
        caret: Some(pointer.config.caret_shape),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Shapes {
    pub pointer: Option<PointerShape>,

    pub caret: Option<CaretShape>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShapeSync {
    support: ShapeSupport,
    pointer: Option<PointerShape>,
    caret: Option<CaretShape>,
}

static SENT: AtomicU8 = AtomicU8::new(0);
const SENT_POINTER: u8 = 1;
const SENT_CARET: u8 = 2;

impl ShapeSync {
    pub fn new(support: ShapeSupport) -> Self {
        Self {
            support,
            pointer: None,
            caret: None,
        }
    }

    pub fn apply(&mut self, want: Shapes, out: &mut impl Write) -> std::io::Result<bool> {
        let mut wrote = false;
        if self.support.pointer
            && let Some(p) = want.pointer
            && self.pointer != Some(p)
        {
            out.write_all(p.osc22().as_bytes())?;
            self.pointer = Some(p);
            SENT.fetch_or(SENT_POINTER, Ordering::Relaxed);
            wrote = true;
        }
        if self.support.caret
            && let Some(c) = want.caret
            && self.caret != Some(c)
            && let Some(seq) = c.decscusr()
        {
            out.write_all(seq.as_bytes())?;
            self.caret = Some(c);
            SENT.fetch_or(SENT_CARET, Ordering::Relaxed);
            wrote = true;
        }
        Ok(wrote)
    }

    pub fn forget(&mut self) {
        self.pointer = None;
        self.caret = None;
    }
}

pub fn reset_sequence() -> String {
    let sent = SENT.swap(0, Ordering::Relaxed);
    let mut s = String::new();
    if sent & SENT_POINTER != 0 {
        s.push_str(&PointerShape::Default.osc22());
    }
    if sent & SENT_CARET != 0 {
        s.push_str(CARET_RESET);
    }
    s
}

use ratatui_core::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerShape {
    Default,
    Pointer,
    Text,
    Crosshair,
    EwResize,
    NsResize,
    Grabbing,
}

impl PointerShape {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Pointer => "pointer",
            Self::Text => "text",
            Self::Crosshair => "crosshair",
            Self::EwResize => "ew-resize",
            Self::NsResize => "ns-resize",
            Self::Grabbing => "grabbing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HyperlinkRegion<'a, Id> {
    pub id: Id,
    pub area: Rect,
    pub url: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardSelection {
    /// The system clipboard (`c`).
    Clipboard,
    /// The primary selection (`p`).
    Primary,
}

impl ClipboardSelection {
    pub(crate) const fn letter(self) -> &'static str {
        match self {
            Self::Clipboard => "c",
            Self::Primary => "p",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipboardWrite<'a> {
    pub selection: ClipboardSelection,
    pub text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request<'a> {
    Pointer(PointerShape),
    Clipboard(ClipboardWrite<'a>),
    HyperlinkOpen { id: Option<&'a str>, url: &'a str },
    HyperlinkClose,
}

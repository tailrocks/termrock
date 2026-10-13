use ratatui_core::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerShape {
    Default,
    Pointer,
    Text,
    Crosshair,
}

impl PointerShape {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Pointer => "pointer",
            Self::Text => "text",
            Self::Crosshair => "crosshair",
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
pub struct ClipboardWrite<'a> {
    pub selection: &'a str,
    pub text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request<'a> {
    Pointer(PointerShape),
    Clipboard(ClipboardWrite<'a>),
    HyperlinkOpen { id: Option<&'a str>, url: &'a str },
    HyperlinkClose,
}

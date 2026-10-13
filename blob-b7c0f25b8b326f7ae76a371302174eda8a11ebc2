use ratatui_core::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Available `PointerShape` choices.
pub enum PointerShape {
    /// Selects the `Default` behavior.
    Default,
    /// Selects the `Pointer` behavior.
    Pointer,
    /// Selects the `Text` behavior.
    Text,
    /// Selects the `Crosshair` behavior.
    Crosshair,
    /// Selects the `EwResize` behavior.
    EwResize,
    /// Selects the `NsResize` behavior.
    NsResize,
    /// Selects the `Grabbing` behavior.
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
/// Data carried by `HyperlinkRegion`.
pub struct HyperlinkRegion<'a, Id> {
    /// Documentation for `item`.
    pub id: Id,
    /// Documentation for `item`.
    pub area: Rect,
    /// Documentation for `item`.
    pub url: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Available `ClipboardSelection` choices.
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
/// Data carried by `ClipboardWrite`.
pub struct ClipboardWrite<'a> {
    /// Documentation for `item`.
    pub selection: ClipboardSelection,
    /// Documentation for `item`.
    pub text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Available `Request` choices.
pub enum Request<'a> {
    /// Selects the `Pointer` behavior.
    Pointer(PointerShape),
    /// Selects the `Clipboard` behavior.
    Clipboard(ClipboardWrite<'a>),
    /// Selects the `HyperlinkOpen` behavior.
    /// The `HyperlinkOpen { id` value.
    HyperlinkOpen {
        /// Optional stable identifier used to update an existing link region.
        id: Option<&'a str>,
        /// Validated hyperlink destination.
        url: &'a str,
    },
    /// Selects the `HyperlinkClose` behavior.
    HyperlinkClose,
}

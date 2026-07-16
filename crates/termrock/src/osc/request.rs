use ratatui_core::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Terminal pointer cursor shapes supported by OSC requests.
pub enum PointerShape {
    /// The default terminal pointer cursor.
    Default,
    /// The pointer terminal pointer cursor.
    Pointer,
    /// The text terminal pointer cursor.
    Text,
    /// The crosshair terminal pointer cursor.
    Crosshair,
    /// The ew resize terminal pointer cursor.
    EwResize,
    /// The ns resize terminal pointer cursor.
    NsResize,
    /// The grabbing terminal pointer cursor.
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
/// A painted terminal region associated with a hyperlink.
pub struct HyperlinkRegion<'a, Id> {
    /// Stable identity used for selection and activation.
    pub id: Id,
    /// Painted terminal rectangle used for hit testing.
    pub area: Rect,
    /// Caller-provided hyperlink target.
    pub url: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Clipboard targets supported by terminal clipboard requests.
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
/// A sanitized terminal clipboard write request.
pub struct ClipboardWrite<'a> {
    /// Terminal clipboard target.
    pub selection: ClipboardSelection,
    /// Caller-visible text.
    pub text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Typed terminal-control requests encoded by the OSC module.
pub enum Request<'a> {
    /// Requests terminal pointer behavior.
    Pointer(PointerShape),
    /// Requests terminal clipboard behavior.
    Clipboard(ClipboardWrite<'a>),
    /// Requests terminal hyperlink open behavior.
    HyperlinkOpen {
        /// Optional stable identifier used to update an existing link region.
        id: Option<&'a str>,
        /// Validated hyperlink destination.
        url: &'a str,
    },
    /// Requests terminal hyperlink close behavior.
    HyperlinkClose,
}

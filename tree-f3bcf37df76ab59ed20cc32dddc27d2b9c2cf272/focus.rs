//! Lookbook identities used with TermRock's shared focus registry.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusId {
    Sidebar,
    Preview,
    Controls,
    ModalContinue,
    ModalDisabled,
    ModalCancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusScope {
    Screen,
    Modal,
}

pub(crate) type FocusRing = termrock::interaction::FocusRing<FocusId, FocusScope>;

//! Lookbook focus identities + host-owned FocusRing.
//!
//! TermRock M3 removed public FocusRing (`InteractionScene` is the library
//! authority). Lookbook keeps a host-local ring for the studio shell until a
//! full InteractionScene HostFrame cutover.

pub(crate) use crate::host_focus::{FocusOutcome, FocusTarget};

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

pub(crate) type FocusRing = crate::host_focus::FocusRing<FocusId, FocusScope>;

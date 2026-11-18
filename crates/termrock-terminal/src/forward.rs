/// What a host lets reach a terminal program from the operator: a pure filter
/// over the bytes of a key and of a paste.
///
/// `Ctrl+D` ends the input of a shell and of a coding agent, so a host that
/// keeps its panes alive never forwards it, and `Ctrl+Q` is the host's own
/// chord. A paste loses its control bytes (an `ESC` in it could close the
/// bracket early), and a paste with several lines into a program that did not
/// ask for brackets is held for the host to confirm, because each line runs as
/// it arrives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardPolicy {
    /// Bytes that no key sends.
    pub never: Vec<u8>,
    /// The most bytes of text one paste may hold.
    pub paste_max_bytes: usize,
}

/// What to do with a paste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Paste {
    /// Send these bytes (bracketed when the program asked for brackets). The
    /// bytes are empty when nothing was left after the control bytes went.
    Send(Vec<u8>),
    /// The program has no paste protection and the text has newlines: ask
    /// first. `lines` counts the lines of the cleaned text.
    NeedsConfirm { lines: usize },
    /// The text is longer than `paste_max_bytes`; `bytes` is its length.
    TooLarge { bytes: usize },
}

const BRACKET_OPEN: &[u8] = b"\x1b[200~";
const BRACKET_CLOSE: &[u8] = b"\x1b[201~";

impl ForwardPolicy {
    pub const NEVER: [u8; 2] = [0x04, 0x11];
    pub const PASTE_MAX_BYTES: usize = 64 * 1024;

    pub fn new(paste_max_bytes: usize) -> Self {
        Self {
            never: Self::NEVER.to_vec(),
            paste_max_bytes,
        }
    }

    /// The bytes of one key, or `None` when the key must not reach the
    /// program.
    pub fn key(&self, bytes: Vec<u8>) -> Option<Vec<u8>> {
        (!bytes.iter().any(|b| self.never.contains(b))).then_some(bytes)
    }

    /// Decides one paste. `bracketed` is whether the program turned on
    /// bracketed paste.
    pub fn paste(&self, text: &str, bracketed: bool) -> Paste {
        let clean = match self.clean(text) {
            Ok(clean) => clean,
            Err(too_large) => return too_large,
        };
        if bracketed {
            if clean.is_empty() {
                return Paste::Send(Vec::new());
            }
            return Paste::Send([BRACKET_OPEN, clean.as_bytes(), BRACKET_CLOSE].concat());
        }
        if clean.contains('\n') {
            return Paste::NeedsConfirm {
                lines: clean.lines().count(),
            };
        }
        Paste::Send(clean.into_bytes())
    }

    /// The paste after the operator confirmed it: sent as it is, with no
    /// brackets, because the program has none to give.
    pub fn confirmed(&self, text: &str) -> Paste {
        match self.clean(text) {
            Ok(clean) => Paste::Send(clean.into_bytes()),
            Err(too_large) => too_large,
        }
    }

    fn clean(&self, text: &str) -> Result<String, Paste> {
        if text.len() > self.paste_max_bytes {
            return Err(Paste::TooLarge { bytes: text.len() });
        }
        Ok(text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .chars()
            .filter(|c| !c.is_control() || matches!(c, '\t' | '\n'))
            .collect())
    }
}

impl Default for ForwardPolicy {
    fn default() -> Self {
        Self::new(Self::PASTE_MAX_BYTES)
    }
}

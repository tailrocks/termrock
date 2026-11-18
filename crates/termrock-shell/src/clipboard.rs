use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::edit::clipboard;
use crate::theme::Tone;
use crate::ui::text::{plural, truncate};

static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);

thread_local! {

    static OUT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub trait NativeClipboard: Send + Sync + std::fmt::Debug {
    fn write(&self, text: &str) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Copied {
    Copied { chars: usize },

    Requested { chars: usize },

    Unsupported,

    TooLarge { bytes: usize, limit: usize },
}

impl Copied {
    pub fn ok(&self) -> bool {
        matches!(self, Self::Copied { .. } | Self::Requested { .. })
    }
}

#[derive(Debug, Clone)]
pub struct Clipboard {
    pub osc52: bool,

    pub limit: usize,

    pub write: bool,

    pub native: Option<Arc<dyn NativeClipboard>>,
}

#[derive(Debug)]
#[must_use = "the hook is removed when this guard is dropped"]
pub struct ClipboardHook {
    owner: u64,
}

impl Drop for ClipboardHook {
    fn drop(&mut self) {
        clipboard::clear_hook(self.owner);
    }
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Clipboard {
    pub const DEFAULT_LIMIT: usize = 100_000;

    pub fn new() -> Self {
        Self {
            osc52: true,
            limit: Self::DEFAULT_LIMIT,
            write: true,
            native: None,
        }
    }

    pub const fn osc52(mut self, on: bool) -> Self {
        self.osc52 = on;
        self
    }

    pub const fn limit(mut self, bytes: usize) -> Self {
        self.limit = bytes;
        self
    }

    pub const fn write(mut self, on: bool) -> Self {
        self.write = on;
        self
    }

    pub fn native(mut self, sink: Arc<dyn NativeClipboard>) -> Self {
        self.native = Some(sink);
        self
    }

    pub fn copy(&self, text: &str) -> Copied {
        clipboard::store(text);
        self.deliver(text)
    }

    fn deliver(&self, text: &str) -> Copied {
        let native = self.native.as_ref().map(|sink| sink.write(text));
        if native == Some(true) {
            return Copied::Copied {
                chars: text.chars().count(),
            };
        }
        match (self.system(text), native) {
            (Copied::Copied { chars }, Some(false)) => Copied::Requested { chars },
            (result, _) => result,
        }
    }

    fn system(&self, text: &str) -> Copied {
        if !self.osc52 {
            return Copied::Unsupported;
        }
        if text.len() > self.limit {
            return Copied::TooLarge {
                bytes: text.len(),
                limit: self.limit,
            };
        }
        if self.write {
            queue_output(osc52(text));
        }
        Copied::Copied {
            chars: text.chars().count(),
        }
    }

    pub fn install(&self) -> ClipboardHook {
        let this = self.clone();
        let owner = NEXT_OWNER.fetch_add(1, Ordering::Relaxed);
        clipboard::set_hook(owner, move |t| {
            this.deliver(t);
        });
        ClipboardHook { owner }
    }

    pub fn report(result: &Copied, text: &str, what: Option<&str>) -> (String, Tone) {
        match result {
            Copied::Copied { .. } if let Some(what) = what => {
                (format!("Copied {what}"), Tone::Secondary)
            }
            Copied::Copied { chars } => {
                let shown = truncate(&text.replace('\n', " · "), 40);
                (
                    format!(
                        "Copied {} · {shown}",
                        plural(*chars, "character", "characters")
                    ),
                    Tone::Secondary,
                )
            }
            Copied::Requested { .. } => (
                "Copy requested · this terminal may ignore OSC 52".to_owned(),
                Tone::Warning,
            ),
            Copied::Unsupported => (
                "Not copied · this terminal does not accept OSC 52".to_owned(),
                Tone::Error,
            ),
            Copied::TooLarge { bytes, limit } => (
                format!(
                    "Not copied · {bytes} bytes exceed the terminal's {limit} byte OSC 52 limit"
                ),
                Tone::Error,
            ),
        }
    }
}

pub fn queue_output(sequence: String) {
    OUT.with(|o| o.borrow_mut().push(sequence));
}

pub fn take_output() -> Vec<String> {
    OUT.with(|o| std::mem::take(&mut *o.borrow_mut()))
}

pub fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

use std::cell::RefCell;

use crate::ui::text::width;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reason {
    pub text: String,
    pub short: Option<String>,
}

impl Reason {
    pub const TEXT_MAX: usize = 60;

    pub const SHORT_MAX: usize = 20;

    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            short: None,
        }
    }

    pub fn short(mut self, short: &str) -> Self {
        self.short = Some(short.to_owned());
        self
    }

    pub fn short_or_text(&self) -> &str {
        self.short.as_deref().unwrap_or(&self.text)
    }

    pub fn within_limits(&self) -> bool {
        width(&self.text) <= Self::TEXT_MAX
            && self
                .short
                .as_deref()
                .is_none_or(|s| width(s) <= Self::SHORT_MAX)
    }

    pub fn text_fits(&self, room: usize) -> bool {
        width(&self.text) <= room
    }

    pub fn short_fits(&self, room: usize) -> bool {
        self.short.as_deref().is_some_and(|s| width(s) <= room)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Control {
    #[default]
    Live,

    Fixed(Reason),
}

impl Control {
    pub fn fixed(reason: Reason) -> Self {
        Self::Fixed(reason)
    }

    pub fn is_fixed(&self) -> bool {
        matches!(self, Self::Fixed(_))
    }

    pub fn reason(&self) -> Option<&Reason> {
        match self {
            Self::Live => None,
            Self::Fixed(reason) => Some(reason),
        }
    }

    pub fn refuse(&self) -> bool {
        match self {
            Self::Live => false,
            Self::Fixed(reason) => {
                raise(reason);
                true
            }
        }
    }
}

thread_local! {
    static SLOT: RefCell<Option<Reason>> = const { RefCell::new(None) };
}

pub fn raise(reason: &Reason) {
    SLOT.replace(Some(reason.clone()));
}

pub fn take_refusal() -> Option<Reason> {
    SLOT.take()
}

pub fn clear_refusal() {
    SLOT.replace(None);
}

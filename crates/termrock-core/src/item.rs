//! The option row descriptor `Item` and the `AsItem` bridge that lets any row type
//! feed the shared row source.
//!
//! `termrock::widgets::option_list::{Item, AsItem}` re-export these. They live
//! here because `[T]: RowSource` for `T: AsItem` has to be implemented in the
//! crate that owns `RowSource` (the orphan rule).

use std::borrow::Cow;

use crate::id::ItemKey;
use crate::list::RowItem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub key: Option<ItemKey>,
    pub label: String,

    pub glyph: Cow<'static, str>,
    pub detail: String,

    pub tag: Option<Cow<'static, str>>,

    pub group: Cow<'static, str>,

    pub matched: Vec<usize>,

    pub insert: Option<String>,

    pub disabled: bool,

    pub reason: Option<String>,

    pub selection_group: Option<ItemKey>,

    pub value: usize,
}

impl Default for Item {
    fn default() -> Self {
        Self::new("")
    }
}

impl Item {
    /// Text bytes a deep clone of this item copies. A borrowed `Cow` copies
    /// no bytes.
    #[cfg(feature = "work-metrics")]
    #[doc(hidden)]
    pub fn payload_bytes(&self) -> usize {
        let owned = |text: &Cow<'static, str>| match text {
            Cow::Owned(text) => text.len(),
            Cow::Borrowed(_) => 0,
        };
        self.label.len()
            + self.detail.len()
            + owned(&self.glyph)
            + self.tag.as_ref().map_or(0, owned)
            + owned(&self.group)
            + self.insert.as_ref().map_or(0, String::len)
            + self.reason.as_ref().map_or(0, String::len)
    }

    pub fn new(label: impl Into<String>) -> Self {
        Self {
            key: None,
            label: label.into(),
            glyph: Cow::Borrowed(""),
            detail: String::new(),
            tag: None,
            group: Cow::Borrowed(""),
            matched: vec![],
            insert: None,
            disabled: false,
            reason: None,
            selection_group: None,
            value: 0,
        }
    }

    pub fn key(mut self, key: ItemKey) -> Self {
        self.key = Some(key);
        self
    }

    pub fn glyph(mut self, glyph: impl Into<Cow<'static, str>>) -> Self {
        self.glyph = glyph.into();
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    pub fn tag(mut self, tag: impl Into<Cow<'static, str>>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn group(mut self, group: impl Into<Cow<'static, str>>) -> Self {
        self.group = group.into();
        self
    }

    pub fn matched(mut self, matched: Vec<usize>) -> Self {
        self.matched = matched;
        self
    }

    pub fn insert(mut self, insert: impl Into<String>) -> Self {
        self.insert = Some(insert.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self.disabled = true;
        self
    }

    pub fn selection_group(mut self, group: ItemKey) -> Self {
        self.selection_group = Some(group);
        self
    }

    pub fn value(mut self, value: usize) -> Self {
        self.value = value;
        self
    }

    pub fn insertion(&self) -> &str {
        self.insert.as_deref().unwrap_or(&self.label)
    }
}

pub trait AsItem {
    fn as_item(&self) -> &Item;
}

impl AsItem for Item {
    fn as_item(&self) -> &Item {
        self
    }
}

impl<T: AsItem> RowItem for T {
    fn label(&self) -> &str {
        &self.as_item().label
    }
    fn enabled(&self) -> bool {
        !self.as_item().disabled
    }
    fn key(&self, index: usize) -> ItemKey {
        self.as_item().key.unwrap_or(ItemKey::index(index))
    }
}

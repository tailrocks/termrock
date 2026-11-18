use std::collections::BTreeSet;

use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::core::event::{ClientPlatform, Key};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    None,

    #[default]
    Single,

    Multi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriState {
    None,

    Mixed,

    All,
}

pub(crate) const CHECK_ON: &str = "[✓]";
pub(crate) const CHECK_OFF: &str = "[ ]";
pub(crate) const CHECK_MIXED: &str = "[–]";

/// The three check strings of a glyph set, implemented by the theme's `Glyphs`.
///
/// `TriState::glyph_in` reads them through this trait so that `core` does not
/// name `theme`. The trait is public only so it can appear in that bound; it is
/// not reachable by path.
mod check_glyphs {
    pub trait CheckGlyphs {
        fn check_on(&self) -> &'static str;
        fn check_off(&self) -> &'static str;
        fn check_mixed(&self) -> &'static str;
    }
}
pub(crate) use check_glyphs::CheckGlyphs;

impl TriState {
    pub fn glyph(self) -> &'static str {
        match self {
            TriState::None => CHECK_OFF,
            TriState::Mixed => CHECK_MIXED,
            TriState::All => CHECK_ON,
        }
    }

    pub fn glyph_in(self, g: &impl CheckGlyphs) -> &'static str {
        match self {
            TriState::None => g.check_off(),
            TriState::Mixed => g.check_mixed(),
            TriState::All => g.check_on(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectGesture {
    Only,

    Toggle,

    Extend { from: usize },

    All,
}

impl SelectGesture {
    pub const TOGGLE_CTRL_CMD: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::SUPER);

    pub const TOGGLE_OPTION_CMD: KeyModifiers = KeyModifiers::ALT.union(KeyModifiers::SUPER);

    pub const TOGGLE_WITH_ALT: KeyModifiers = Self::TOGGLE_CTRL_CMD.union(KeyModifiers::ALT);

    pub const fn toggle_mods_for(platform: ClientPlatform) -> KeyModifiers {
        match platform {
            ClientPlatform::MacOs => Self::TOGGLE_OPTION_CMD,
            ClientPlatform::Other => Self::TOGGLE_CTRL_CMD,
        }
    }

    pub fn toggle_mods() -> KeyModifiers {
        Self::toggle_mods_for(ClientPlatform::current())
    }

    pub fn from_mods(mods: KeyModifiers, toggle: KeyModifiers, from: usize) -> Self {
        if mods.contains(KeyModifiers::SHIFT) {
            Self::Extend { from }
        } else if mods.intersects(toggle) {
            Self::Toggle
        } else {
            Self::Only
        }
    }

    pub fn from_key(key: &Key, from: usize) -> Option<Self> {
        let other = key.ctrl() || key.cmd() || key.alt();
        match key.code {
            KeyCode::Char(' ') if key.mods.is_empty() => Some(Self::Toggle),
            KeyCode::Char(_) => None,
            _ if key.shift() && !other => Some(Self::Extend { from }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Activate {
    Single,

    #[default]
    Double,

    ClickAgain,

    Never,
}

impl Activate {
    pub fn fires(self, count: u8, again: bool) -> bool {
        match self {
            Self::Single => true,
            Self::Double => count >= 2,
            Self::ClickAgain => again,
            Self::Never => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheckClick {
    #[default]
    BoxOnly,

    Row,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClickPolicy {
    pub activate: Activate,

    pub edit_on: Activate,

    pub check: CheckClick,
}

impl ClickPolicy {
    pub const DATA: Self = Self {
        activate: Activate::Double,
        edit_on: Activate::Double,
        check: CheckClick::BoxOnly,
    };

    pub const NAVIGATION: Self = Self {
        activate: Activate::Single,
        edit_on: Activate::Never,
        check: CheckClick::BoxOnly,
    };

    pub const fn activate(mut self, activate: Activate) -> Self {
        self.activate = activate;
        self
    }

    pub const fn edit_on(mut self, edit_on: Activate) -> Self {
        self.edit_on = edit_on;
        self
    }

    pub const fn check(mut self, check: CheckClick) -> Self {
        self.check = check;
        self
    }

    pub fn activates(&self, count: u8, again: bool) -> bool {
        self.activate.fires(count, again)
    }

    pub fn edits(&self, count: u8, again: bool) -> bool {
        self.edit_on.fires(count, again)
    }

    pub fn toggles(&self, on_box: bool) -> bool {
        on_box || self.check == CheckClick::Row
    }
}

impl Default for ClickPolicy {
    fn default() -> Self {
        Self::DATA
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RowSelection {
    rows: BTreeSet<usize>,

    anchor: Option<usize>,

    base: BTreeSet<usize>,
}

impl RowSelection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn contains(&self, src: usize) -> bool {
        self.rows.contains(&src)
    }

    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.rows.iter().copied()
    }

    pub fn to_vec(&self) -> Vec<usize> {
        self.iter().collect()
    }

    pub fn anchor(&self) -> Option<usize> {
        self.anchor
    }

    fn set_anchor(&mut self, src: Option<usize>) {
        self.anchor = src;
        self.base = self.rows.clone();
        if let Some(a) = src {
            self.base.remove(&a);
        }
    }

    pub fn clear(&mut self) -> bool {
        let changed = !self.rows.is_empty();
        self.rows.clear();
        self.set_anchor(None);
        changed
    }

    pub fn select_only(&mut self, src: usize, selectable: impl Fn(usize) -> bool) -> bool {
        let before = self.rows.clone();
        self.rows.retain(|&s| !selectable(s));
        if selectable(src) {
            self.rows.insert(src);
            self.set_anchor(Some(src));
        } else {
            self.set_anchor(None);
        }
        self.rows != before
    }

    pub fn toggle(&mut self, src: usize, selectable: impl Fn(usize) -> bool) -> bool {
        if !selectable(src) {
            return false;
        }
        if !self.rows.remove(&src) {
            self.rows.insert(src);
        }
        self.set_anchor(Some(src));
        true
    }

    pub fn extend(
        &mut self,
        order: &[usize],
        from: usize,
        to: usize,
        selectable: impl Fn(usize) -> bool,
    ) -> bool {
        if order.is_empty() {
            return false;
        }
        let to = to.min(order.len() - 1);
        let anchor_display = self
            .anchor
            .and_then(|a| order.iter().position(|&s| s == a))
            .unwrap_or_else(|| {
                let from = from.min(order.len() - 1);
                self.set_anchor(Some(order[from]));
                from
            });
        let (lo, hi) = if anchor_display <= to {
            (anchor_display, to)
        } else {
            (to, anchor_display)
        };
        let mut next = self.base.clone();
        next.extend(order[lo..=hi].iter().copied().filter(|&s| selectable(s)));
        next.extend(self.rows.iter().copied().filter(|&s| !selectable(s)));
        let changed = next != self.rows;
        self.rows = next;
        changed
    }

    pub fn select_all(
        &mut self,
        rows: impl IntoIterator<Item = usize>,
        selectable: impl Fn(usize) -> bool,
    ) -> bool {
        let before = self.rows.len();
        self.rows
            .extend(rows.into_iter().filter(|&s| selectable(s)));
        self.set_anchor(self.anchor);
        self.rows.len() != before
    }

    pub fn toggle_all(
        &mut self,
        rows: impl IntoIterator<Item = usize> + Clone,
        selectable: impl Fn(usize) -> bool + Copy,
    ) -> bool {
        match self.tri_state(rows.clone(), selectable) {
            TriState::All => {
                let before = self.rows.len();
                self.rows.retain(|&s| !selectable(s));
                self.set_anchor(None);
                self.rows.len() != before
            }
            _ => self.select_all(rows, selectable),
        }
    }

    pub fn tri_state(
        &self,
        rows: impl IntoIterator<Item = usize>,
        selectable: impl Fn(usize) -> bool,
    ) -> TriState {
        let mut total = 0usize;
        let mut on = 0usize;
        for s in rows.into_iter().filter(|&s| selectable(s)) {
            total += 1;
            if self.rows.contains(&s) {
                on += 1;
            }
        }
        if on == 0 {
            TriState::None
        } else if on == total {
            TriState::All
        } else {
            TriState::Mixed
        }
    }

    pub fn apply(
        &mut self,
        g: SelectGesture,
        to: usize,
        order: &[usize],
        enabled: impl Fn(usize) -> bool,
    ) -> bool {
        if g == SelectGesture::All {
            return self.select_all(order.iter().copied(), enabled);
        }
        let Some(&src) = order.get(to) else {
            return false;
        };
        match g {
            SelectGesture::Only => self.select_only(src, enabled),
            SelectGesture::Toggle => self.toggle(src, enabled),
            SelectGesture::Extend { from } => self.extend(order, from, to, enabled),
            SelectGesture::All => false,
        }
    }

    pub fn context_select(
        &mut self,
        to: usize,
        order: &[usize],
        enabled: impl Fn(usize) -> bool,
    ) -> bool {
        match order.get(to) {
            Some(&src) if !self.contains(src) => {
                self.apply(SelectGesture::Only, to, order, enabled)
            }
            _ => false,
        }
    }

    pub fn set(&mut self, src: usize, on: bool) -> bool {
        if on {
            self.rows.insert(src)
        } else {
            self.rows.remove(&src)
        }
    }

    /// Replaces the selected rows and the range anchor in one step.
    pub fn replace(&mut self, rows: impl IntoIterator<Item = usize>, anchor: Option<usize>) {
        self.rows = rows.into_iter().collect();
        self.set_anchor(anchor);
    }

    pub fn retain(&mut self, keep: impl Fn(usize) -> bool) {
        self.rows.retain(|&s| keep(s));
        self.base.retain(|&s| keep(s));
        if self.anchor.is_some_and(|a| !keep(a)) {
            self.anchor = None;
        }
    }

    pub fn remap(&mut self, map: impl Fn(usize) -> Option<usize>) {
        self.rows = self.rows.iter().filter_map(|&s| map(s)).collect();
        self.base = self.base.iter().filter_map(|&s| map(s)).collect();
        self.anchor = self.anchor.and_then(&map);
    }
}

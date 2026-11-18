use std::cell::Cell;

use crate::event::{Key, Outcome};
use crate::hit::Axis;
use crate::id::ItemKey;
use crate::keymap::{Nav, NavKeys};
use crate::typeahead::TypeAhead;

pub trait RowSource {
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn enabled(&self, i: usize) -> bool {
        let _ = i;
        true
    }

    fn label(&self, i: usize) -> &str;

    fn key(&self, i: usize) -> ItemKey {
        ItemKey::index(i)
    }
}

impl RowSource for [&str] {
    fn len(&self) -> usize {
        <[&str]>::len(self)
    }
    fn label(&self, i: usize) -> &str {
        self[i]
    }
}

impl RowSource for [String] {
    fn len(&self) -> usize {
        <[String]>::len(self)
    }
    fn label(&self, i: usize) -> &str {
        termrock_metrics::record_work!(LabelsRead, 1usize);
        &self[i]
    }
}

/// One row of a slice that is a [`RowSource`]: the three things a source asks
/// of a row. A slice of rows cannot get `RowSource` from a trait of another
/// crate, so the row types implement this one instead.
#[doc(hidden)]
pub trait RowItem {
    fn label(&self) -> &str;

    fn enabled(&self) -> bool {
        true
    }

    fn key(&self, index: usize) -> ItemKey {
        ItemKey::index(index)
    }
}

impl<T: RowItem> RowSource for [T] {
    fn len(&self) -> usize {
        <[T]>::len(self)
    }
    fn enabled(&self, i: usize) -> bool {
        RowItem::enabled(&self[i])
    }
    fn label(&self, i: usize) -> &str {
        RowItem::label(&self[i])
    }
    fn key(&self, i: usize) -> ItemKey {
        RowItem::key(&self[i], i)
    }
}

pub struct RowFn<'a> {
    pub len: usize,

    pub enabled: &'a dyn Fn(usize) -> bool,

    pub label: &'a dyn Fn(usize) -> &'a str,
}

impl RowSource for RowFn<'_> {
    fn len(&self) -> usize {
        self.len
    }
    fn enabled(&self, i: usize) -> bool {
        (self.enabled)(i)
    }
    fn label(&self, i: usize) -> &str {
        (self.label)(i)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved {
    Ignored,

    Stayed(usize),

    To(usize),
}

impl Moved {
    pub fn index(self) -> Option<usize> {
        match self {
            Moved::Ignored => None,
            Moved::Stayed(i) | Moved::To(i) => Some(i),
        }
    }

    pub fn moved(self) -> bool {
        matches!(self, Moved::To(_))
    }

    pub fn outcome(self) -> Outcome {
        match self {
            Moved::Ignored => Outcome::Ignored,
            Moved::Stayed(_) => Outcome::Consumed,
            Moved::To(_) => Outcome::Changed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub index: usize,

    pub wrap: bool,

    pub axis: Axis,

    pub skip_disabled: bool,
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Cursor {
    pub const fn new(index: usize) -> Self {
        Self {
            index,
            wrap: false,
            axis: Axis::Vertical,
            skip_disabled: true,
        }
    }

    pub const fn wrap(mut self, on: bool) -> Self {
        self.wrap = on;
        self
    }

    pub const fn axis(mut self, axis: Axis) -> Self {
        self.axis = axis;
        self
    }

    pub const fn skip_disabled(mut self, on: bool) -> Self {
        self.skip_disabled = on;
        self
    }

    fn usable<S: RowSource + ?Sized>(&self, src: &S, i: usize) -> bool {
        !self.skip_disabled || src.enabled(i)
    }

    fn settle(&mut self, from: usize, to: usize) -> Moved {
        self.index = to;
        if to == from {
            Moved::Stayed(to)
        } else {
            Moved::To(to)
        }
    }

    pub fn apply<S: RowSource + ?Sized>(&mut self, nav: Nav, src: &S, page: usize) -> Moved {
        let n = src.len();
        if n == 0 {
            return Moved::Ignored;
        }
        let vertical = self.axis == Axis::Vertical;
        match nav {
            Nav::Up | Nav::Down if vertical => self.step(if nav == Nav::Up { -1 } else { 1 }, src),
            Nav::Left | Nav::Right if !vertical => {
                self.step(if nav == Nav::Left { -1 } else { 1 }, src)
            }
            Nav::PageUp | Nav::PageDown if vertical => {
                let from = self.index.min(n - 1);
                let page = page.max(1);
                let to = if nav == Nav::PageUp {
                    from.saturating_sub(page)
                } else {
                    (from + page).min(n - 1)
                };
                let to = self.nearest(src, to, from);
                self.settle(from, to)
            }
            Nav::Home => self.first(src),
            Nav::End => self.last(src),
            _ => Moved::Ignored,
        }
    }

    pub fn step<S: RowSource + ?Sized>(&mut self, delta: isize, src: &S) -> Moved {
        let n = src.len();
        if n == 0 {
            return Moved::Ignored;
        }
        let from = self.index.min(n - 1);
        let mut i = from;
        for _ in 0..n {
            let next = crate::nav::step_index(i, n, delta, self.wrap);
            if next == i {
                break;
            }
            i = next;
            if self.usable(src, i) {
                return self.settle(from, i);
            }
        }
        self.settle(from, from)
    }

    pub fn first<S: RowSource + ?Sized>(&mut self, src: &S) -> Moved {
        let n = src.len();
        if n == 0 {
            return Moved::Ignored;
        }
        let from = self.index.min(n - 1);
        let to = (0..n).find(|&i| self.usable(src, i)).unwrap_or(from);
        self.settle(from, to)
    }

    pub fn last<S: RowSource + ?Sized>(&mut self, src: &S) -> Moved {
        let n = src.len();
        if n == 0 {
            return Moved::Ignored;
        }
        let from = self.index.min(n - 1);
        let to = (0..n).rev().find(|&i| self.usable(src, i)).unwrap_or(from);
        self.settle(from, to)
    }

    fn nearest<S: RowSource + ?Sized>(&self, src: &S, to: usize, from: usize) -> usize {
        let n = src.len();
        if self.usable(src, to) {
            return to;
        }
        let ahead: Box<dyn Iterator<Item = usize>> = if to >= from {
            Box::new(to..n)
        } else {
            Box::new((0..=to).rev())
        };
        let behind: Box<dyn Iterator<Item = usize>> = if to >= from {
            Box::new((0..to).rev())
        } else {
            Box::new(to + 1..n)
        };
        ahead
            .chain(behind)
            .find(|&i| self.usable(src, i))
            .unwrap_or(from)
    }

    pub fn select<S: RowSource + ?Sized>(&mut self, i: usize, src: &S) -> Moved {
        if i >= src.len() {
            return Moved::Ignored;
        }
        let from = self.index;
        self.settle(from, i)
    }

    pub fn type_ahead<S: RowSource + ?Sized>(
        &mut self,
        c: char,
        ta: &mut TypeAhead,
        src: &S,
    ) -> Moved {
        let n = src.len();
        if n == 0 {
            return Moved::Ignored;
        }
        let from = self.index.min(n - 1);
        let prefix = ta.feed(c).to_owned();
        let found = TypeAhead::find(&prefix, (0..n).map(|i| src.label(i)), from, |i| {
            src.enabled(i)
        });
        match found {
            Some(i) => self.settle(from, i),
            None => Moved::Stayed(from),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStep {
    Nav(Nav),

    Type(char),
}

impl KeyStep {
    pub fn of(key: &Key, nav: &NavKeys, type_ahead: bool) -> Option<Self> {
        Self::ordered(key, nav, type_ahead, TypeAheadPrecedence::TypeAhead)
    }

    pub fn ordered(
        key: &Key,
        nav: &NavKeys,
        type_ahead: bool,
        precedence: TypeAheadPrecedence,
    ) -> Option<Self> {
        if type_ahead && let Some(c) = TypeAhead::char_of(key) {
            if precedence == TypeAheadPrecedence::Shortcuts
                && ROUTE.with(Cell::get) == Pass::Shortcuts
            {
                YIELDED.with(|y| y.set(true));
            } else {
                return Some(KeyStep::Type(c));
            }
        }
        nav.action(key).map(KeyStep::Nav)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum TypeAheadPrecedence {
    #[default]
    Shortcuts,

    TypeAhead,
}

thread_local! {
    static PRECEDENCE: Cell<TypeAheadPrecedence> =
        const { Cell::new(TypeAheadPrecedence::Shortcuts) };
    static ROUTE: Cell<Pass> = const { Cell::new(Pass::Direct) };
    static YIELDED: Cell<bool> = const { Cell::new(false) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pass {
    Direct,

    Shortcuts,

    TypeAhead,
}

impl TypeAheadPrecedence {
    pub fn install(self) {
        PRECEDENCE.with(|p| p.set(self));
    }

    pub fn current() -> Self {
        PRECEDENCE.with(Cell::get)
    }
}

struct PassGuard(Pass);

impl PassGuard {
    fn enter(pass: Pass) -> Self {
        Self(ROUTE.with(|r| r.replace(pass)))
    }
}

impl Drop for PassGuard {
    fn drop(&mut self) {
        ROUTE.with(|r| r.set(self.0));
    }
}

pub fn route_key(key: &Key, mut handle: impl FnMut() -> Outcome) -> Outcome {
    if ROUTE.with(Cell::get) != Pass::Direct || TypeAhead::char_of(key).is_none() {
        return handle();
    }
    let first = {
        let _pass = PassGuard::enter(Pass::Shortcuts);
        YIELDED.with(|y| y.set(false));
        handle()
    };
    if !YIELDED.with(|y| y.replace(false)) || first == Outcome::Changed {
        return first;
    }
    let _pass = PassGuard::enter(Pass::TypeAhead);
    handle().or(first)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum NavProfile {
    #[default]
    Desktop,

    Vim,
}

thread_local! {
    static PROFILE: Cell<NavProfile> = const { Cell::new(NavProfile::Desktop) };
}

impl NavProfile {
    pub const fn nav(self) -> NavKeys {
        match self {
            NavProfile::Desktop => NavKeys::DESKTOP,
            NavProfile::Vim => NavKeys::VIM,
        }
    }

    pub const fn type_ahead(self) -> bool {
        matches!(self, NavProfile::Desktop)
    }

    pub fn install(self) {
        PROFILE.with(|p| p.set(self));
    }

    pub fn current() -> Self {
        PROFILE.with(Cell::get)
    }
}

pub trait Navigable: RowSource {
    fn cursor(&self) -> Cursor;
}

/// Identity and revisions of a keyed provider.
///
/// `instance` identifies a provider lifetime, never a reusable label: a new
/// store gets a new instance even when its counters equal an older store's.
/// `data` changes when authoritative membership, payload or enablement
/// changes. `view` changes when the visible order, filter or match metadata
/// changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceStamp {
    pub instance: ItemKey,
    pub data: u64,
    pub view: u64,
}

/// A [`RowSource`] whose rows have stable logical identity.
///
/// The inherited `len`, `key`, `label` and `enabled` describe the visible
/// ordered projection. `source_len` and `source_key` enumerate every
/// authoritative key, including records a filter currently hides. Keys must
/// not be [`ItemKey::Index`], must be unique, and every visible key must be
/// authoritative; `KeyedState::reconcile` rejects a source that is not.
pub trait KeyedRowSource: RowSource {
    fn stamp(&self) -> SourceStamp;
    fn source_len(&self) -> usize;
    fn source_key(&self, source_index: usize) -> ItemKey;
}

use std::ops::{BitAnd, BitOr, BitOrAssign, Not, Sub};

use super::event::Outcome;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, PartialOrd, Ord, Hash)]
pub enum Flow {
    #[default]
    Ignored,

    Consumed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, PartialOrd, Ord, Hash)]
pub enum Invalidate {
    #[default]
    None,

    Paint,

    Layout,
}

/// The ordered semantic actions one input produced.
///
/// Zero or one action is stored inline; a second action allocates. The
/// unrelated [`crate::core::keymap::Actions`] trait is a key-map concept; import
/// this type under another name (`ResponseActions`) when a module needs both.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Actions<A> {
    None,
    One(A),
    Many(Vec<A>),
}

impl<A> Default for Actions<A> {
    fn default() -> Self {
        Self::None
    }
}

impl<A> Actions<A> {
    pub fn len(&self) -> usize {
        match self {
            Self::None => 0,
            Self::One(_) => 1,
            Self::Many(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push(&mut self, next: A) {
        let previous = std::mem::take(self);
        *self = match previous {
            Self::None => Self::One(next),
            Self::One(first) => Self::Many(vec![first, next]),
            Self::Many(mut values) => {
                values.push(next);
                Self::Many(values)
            }
        };
    }

    pub fn append(&mut self, other: Self) {
        for action in other {
            self.push(action);
        }
    }

    /// Borrowed, ordered view of the batch.
    pub fn iter(&self) -> ActionsIter<'_, A> {
        match self {
            Self::None => ActionsIter::Small(None.into_iter()),
            Self::One(value) => ActionsIter::Small(Some(value).into_iter()),
            Self::Many(values) => ActionsIter::Many(values.iter()),
        }
    }
}

impl<A> From<Option<A>> for Actions<A> {
    fn from(action: Option<A>) -> Self {
        action.map_or(Self::None, Self::One)
    }
}

impl<A> Extend<A> for Actions<A> {
    fn extend<I: IntoIterator<Item = A>>(&mut self, iter: I) {
        for action in iter {
            self.push(action);
        }
    }
}

impl<A> FromIterator<A> for Actions<A> {
    fn from_iter<I: IntoIterator<Item = A>>(iter: I) -> Self {
        let mut actions = Self::None;
        actions.extend(iter);
        actions
    }
}

/// Owning iterator over [`Actions`].
pub enum ActionsIntoIter<A> {
    Small(std::option::IntoIter<A>),
    Many(std::vec::IntoIter<A>),
}

impl<A> Iterator for ActionsIntoIter<A> {
    type Item = A;

    fn next(&mut self) -> Option<A> {
        match self {
            Self::Small(it) => it.next(),
            Self::Many(it) => it.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Small(it) => it.size_hint(),
            Self::Many(it) => it.size_hint(),
        }
    }
}

impl<A> ExactSizeIterator for ActionsIntoIter<A> {}

impl<A> IntoIterator for Actions<A> {
    type Item = A;
    type IntoIter = ActionsIntoIter<A>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::None => ActionsIntoIter::Small(None.into_iter()),
            Self::One(value) => ActionsIntoIter::Small(Some(value).into_iter()),
            Self::Many(values) => ActionsIntoIter::Many(values.into_iter()),
        }
    }
}

/// Borrowed iterator over [`Actions`].
pub enum ActionsIter<'a, A> {
    Small(std::option::IntoIter<&'a A>),
    Many(std::slice::Iter<'a, A>),
}

impl<'a, A> Iterator for ActionsIter<'a, A> {
    type Item = &'a A;

    fn next(&mut self) -> Option<&'a A> {
        match self {
            Self::Small(it) => it.next(),
            Self::Many(it) => it.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Small(it) => it.size_hint(),
            Self::Many(it) => it.size_hint(),
        }
    }
}

impl<A> ExactSizeIterator for ActionsIter<'_, A> {}

#[must_use = "a Response carries the consumed/repaint answer and its actions; fold it with `|`, drain it or read it"]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response<A = ()> {
    flow: Flow,
    invalidate: Invalidate,
    actions: Actions<A>,
}

impl<A> Default for Response<A> {
    fn default() -> Self {
        Self::ignored()
    }
}

impl<A> Response<A> {
    pub const fn ignored() -> Self {
        Self {
            flow: Flow::Ignored,
            invalidate: Invalidate::None,
            actions: Actions::None,
        }
    }

    pub const fn consumed() -> Self {
        Self {
            flow: Flow::Consumed,
            invalidate: Invalidate::None,
            actions: Actions::None,
        }
    }

    pub const fn changed() -> Self {
        Self {
            flow: Flow::Consumed,
            invalidate: Invalidate::Paint,
            actions: Actions::None,
        }
    }

    pub const fn emit(action: A) -> Self {
        Self {
            flow: Flow::Consumed,
            invalidate: Invalidate::Paint,
            actions: Actions::One(action),
        }
    }

    pub fn repaint(mut self) -> Self {
        self.invalidate = self.invalidate.max(Invalidate::Paint);
        self
    }

    pub const fn relayout(mut self) -> Self {
        self.invalidate = Invalidate::Layout;
        self
    }

    pub const fn no_repaint(mut self) -> Self {
        self.invalidate = Invalidate::None;
        self
    }

    pub const fn flow(&self) -> Flow {
        self.flow
    }

    pub fn is_consumed(&self) -> bool {
        self.flow == Flow::Consumed
    }

    pub const fn invalidate(&self) -> Invalidate {
        self.invalidate
    }

    pub fn is_changed(&self) -> bool {
        self.invalidate >= Invalidate::Paint
    }

    pub fn outcome(&self) -> Outcome {
        match (self.flow, self.is_changed()) {
            (_, true) => Outcome::Changed,
            (Flow::Consumed, false) => Outcome::Consumed,
            (Flow::Ignored, false) => Outcome::Ignored,
        }
    }

    /// Borrowed, ordered view of the pending actions; does not take them.
    pub fn actions(&self) -> ActionsIter<'_, A> {
        self.actions.iter()
    }

    /// Whether the response carries at least one action.
    pub fn has_actions(&self) -> bool {
        !self.actions.is_empty()
    }

    pub fn into_actions(self) -> Actions<A> {
        self.actions
    }

    pub fn take_actions(&mut self) -> Actions<A> {
        std::mem::take(&mut self.actions)
    }

    /// Legacy decomposition. `Outcome` has no layout case, so a `Layout`
    /// invalidation collapses to `Changed`; use [`Response::into_raw_parts`]
    /// when the invalidation level must survive.
    pub fn into_parts(self) -> (Outcome, Actions<A>) {
        (self.outcome(), self.actions)
    }

    /// Flag-preserving decomposition: flow, invalidation level and actions.
    pub fn into_raw_parts(self) -> (Flow, Invalidate, Actions<A>) {
        (self.flow, self.invalidate, self.actions)
    }

    /// Appends one action after any already pending.
    pub fn push_action(&mut self, action: A) {
        self.flow = Flow::Consumed;
        self.invalidate = self.invalidate.max(Invalidate::Paint);
        self.actions.push(action);
    }

    /// Folds `other` after `self`: flow and invalidation take the maximum and
    /// the actions keep their order.
    pub fn merge(mut self, other: Self) -> Self {
        self.flow = self.flow.max(other.flow);
        self.invalidate = self.invalidate.max(other.invalidate);
        self.actions.append(other.actions);
        self
    }

    pub fn map_actions<B>(self, mut f: impl FnMut(A) -> B) -> Response<B> {
        let mut mapped = Actions::None;
        for action in self.actions {
            mapped.push(f(action));
        }
        Response {
            flow: self.flow,
            invalidate: self.invalidate,
            actions: mapped,
        }
    }

    pub fn on_actions(self, mut f: impl FnMut(A)) -> Response<()> {
        for action in self.actions {
            f(action);
        }
        Response {
            flow: self.flow,
            invalidate: self.invalidate,
            actions: Actions::None,
        }
    }

    /// For a boundary whose contract proves at most one action. A batch of
    /// two or more comes back unchanged in `Err`.
    pub fn try_into_single(self) -> Result<(Outcome, Option<A>), Self> {
        if self.actions.len() > 1 {
            return Err(self);
        }
        let outcome = self.outcome();
        Ok((outcome, self.actions.into_iter().next()))
    }

    /// Deliberately discards the actions, keeping flow and invalidation.
    pub fn erase(self) -> Response<()> {
        Response {
            flow: self.flow,
            invalidate: self.invalidate,
            actions: Actions::None,
        }
    }
}

impl Response<()> {
    pub fn lift<B>(self) -> Response<B> {
        Response {
            flow: self.flow,
            invalidate: self.invalidate,
            actions: Actions::None,
        }
    }
}

impl<A> From<Outcome> for Response<A> {
    fn from(o: Outcome) -> Self {
        match o {
            Outcome::Ignored => Self::ignored(),
            Outcome::Consumed => Self::consumed(),
            Outcome::Changed => Self::changed(),
        }
    }
}

impl<A> From<(Outcome, Option<A>)> for Response<A> {
    fn from((o, a): (Outcome, Option<A>)) -> Self {
        let mut r = Self::from(o);
        if let Some(a) = a {
            r.push_action(a);
        }
        r
    }
}

impl<A> BitOr for Response<A> {
    type Output = Response<A>;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.merge(rhs)
    }
}

impl<A> BitOrAssign for Response<A> {
    fn bitor_assign(&mut self, rhs: Self) {
        self.flow = self.flow.max(rhs.flow);
        self.invalidate = self.invalidate.max(rhs.invalidate);
        self.actions.append(rhs.actions);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StateFlags(u16);

impl StateFlags {
    pub const FOCUSED: Self = Self(1 << 0);

    pub const FOCUS_VISIBLE: Self = Self(1 << 1);

    pub const HOVERED: Self = Self(1 << 2);

    pub const PRESSED: Self = Self(1 << 3);

    pub const SELECTED: Self = Self(1 << 4);

    pub const ACTIVE: Self = Self(1 << 5);

    pub const CHECKED: Self = Self(1 << 6);

    pub const DISABLED: Self = Self(1 << 7);

    pub const READ_ONLY: Self = Self(1 << 8);

    pub const ERROR: Self = Self(1 << 9);

    pub const WARNING: Self = Self(1 << 10);

    pub const BUSY: Self = Self(1 << 11);

    pub const EDITING: Self = Self(1 << 12);

    pub const DIRTY: Self = Self(1 << 13);

    pub const EXPANDED: Self = Self(1 << 14);

    pub const LOADING: Self = Self(1 << 15);

    const NAMES: [(Self, &'static str); 16] = [
        (Self::FOCUSED, "FOCUSED"),
        (Self::FOCUS_VISIBLE, "FOCUS_VISIBLE"),
        (Self::HOVERED, "HOVERED"),
        (Self::PRESSED, "PRESSED"),
        (Self::SELECTED, "SELECTED"),
        (Self::ACTIVE, "ACTIVE"),
        (Self::CHECKED, "CHECKED"),
        (Self::DISABLED, "DISABLED"),
        (Self::READ_ONLY, "READ_ONLY"),
        (Self::ERROR, "ERROR"),
        (Self::WARNING, "WARNING"),
        (Self::BUSY, "BUSY"),
        (Self::EDITING, "EDITING"),
        (Self::DIRTY, "DIRTY"),
        (Self::EXPANDED, "EXPANDED"),
        (Self::LOADING, "LOADING"),
    ];

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    pub fn set(&mut self, other: Self, on: bool) {
        if on {
            self.insert(other);
        } else {
            self.remove(other);
        }
    }

    pub const fn with(self, other: Self, on: bool) -> Self {
        if on {
            Self(self.0 | other.0)
        } else {
            Self(self.0 & !other.0)
        }
    }
}

impl BitOr for StateFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for StateFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for StateFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl Sub for StateFlags {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 & !rhs.0)
    }
}

impl Not for StateFlags {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

impl std::fmt::Debug for StateFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            return f.write_str("StateFlags(empty)");
        }
        f.write_str("StateFlags(")?;
        let mut first = true;
        for (flag, name) in Self::NAMES {
            if self.contains(flag) {
                if !first {
                    f.write_str(" | ")?;
                }
                f.write_str(name)?;
                first = false;
            }
        }
        f.write_str(")")
    }
}

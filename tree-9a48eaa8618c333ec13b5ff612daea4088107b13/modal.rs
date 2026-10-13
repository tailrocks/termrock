// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Crate-private modal helpers (Break D).
//!
//! Domain-neutral overlay z-order / Esc / outside-click live on
//! [`crate::interaction::OverlayStack`]. This module keeps:
//! - [`render_backdrop`] — paint helper when stack requests a wash
//! - [`ModalStack`] — crate-private legacy container (not public API)

#![allow(dead_code)] // legacy / unit-test surface
use ratatui_core::layout::Rect;
use ratatui_core::terminal::Frame;

use crate::style::DesignSystem;
use crate::widgets::Backdrop;

/// Stack of modal dialogs with "Esc walks back one step" semantics.
///
/// The active modal lives in `current`; every sub-modal push moves the previous
/// active modal into `parents`. `pop` restores exactly one parent, while
/// `clear_chain` closes the whole flow after a terminal commit/cancel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModalStack<M> {
    current: Option<M>,
    parents: Vec<M>,
}

impl<M> Default for ModalStack<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> ModalStack<M> {
    #[must_use]
    /// Creates an empty modal stack.
    pub const fn new() -> Self {
        Self {
            current: None,
            parents: Vec::new(),
        }
    }

    #[must_use]
    /// Returns the currently active modal.
    pub const fn current(&self) -> Option<&M> {
        self.current.as_ref()
    }

    #[must_use]
    /// Returns mutable access to the currently active modal.
    pub fn current_mut(&mut self) -> Option<&mut M> {
        self.current.as_mut()
    }

    #[must_use]
    /// Returns the suspended parent-modal chain.
    pub fn parents(&self) -> &[M] {
        &self.parents
    }

    #[must_use]
    /// Returns whether the stack has an active modal.
    pub const fn is_open(&self) -> bool {
        self.current.is_some()
    }

    #[must_use]
    /// Returns whether closing the current modal can restore a parent.
    pub fn has_parent(&self) -> bool {
        !self.parents.is_empty()
    }

    #[must_use]
    /// Returns the active modal depth, including the current modal.
    pub fn depth(&self) -> usize {
        self.parents.len() + usize::from(self.current.is_some())
    }

    /// Open a new root modal and discard any existing parent chain.
    pub fn open(&mut self, modal: M) {
        self.current = Some(modal);
        self.parents.clear();
    }

    /// Open a child modal, preserving the existing active modal as the parent.
    pub fn open_sub(&mut self, child: M) {
        if let Some(parent) = self.current.take() {
            self.parents.push(parent);
        } else {
            self.parents.clear();
        }
        self.current = Some(child);
    }

    /// Close the active modal and restore one parent, if any.
    pub fn pop(&mut self) {
        self.current = self.parents.pop();
    }

    /// Close the active modal and every saved parent.
    pub fn clear_chain(&mut self) {
        self.current = None;
        self.parents.clear();
    }
}

/// Render the themed backdrop across the whole overlay layer.
///
/// Call this with the layer rect (usually the frame area) before drawing the
/// modal, whenever the overlay stack asks for
/// [`BackdropPolicy::Dim`](crate::interaction::BackdropPolicy::Dim): the widget
/// itself only ever receives its own rect, so the dim belongs to the host that
/// owns the layer.
pub fn render_backdrop(frame: &mut Frame<'_>, full_area: Rect, tokens: &DesignSystem) {
    frame.render_widget(Backdrop::from_tokens(tokens), full_area);
}

/// Classify a mouse click relative to an open modal rect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalClickResult {
    /// Click was outside the modal rect — dismiss the modal (same as Esc).
    OutsideDismiss,
    /// Click was inside the modal rect on the given col/row — let the modal handle it.
    InsideHit,
}

/// Classify a click at `(col, row)` against `modal_rect`.
///
/// Returns:
/// - `OutsideDismiss` if the click is outside the modal (dismiss the modal).
/// - `InsideHit` if inside (caller decides what to do within the modal).
#[must_use]
pub(crate) fn classify_click(modal_rect: Rect, col: u16, row: u16) -> ModalClickResult {
    if modal_rect.contains(ratatui_core::layout::Position { x: col, y: row }) {
        ModalClickResult::InsideHit
    } else {
        ModalClickResult::OutsideDismiss
    }
}

#[cfg(test)]
mod tests;

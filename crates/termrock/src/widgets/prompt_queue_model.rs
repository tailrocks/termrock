// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Prompt queue model** — product-neutral queued-message identity types.
//!
//! Used by [`crate::widgets::PromptComposer`] (FIFO chrome) and the
//! `termrock::patterns::PromptQueue` management recipe. Domain hosts own
//! persistence and drain policy; these types carry no I/O.
use super::SemanticStatus;

/// Lifecycle of one queued prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PromptQueueStatus {
    /// Waiting behind active work.
    #[default]
    Queued,
    /// Host is currently sending this entry.
    Sending,
    /// Blocked (busy gate, permission, connection).
    Blocked,
    /// Send failed; held for user edit (no auto-drain).
    Failed,
    /// Cancelled by user/host; held until removed.
    Cancelled,
    /// Successfully sent (may remain briefly for chrome).
    Sent,
}

impl PromptQueueStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Sending => "sending",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Sent => "sent",
        }
    }

    /// Letter (colorless).
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Queued => 'Q',
            Self::Sending => 'S',
            Self::Blocked => 'B',
            Self::Failed => 'F',
            Self::Cancelled => 'C',
            Self::Sent => '+',
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            return match self {
                Self::Queued => "o",
                Self::Sending => ">",
                Self::Blocked => "!",
                Self::Failed => "x",
                Self::Cancelled => "-",
                Self::Sent => "+",
            };
        }
        match self {
            Self::Queued => "○",
            Self::Sending => "◎",
            Self::Blocked => "⚠",
            Self::Failed => "✗",
            Self::Cancelled => "–",
            Self::Sent => "✓",
        }
    }

    /// Shared lifecycle projection for recipe-owned status paint.
    #[must_use]
    pub const fn semantic(self) -> SemanticStatus {
        match self {
            Self::Queued => SemanticStatus::Queued,
            Self::Sending => SemanticStatus::Running,
            Self::Blocked => SemanticStatus::Waiting,
            Self::Failed => SemanticStatus::Failed,
            Self::Cancelled => SemanticStatus::Paused,
            Self::Sent => SemanticStatus::Success,
        }
    }

    /// Whether entry is still “in queue” for depth chrome.
    #[must_use]
    pub const fn counts_as_pending(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Sending | Self::Blocked | Self::Failed | Self::Cancelled
        )
    }

    /// Whether user can reorder this row.
    #[must_use]
    pub const fn can_reorder(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Blocked | Self::Failed | Self::Cancelled
        )
    }

    /// Whether entry can be sent next.
    #[must_use]
    pub const fn can_send(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Failed | Self::Blocked | Self::Cancelled
        )
    }
}

/// Preserved attachment or mention identity (no payload/body ownership).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptQueueRef {
    /// Stable id (matches ComposerChip / AttachmentItem).
    pub id: String,
    /// Kind tag (`file`, `paste`, `mention`, `image`, …).
    pub kind: String,
    /// Display label.
    pub label: String,
}

impl PromptQueueRef {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, kind: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
            label: label.into(),
        }
    }

    /// File attachment ref.
    #[must_use]
    pub fn file(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, "file", label)
    }

    /// Paste chip ref.
    #[must_use]
    pub fn paste(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, "paste", label)
    }

    /// Mention ref.
    #[must_use]
    pub fn mention(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, "mention", label)
    }
}

/// One queued user message.
///
/// Host owns persistence and when to drain. Fail/cancel **must not** be
/// auto-drained by TermRock (KD-29).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptQueueItem {
    /// Stable queue entry id.
    pub id: String,
    /// Prompt text snapshot.
    pub text: String,
    /// Attachment identities (preserved across edit/reorder).
    pub attachments: Vec<PromptQueueRef>,
    /// Mention identities.
    pub mentions: Vec<PromptQueueRef>,
    /// Status.
    pub status: PromptQueueStatus,
    /// Blocked reason (permission, offline, busy policy).
    pub blocked_reason: Option<String>,
    /// Failure message.
    pub error: Option<String>,
    /// Optional recency / enqueue label.
    pub when: Option<String>,
}

impl PromptQueueItem {
    /// Queued text-only entry.
    #[must_use]
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            attachments: Vec::new(),
            mentions: Vec::new(),
            status: PromptQueueStatus::Queued,
            blocked_reason: None,
            error: None,
            when: None,
        }
    }

    /// From composer-style chip id list (kind unknown → `chip`).
    #[must_use]
    pub fn from_text_and_chip_ids(
        id: impl Into<String>,
        text: impl Into<String>,
        chip_ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let attachments = chip_ids
            .into_iter()
            .map(|c| {
                let id = c.into();
                PromptQueueRef::new(id.clone(), "chip", id)
            })
            .collect();
        Self {
            id: id.into(),
            text: text.into(),
            attachments,
            mentions: Vec::new(),
            status: PromptQueueStatus::Queued,
            blocked_reason: None,
            error: None,
            when: None,
        }
    }

    /// Chip ids only (composer bridge).
    #[must_use]
    pub fn chip_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.attachments.iter().map(|a| a.id.clone()).collect();
        ids.extend(self.mentions.iter().map(|m| m.id.clone()));
        ids
    }

    /// Attachments.
    #[must_use]
    pub fn attachments(mut self, a: Vec<PromptQueueRef>) -> Self {
        self.attachments = a;
        self
    }

    /// Mentions.
    #[must_use]
    pub fn mentions(mut self, m: Vec<PromptQueueRef>) -> Self {
        self.mentions = m;
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: PromptQueueStatus) -> Self {
        self.status = s;
        self
    }

    /// Blocked.
    #[must_use]
    pub fn blocked(mut self, reason: impl Into<String>) -> Self {
        self.status = PromptQueueStatus::Blocked;
        self.blocked_reason = Some(reason.into());
        self
    }

    /// Failed.
    #[must_use]
    pub fn failed(mut self, err: impl Into<String>) -> Self {
        self.status = PromptQueueStatus::Failed;
        self.error = Some(err.into());
        self
    }

    /// When label.
    #[must_use]
    pub fn when(mut self, w: impl Into<String>) -> Self {
        self.when = Some(w.into());
        self
    }

    /// One-line preview.
    #[must_use]
    pub fn preview(&self, max_cols: usize) -> String {
        let t = self.text.replace('\n', " ");
        let t = t.trim();
        crate::text::truncate_cols(t, max_cols, "…").into_owned()
    }
}

/// Agent execution context for chrome (host-projected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AgentBusyState {
    /// Agent idle — queue can send immediately.
    #[default]
    Idle,
    /// Agent running a turn.
    Busy,
    /// Waiting on user (permission / question).
    WaitingUser,
    /// Stopping / interrupting.
    Interrupting,
}

impl AgentBusyState {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Busy => "busy",
            Self::WaitingUser => "waiting_user",
            Self::Interrupting => "interrupting",
        }
    }

    /// Human label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Busy => "agent busy",
            Self::WaitingUser => "waiting on you",
            Self::Interrupting => "interrupting…",
        }
    }

    /// Shared lifecycle projection for queue chrome.
    #[must_use]
    pub const fn semantic(self) -> SemanticStatus {
        match self {
            Self::Idle => SemanticStatus::Idle,
            Self::Busy | Self::Interrupting => SemanticStatus::Running,
            Self::WaitingUser => SemanticStatus::Waiting,
        }
    }

    /// Whether new work is blocked behind active run.
    #[must_use]
    pub const fn is_busy(self) -> bool {
        !matches!(self, Self::Idle)
    }
}

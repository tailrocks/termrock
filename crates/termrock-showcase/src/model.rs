// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! What the showcase knows: the conversation, the runs, the subagents.
//!
//! All of it consumer-owned, which is the point — TermRock paints projections
//! of this, and none of these types leak into the library.

/// One block in the conversation, as the host stores it.
///
/// The widget takes borrowed `&[&str]` lines, so the model keeps owned strings
/// and projects them per frame.
#[derive(Debug, Clone)]
pub struct Message {
    /// Stable id, used as the transcript block id.
    pub id: String,
    /// Who said it.
    pub role: Role,
    /// Body lines.
    pub lines: Vec<String>,
    /// Whether the block is still being written.
    pub streaming: bool,
}

/// Who produced a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The operator.
    User,
    /// The agent.
    Assistant,
    /// The system, for failures worth keeping in the thread.
    ///
    /// Tool runs are not a role: they are their own model, projected as tool
    /// blocks, so a run keeps its output and its exit code.
    System,
}

impl Message {
    /// A finished message.
    #[must_use]
    pub fn new(id: impl Into<String>, role: Role, line: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role,
            lines: vec![line.into()],
            streaming: false,
        }
    }

    /// An assistant message still being streamed into.
    #[must_use]
    pub fn streaming(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role: Role::Assistant,
            lines: vec![String::new()],
            streaming: true,
        }
    }

    /// Appends streamed text, wrapping words onto new lines at `width`.
    ///
    /// The transcript wraps for paint; this keeps the *model* readable so a
    /// recording of it is legible without a terminal.
    pub fn push_delta(&mut self, text: &str, width: usize) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        for word in text.split_inclusive(' ') {
            let last = self.lines.last_mut().expect("a line exists");
            if !last.is_empty() && last.chars().count() + word.chars().count() > width {
                self.lines.push(word.trim_start().to_string());
            } else {
                last.push_str(word);
            }
        }
    }
}

/// A tool run the operator can watch.
#[derive(Debug, Clone)]
pub struct ToolRun {
    /// Stable id.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// The command or path, verbatim.
    pub detail: String,
    /// Output lines so far.
    pub output: Vec<String>,
    /// `None` while running.
    pub ok: Option<bool>,
}

/// A subagent working in parallel.
#[derive(Debug, Clone)]
pub struct Subagent {
    /// Stable id.
    pub id: String,
    /// What it is doing.
    pub title: String,
    /// Whether it has reported back.
    pub done: bool,
}

/// One conversation with its runs.
#[derive(Debug, Clone, Default)]
pub struct Session {
    /// Display title.
    pub title: String,
    /// Conversation blocks, oldest first.
    pub messages: Vec<Message>,
    /// Tool runs, newest last.
    pub runs: Vec<ToolRun>,
    /// Subagents spawned this session.
    pub subagents: Vec<Subagent>,
}

impl Session {
    /// A named, empty session.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Self::default()
        }
    }

    /// The streaming assistant block, if one is open.
    pub fn streaming_mut(&mut self) -> Option<&mut Message> {
        self.messages.iter_mut().rev().find(|m| m.streaming)
    }

    /// Closes any open streaming block.
    pub fn finish_streaming(&mut self) {
        if let Some(message) = self.streaming_mut() {
            message.streaming = false;
        }
    }

    /// The run with `id`, if it is still known.
    pub fn run_mut(&mut self, id: &str) -> Option<&mut ToolRun> {
        self.runs.iter_mut().find(|run| run.id == id)
    }
}

/// A file in the fake workspace tree.
#[derive(Debug, Clone, Copy)]
pub struct FileEntry {
    /// Display path.
    pub path: &'static str,
    /// Nesting depth.
    pub depth: u16,
    /// Whether it is a directory.
    pub directory: bool,
}

/// The workspace the demo pretends to be working in.
///
/// Fake on purpose: the showcase performs no filesystem I/O, so a demo can be
/// run anywhere without surprising anyone's disk (SKD-2).
#[must_use]
pub fn demo_files() -> Vec<FileEntry> {
    vec![
        FileEntry {
            path: "crates",
            depth: 0,
            directory: true,
        },
        FileEntry {
            path: "termrock",
            depth: 1,
            directory: true,
        },
        FileEntry {
            path: "widgets",
            depth: 2,
            directory: true,
        },
        FileEntry {
            path: "list.rs",
            depth: 3,
            directory: false,
        },
        FileEntry {
            path: "panel.rs",
            depth: 3,
            directory: false,
        },
        FileEntry {
            path: "patterns",
            depth: 2,
            directory: true,
        },
        FileEntry {
            path: "agent_workbench.rs",
            depth: 3,
            directory: false,
        },
        FileEntry {
            path: "termrock-showcase",
            depth: 1,
            directory: true,
        },
        FileEntry {
            path: "main.rs",
            depth: 2,
            directory: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_text_wraps_into_readable_lines() {
        let mut message = Message::streaming("a");
        message.push_delta("the quick brown fox jumps over the lazy dog ", 16);
        assert!(message.lines.len() > 1, "{:?}", message.lines);
        assert!(
            message.lines.iter().all(|line| line.chars().count() <= 20),
            "{:?}",
            message.lines
        );
    }

    #[test]
    fn a_session_has_at_most_one_open_stream() {
        let mut session = Session::new("demo");
        session.messages.push(Message::new("u1", Role::User, "hi"));
        session.messages.push(Message::streaming("a1"));
        assert!(session.streaming_mut().is_some());
        session.finish_streaming();
        assert!(session.streaming_mut().is_none());
    }
}

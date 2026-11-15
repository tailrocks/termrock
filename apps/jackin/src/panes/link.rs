//! What a terminal pane needs from the thing that runs its program: a link it
//! sends commands into and drains events out of, and a source that opens one.

use crate::panes::PaneRequest;

/// A grid size in rows and columns: what the program is told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneSize {
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneCommand {
    /// Bytes the operator typed or pasted.
    Input(Vec<u8>),
    /// Bytes the terminal answers to a query of the program.
    Reply(Vec<u8>),
    /// Repaint the screen: the link resizes by one row and back.
    Redraw,
    Detach,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sent {
    Queued,
    /// The queue is full; the command was not taken.
    Full,
    Closed,
}

/// Whether this client may type into the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lease {
    Owner,
    ReadOnly,
}

/// What the operator can do about a failed attach besides retrying with
/// Enter. The words come from `PaneWords`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FailNext {
    #[default]
    None,
    RunDoctor,
}

/// Why an attach failed: the host's own text, and the step that may help.
/// An empty `reason` means the stream closed before the attach completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkFault {
    pub reason: String,
    pub next: FailNext,
}

impl LinkFault {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            next: FailNext::None,
        }
    }

    pub fn then(mut self, next: FailNext) -> Self {
        self.next = next;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndKind {
    /// The pane was detached, by the operator or by the other side.
    Detached,
    /// The stream closed without a word.
    StreamClosed,
    /// The other side refused the attach.
    Refused(LinkFault),
    Failed(LinkFault),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkEvent {
    /// The attach completed: the replay of the screen follows.
    Attached,
    Lease(Lease),
    Output(Vec<u8>),
    /// A line the server wants the operator to read.
    Notice(String),
    /// The program ended with this exit code.
    Exited(Option<i32>),
    Ended(EndKind),
}

/// One attach to a program. A link is owned by one pane; dropping it detaches
/// a main process and closes a shell.
pub trait PaneLink: Send + std::fmt::Debug {
    fn send(&self, command: PaneCommand) -> Sent;

    /// The grid the program should have now. The latest size wins: a burst of
    /// sizes sends the last one.
    fn set_size(&self, size: PaneSize);

    /// The events that arrived since the last call, at most `output_budget`
    /// bytes of output. `now_ms` is the console's clock, so a scripted link
    /// plays on the same time as the tests.
    ///
    /// A zero budget takes no output bytes, but events before the first
    /// blocked output still return. An output larger than the budget is split
    /// into ordered pieces across calls. Events behind a blocked or partly
    /// sent output wait for the rest of it, so the order of events holds.
    fn drain(&self, now_ms: i64, output_budget: usize) -> Vec<LinkEvent>;
}

/// Opens links lazily, at the first size a pane can use. It never blocks: a
/// failure to start is an `Err` with the reason the operator reads.
pub trait LinkSource: Send + Sync + std::fmt::Debug {
    fn open(&self, request: &PaneRequest, size: PaneSize) -> Result<Box<dyn PaneLink>, LinkFault>;

    /// Starts the program again after it exited, as `jackin attach --restart`
    /// does, and opens a link to the new run. Never blocks, like `open`.
    fn restart(
        &self,
        request: &PaneRequest,
        size: PaneSize,
    ) -> Result<Box<dyn PaneLink>, LinkFault>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetachCause {
    Operator,
    GatewayLost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkStatus {
    /// No link yet: the pane is waiting for room, or for a retry.
    Pending,
    /// A link exists and the replay has not finished.
    Attaching,
    Attached(Lease),
    Detached(DetachCause),
    Exited(Option<i32>),
    /// The attach failed.
    Failed(LinkFault),
}

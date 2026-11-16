//! A scripted link for a terminal pane: it plays a byte stream and the link
//! events around it on the console's clock, so the terminal pane has a program
//! to show without any gateway.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::panes::{
    EndKind, Lease, LinkEvent, LinkFault, LinkSource, PaneCommand, PaneLink, PaneRequest, PaneSize,
    Sent,
};

/// Link events, each due this many milliseconds after the first time the pane
/// drained the link.
#[derive(Debug, Clone)]
pub struct Timed {
    pub after_ms: i64,
    pub event: LinkEvent,
}

#[derive(Debug)]
pub struct SimLink {
    plan: Mutex<Plan>,
}

#[derive(Debug)]
struct Plan {
    started: Option<i64>,
    steps: VecDeque<QueuedStep>,
}

/// A step waiting to be drained. `output_offset` counts the bytes of an
/// Output that earlier drains already returned; it stays zero for controls.
#[derive(Debug)]
struct QueuedStep {
    timed: Timed,
    output_offset: usize,
}

impl SimLink {
    pub fn new(steps: Vec<Timed>) -> Self {
        Self {
            plan: Mutex::new(Plan {
                started: None,
                steps: steps
                    .into_iter()
                    .map(|timed| QueuedStep {
                        timed,
                        output_offset: 0,
                    })
                    .collect(),
            }),
        }
    }
}

impl PaneLink for SimLink {
    fn send(&self, _: PaneCommand) -> Sent {
        Sent::Queued
    }

    fn set_size(&self, _: PaneSize) {}

    fn drain(&self, now_ms: i64, output_budget: usize) -> Vec<LinkEvent> {
        let mut plan = self.plan.lock().expect("plan");
        let started = *plan.started.get_or_insert(now_ms);
        let mut out = Vec::new();
        // Never exceeds `output_budget`, so `output_budget - bytes` cannot underflow.
        let mut bytes = 0;
        while let Some(front) = plan.steps.front_mut() {
            if started.saturating_add(front.timed.after_ms) > now_ms {
                break;
            }
            let LinkEvent::Output(chunk) = &front.timed.event else {
                if let Some(step) = plan.steps.pop_front() {
                    out.push(step.timed.event);
                }
                continue;
            };
            let offset = front.output_offset;
            let take = (chunk.len() - offset).min(output_budget - bytes);
            if !chunk.is_empty() && take == 0 {
                break;
            }
            let piece = chunk[offset..offset + take].to_vec();
            bytes += take;
            if offset + take < chunk.len() {
                // A partly sent output holds back everything behind it, so
                // no control overtakes its remainder.
                front.output_offset = offset + take;
                out.push(LinkEvent::Output(piece));
                break;
            }
            out.push(LinkEvent::Output(piece));
            plan.steps.pop_front();
        }
        out
    }
}

/// What the program on the other end of a pane does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Play {
    /// Attaches, shows the stream and stays.
    Stays(Stream),
    /// Attaches as a second client: the stream, read only.
    Watches(Stream),
    /// Never finishes attaching.
    Hangs,
    /// Attaches, shows the stream, and is detached a second later; the next
    /// attach stays.
    Detaches(Stream),
    /// Attaches, shows the stream, and exits with code 0 a second later; a
    /// restart attaches and stays.
    Exits(Stream),
    /// The attach cannot start.
    Refuses(LinkFault),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Codex,
    Shell,
    Pager,
}

impl Stream {
    pub fn bytes(self) -> Vec<u8> {
        match self {
            Stream::Codex => codex_screen(),
            Stream::Shell => shell_screen(),
            Stream::Pager => pager_screen(),
        }
    }
}

/// Opens the scripted link of one pane. It counts its opens, so a play can
/// behave differently the second time.
#[derive(Debug)]
pub struct SimSource {
    play: Play,
    opens: AtomicUsize,
}

impl SimSource {
    pub fn new(play: Play) -> Self {
        Self {
            play,
            opens: AtomicUsize::new(0),
        }
    }
}

fn at(after_ms: i64, event: LinkEvent) -> Timed {
    Timed { after_ms, event }
}

fn attach(stream: Stream) -> Vec<Timed> {
    vec![
        at(0, LinkEvent::Attached),
        at(0, LinkEvent::Lease(Lease::Owner)),
        at(0, LinkEvent::Output(stream.bytes())),
    ]
}

impl LinkSource for SimSource {
    fn open(&self, _: &PaneRequest, _: PaneSize) -> Result<Box<dyn PaneLink>, LinkFault> {
        let again = self.opens.fetch_add(1, Ordering::Relaxed) > 0;
        let steps = match &self.play {
            Play::Refuses(fault) => return Err(fault.clone()),
            Play::Hangs => vec![],
            Play::Stays(stream) => attach(*stream),
            Play::Watches(stream) => vec![
                at(0, LinkEvent::Attached),
                at(0, LinkEvent::Lease(Lease::ReadOnly)),
                at(
                    0,
                    LinkEvent::Notice("another client controls this pane".into()),
                ),
                at(0, LinkEvent::Output(stream.bytes())),
            ],
            Play::Detaches(stream) => {
                let mut steps = attach(*stream);
                if !again {
                    steps.push(at(1_000, LinkEvent::Ended(EndKind::Detached)));
                }
                steps
            }
            Play::Exits(stream) => {
                let mut steps = attach(*stream);
                steps.push(at(1_000, LinkEvent::Exited(Some(0))));
                steps
            }
        };
        Ok(Box::new(SimLink::new(steps)))
    }

    fn restart(
        &self,
        request: &PaneRequest,
        size: PaneSize,
    ) -> Result<Box<dyn PaneLink>, LinkFault> {
        match &self.play {
            Play::Exits(stream) => Ok(Box::new(SimLink::new(attach(*stream)))),
            _ => self.open(request, size),
        }
    }
}

fn codex_screen() -> Vec<u8> {
    [
        "\x1b]2;payments-platform · gpt-5-codex\x07",
        "\x1b[2J\x1b[H",
        "\x1b[1;38;5;75m>_ Codex\x1b[0m \x1b[2m(v0.9.2)\x1b[0m\r\n",
        "\x1b[2mmodel:\x1b[0m     gpt-5-codex\r\n",
        "\x1b[2mdirectory:\x1b[0m /sandbox/project\r\n\r\n",
        "\x1b[1;36m›\x1b[0m fix the flaky retry test in settlement\r\n\r\n",
        "\x1b[38;2;120;200;120m●\x1b[0m Read \x1b[4msrc/settlement/retry.rs\x1b[0m\r\n",
        "  \x1b[2m日本語のコメントは残します\x1b[0m\r\n",
        "\x1b[38;2;230;180;60m●\x1b[0m Edit \x1b[1m2 files\x1b[0m\r\n",
        "  \x1b[31m- backoff(500)\x1b[0m\r\n",
        "  \x1b[32m+ backoff(attempt * 500)\x1b[0m\r\n\r\n",
        "\x1b[1;36m›\x1b[0m ",
    ]
    .concat()
    .into_bytes()
}

fn shell_screen() -> Vec<u8> {
    [
        "\x1b[2J\x1b[H",
        "\x1b[32mdev@sandbox\x1b[0m:\x1b[34m/sandbox/project\x1b[0m$ ls\r\n",
        "\x1b[1;34mscripts\x1b[0m  \x1b[1;34msrc\x1b[0m  Cargo.toml  README.md\r\n",
        "\x1b[32mdev@sandbox\x1b[0m:\x1b[34m/sandbox/project\x1b[0m$ ",
    ]
    .concat()
    .into_bytes()
}

fn pager_screen() -> Vec<u8> {
    [
        "\x1b[?1049h\x1b[2J\x1b[H\x1b[?25l",
        "\x1b[7m top · 3 tasks · load 0.42 \x1b[0m\r\n",
        "  PID  USER  CMD\r\n",
        " 1204  dev   \x1b[32mcodex\x1b[0m\r\n",
        " 1311  dev   cargo test\r\n",
        "\x1b[24;1H\x1b[2m q quit\x1b[0m",
    ]
    .concat()
    .into_bytes()
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "work-metrics")]
    mod work;

    use std::sync::Arc;

    use ratatui::layout::Rect;

    use super::*;
    use crate::domain::instance::InstanceId;
    use crate::panes::{
        Lease, LinkFault, LinkSource, PaneBody, PaneLink, PanePolicy, PaneRequest, PaneTiming,
        TerminalPane,
    };

    fn output_bytes(events: &[LinkEvent]) -> Vec<u8> {
        events
            .iter()
            .filter_map(|event| match event {
                LinkEvent::Output(bytes) => Some(bytes.as_slice()),
                _ => None,
            })
            .flatten()
            .copied()
            .collect()
    }

    fn output(bytes: &[u8]) -> LinkEvent {
        LinkEvent::Output(bytes.to_vec())
    }

    #[test]
    fn sim_link_first_chunk_obeys_budget() {
        let link = SimLink::new(vec![at(0, output(b"0123456789"))]);
        let parts: Vec<Vec<u8>> = (0..4)
            .map(|_| {
                let events = link.drain(0, 3);
                assert!(output_bytes(&events).len() <= 3, "{events:?}");
                output_bytes(&events)
            })
            .collect();
        let lengths: Vec<usize> = parts.iter().map(Vec::len).collect();
        assert_eq!(lengths, [3, 3, 3, 1]);
        assert_eq!(parts.concat(), b"0123456789");
        assert_eq!(link.drain(0, 3), []);
    }

    #[test]
    fn sim_link_zero_budget_retains_output_in_fifo() {
        let link = SimLink::new(vec![
            at(0, LinkEvent::Attached),
            at(0, LinkEvent::Lease(Lease::Owner)),
            at(0, output(b"abc")),
            at(0, LinkEvent::Notice("after".into())),
        ]);
        assert_eq!(
            link.drain(0, 0),
            [LinkEvent::Attached, LinkEvent::Lease(Lease::Owner)],
            "controls before the blocked output drain at zero budget"
        );
        assert_eq!(link.drain(0, 0), [], "a zero budget takes no output");
        assert_eq!(
            link.drain(0, 10),
            [output(b"abc"), LinkEvent::Notice("after".into())],
            "the notice waits behind the output it follows"
        );
    }

    #[test]
    fn sim_link_budget_applies_across_many_chunks() {
        let link = SimLink::new(vec![
            at(0, output(b"aaaa")),
            at(0, LinkEvent::Notice("1".into())),
            at(0, output(b"bbbbbb")),
            at(0, LinkEvent::Exited(Some(0))),
        ]);
        let first = link.drain(0, 5);
        assert_eq!(
            first,
            [output(b"aaaa"), LinkEvent::Notice("1".into()), output(b"b"),]
        );
        let second = link.drain(0, 5);
        assert_eq!(
            second,
            [output(b"bbbbb"), LinkEvent::Exited(Some(0))],
            "the exit follows once the output it waited behind is complete"
        );
        assert_eq!(link.drain(0, 5), []);
    }

    #[test]
    fn sim_link_partial_utf8_and_escape_stream_is_lossless() {
        let stream = "é日本語 😀\x1b]2;tïtle 😀\x07\x1b[31;1mred\x1b[0m\r\n😀\x1b[2;3Hend"
            .as_bytes()
            .to_vec();
        let link = SimLink::new(vec![at(0, output(&stream))]);
        let mut received = Vec::new();
        for _ in 0..stream.len() {
            let events = link.drain(0, 1);
            let bytes = output_bytes(&events);
            assert!(bytes.len() <= 1, "{events:?}");
            received.extend(bytes);
        }
        assert_eq!(received, stream);
        assert_eq!(link.drain(0, 1), []);

        let mut reference = pane_over(stream.clone(), 64 * 1024);
        reference.tick(0);
        assert!(
            reference.title().is_some(),
            "the OSC title reaches the pane"
        );
        let mut budgeted = pane_over(stream.clone(), 1);
        for now in 0..=stream.len() as i64 {
            budgeted.tick(now);
        }
        assert_eq!(screen(&budgeted), screen(&reference));
    }

    #[test]
    fn sim_link_empty_output_and_large_clock_are_safe() {
        let empty = SimLink::new(vec![
            at(0, output(b"")),
            at(0, LinkEvent::Notice("x".into())),
        ]);
        assert_eq!(
            empty.drain(0, 0),
            [output(b""), LinkEvent::Notice("x".into())],
            "an empty chunk costs no budget and does not stall the queue"
        );

        let late = SimLink::new(vec![at(10, LinkEvent::Notice("late".into()))]);
        assert_eq!(late.drain(i64::MAX - 2, 0), []);
        assert_eq!(
            late.drain(i64::MAX, 0),
            [LinkEvent::Notice("late".into())],
            "the due time saturates instead of overflowing"
        );

        let early = SimLink::new(vec![at(-10, LinkEvent::Notice("early".into()))]);
        assert_eq!(
            early.drain(i64::MIN + 2, 0),
            [LinkEvent::Notice("early".into())]
        );
    }

    /// Hands one prepared link to the pane, which opens it once.
    #[derive(Debug)]
    struct OneLink(Mutex<Option<SimLink>>);

    impl LinkSource for OneLink {
        fn open(&self, _: &PaneRequest, _: PaneSize) -> Result<Box<dyn PaneLink>, LinkFault> {
            let link = self.0.lock().expect("link").take().expect("one open");
            Ok(Box::new(link))
        }

        fn restart(
            &self,
            request: &PaneRequest,
            size: PaneSize,
        ) -> Result<Box<dyn PaneLink>, LinkFault> {
            self.open(request, size)
        }
    }

    /// What the user sees of a pane: every cell's text, the cursor, the title
    /// and the history kept above the screen.
    #[derive(Debug, PartialEq, Eq)]
    struct Screen {
        cells: Vec<(String, bool)>,
        cursor: (u16, u16),
        title: Option<String>,
        history: usize,
    }

    fn screen(pane: &TerminalPane) -> Screen {
        let state = pane.grid().state();
        let view = state.view(0);
        let cells = (0..view.rows())
            .flat_map(|row| (0..view.cols()).map(move |col| (row, col)))
            .filter_map(|(row, col)| view.cell(row, col))
            .map(|cell| (cell.text().to_owned(), cell.is_wide_continuation()))
            .collect();
        Screen {
            cells,
            cursor: view.cursor(),
            title: pane.title().map(str::to_owned),
            history: state.scrollback_len(),
        }
    }

    /// A pane over one attach that shows `stream`, fed at most `feed_budget_bytes`
    /// per tick.
    fn pane_over(stream: Vec<u8>, feed_budget_bytes: usize) -> TerminalPane {
        let instance = InstanceId::simulated("jk-sim-link");
        let request = PaneRequest::new(&instance, None, None);
        let link = SimLink::new(vec![
            at(0, LinkEvent::Attached),
            at(0, LinkEvent::Lease(Lease::Owner)),
            at(0, LinkEvent::Output(stream)),
        ]);
        let mut pane = TerminalPane::new(
            1,
            request,
            Arc::new(OneLink(Mutex::new(Some(link)))),
            PaneTiming::default(),
            0,
        );
        pane.set_policy(&Arc::new(PanePolicy {
            feed_budget_bytes,
            ..PanePolicy::default()
        }));
        pane.sync_size(Rect::new(0, 0, 81, 24));
        pane
    }
}

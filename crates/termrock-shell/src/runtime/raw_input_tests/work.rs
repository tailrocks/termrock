//! Work contracts for the runtime's input, external and output paths (#56).
//!
//! Each test offers a backlog of 1,000, 10,000 or 100,000 items and measures
//! one bounded turn. The counted work is the turn's bound, so it must be equal
//! at every backlog size: a turn that reads or delivers more than its budget,
//! or does the same work twice, changes the exact counts.

use super::*;
use ratatui::layout::Rect;
use ratatui::{Terminal, TerminalOptions, Viewport};
use termrock_metrics::{Metric, WorkDelta, measure};

#[track_caller]
fn assert_exact(work: &WorkDelta, what: &str, expected: &[(Metric, u64)]) {
    if let Err(violations) = work.check_exact(expected) {
        panic!("{what}: {violations}");
    }
}

const SIZES: [usize; 3] = [1_000, 10_000, 100_000];

#[test]
fn work_contract_runtime_raw_filtered_events_yield() {
    for n in SIZES {
        // Only filtered events queued: the turn reads exactly its budget of 16,
        // drops every one, delivers nothing and tells the loop to come back.
        let mut source = Script::releases_then(n, [press('a')]);
        let mut turns = Turns::new(policy(16, Duration::from_secs(60)));
        let (turn, work) = measure(|| turns.turn(&mut source));
        turn.expect("the turn reads");
        assert_exact(
            &work,
            "filtered flood turn",
            &[(Metric::RawEventsRead, 16), (Metric::RawEventsFiltered, 16)],
        );
        assert!(turns.raw_pending, "the turn yields with work queued");
        assert!(turns.app.seen.is_empty());
        assert_eq!(
            source.queue.len(),
            n + 1 - 16,
            "the rest stays queued, n={n}"
        );

        // Filtered and accepted events share one budget of 8: four releases,
        // four inputs.
        let queue = (0..n).flat_map(|i| [release('r'), press(char::from(b'a' + (i % 26) as u8))]);
        let mut source = Script::new(queue);
        let mut turns = Turns::new(policy(8, Duration::from_secs(60)));
        let (turn, work) = measure(|| turns.turn(&mut source));
        turn.expect("the turn reads");
        assert_exact(
            &work,
            "mixed flood turn",
            &[
                (Metric::RawEventsRead, 8),
                (Metric::RawEventsFiltered, 4),
                (Metric::InputDispatches, 4),
            ],
        );
        assert_eq!(turns.app.seen.len(), 4);
        assert!(turns.raw_pending);
    }
}

#[test]
fn work_contract_runtime_startup_discard_is_not_input_work() {
    // The drain of stale events before the first turn is bounded by its own
    // limit and is not in `RawEventsRead`, which counts input turns only.
    let mut source = Script::new((0..50).map(|_| press('a')));
    let (discarded, work) = measure(|| discard_ready(&mut source, 32));
    assert_eq!(discarded.expect("discard"), 32);
    assert_exact(&work, "startup discard", &[]);
}

/// An application that owns an external receiver and ignores what it gets.
struct Flood {
    receiver: ExternalReceiver,
}

impl Application for Flood {
    fn handle(&mut self, _: Input) -> Outcome {
        Outcome::Ignored
    }
    fn render(&mut self, _: &mut Frame) {}
    fn should_quit(&self) -> bool {
        false
    }
    fn tick_interval(&self) -> Duration {
        Duration::from_millis(100)
    }
    fn external(&self) -> Option<&ExternalReceiver> {
        Some(&self.receiver)
    }
}

#[test]
fn work_contract_runtime_flood_services_input() {
    for n in SIZES {
        // External flood: one turn delivers its 3 messages, then the resize
        // and the tick still run.
        let (tx, rx) = external_channel();
        for i in 0..n {
            assert!(tx.send(i as u32));
        }
        let policy = ExternalPolicy {
            max_messages: 3,
            max_duration: Duration::from_secs(60),
        };
        let mut app = Flood {
            receiver: rx.with_policy(policy),
        };
        let mut pending = false;
        let (turn, work) = measure(|| {
            service_turn(
                &mut app,
                &mut pending,
                || Duration::ZERO,
                || Some(Input::Resize(80, 24)),
                || true,
            )
        });
        assert!(turn.ticked && pending, "the tick ran and more is queued");
        assert_exact(
            &work,
            "external flood turn",
            &[(Metric::InputDispatches, 5)],
        );

        // Input flood: every input is ignored, and one drain stops at
        // DRAIN_BUDGET so the loop can reach the tick.
        let mut app = Recorder::default();
        let mut offered = 0usize;
        let (changed, work) = measure(|| {
            drain_ready_inputs(&mut app, || {
                offered += 1;
                Ok((offered <= n).then(|| key('x')))
            })
        });
        assert!(!changed.expect("drain"));
        assert_eq!(offered, DRAIN_BUDGET, "inputs offered, n={n}");
        assert_exact(
            &work,
            "input flood drain",
            &[(Metric::InputDispatches, DRAIN_BUDGET as u64)],
        );
    }
}

/// An application that reports every input as a change.
#[derive(Default)]
struct Eager {
    seen: Vec<Input>,
}

impl Application for Eager {
    fn handle(&mut self, input: Input) -> Outcome {
        self.seen.push(input);
        Outcome::Changed
    }
    fn render(&mut self, _: &mut Frame) {}
    fn should_quit(&self) -> bool {
        false
    }
    fn tick_interval(&self) -> Duration {
        Duration::from_millis(100)
    }
}

fn pointer_move(x: u16) -> Input {
    Input::Mouse(Mouse::new(
        MouseKind::Move,
        ratatui::layout::Position::new(x, 1),
    ))
}

#[test]
fn work_contract_runtime_settle_counts_the_settled_pointer_move() {
    // A burst of pointer moves folds into one dispatch per move the drain
    // takes, and the last move is delivered once more by `settle` so the
    // pointer rests on the right cell. That second delivery is a dispatch.
    for n in [3usize, 30] {
        let mut app = Eager::default();
        let mut pump = InputPump::new(Coalesce {
            max_batch: 64,
            max_latency: Duration::from_secs(60),
            ..Coalesce::default()
        });
        let mut offered = 0usize;
        let (changed, drained) = measure(|| {
            pump.drain(&mut app, || {
                offered += 1;
                Ok((offered <= n).then(|| pointer_move(offered as u16)))
            })
        });
        assert!(changed.expect("drain"));
        assert_exact(
            &drained,
            "pointer burst drain",
            &[(Metric::InputDispatches, n as u64)],
        );

        let (changed, settled) = measure(|| pump.settle(&mut app));
        assert!(changed, "the settled move reaches the application");
        assert_exact(
            &settled,
            "settled pointer move",
            &[(Metric::InputDispatches, 1)],
        );
        assert_eq!(app.seen.len(), n + 1);
        assert_eq!(app.seen.last(), app.seen.get(n - 1), "the last move again");

        let (changed, again) = measure(|| pump.settle(&mut app));
        assert!(!changed, "a move settles once");
        assert_exact(&again, "second settle", &[]);
    }
}

#[cfg(unix)]
#[test]
fn work_contract_runtime_resume_resize_is_one_dispatch() {
    // After a suspend the loop tells the application the new size once, and
    // records the size so the next size check does not repeat it.
    let mut app = Eager::default();
    let mut sizes = SizeWatch::new(Some((80, 24)));
    let ((), work) = measure(|| deliver_resume_resize(&mut app, &mut sizes, (100, 30)));
    assert_exact(&work, "resume resize", &[(Metric::InputDispatches, 1)]);
    assert_eq!(app.seen, [Input::Resize(100, 30)]);
    assert_eq!(sizes.check((100, 30)), None, "the watch knows the new size");
}

/// Draws a screen of one repeated glyph, with an optional marker in the
/// top-left cell.
struct Wall {
    marker: char,
}

impl Application for Wall {
    fn handle(&mut self, _: Input) -> Outcome {
        Outcome::Ignored
    }
    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let buf = frame.buffer_mut();
        for y in 0..area.height {
            for x in 0..area.width {
                buf[(x, y)].set_char('#');
            }
        }
        buf[(0, 0)].set_char(self.marker);
    }
    fn should_quit(&self) -> bool {
        false
    }
    fn tick_interval(&self) -> Duration {
        Duration::from_millis(100)
    }
}

fn wall_terminal(w: u16, h: u16) -> Terminal<MeteredBackend<Vec<u8>>> {
    Terminal::with_options(
        MeteredBackend::new(Vec::new()),
        TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, w, h)),
        },
    )
    .expect("fixed terminal")
}

fn paint_wall(terminal: &mut Terminal<MeteredBackend<Vec<u8>>>, app: &mut Wall) -> WorkDelta {
    let mut dirty = false;
    let ((), work) = measure(|| paint(terminal, app, false, &mut dirty).expect("paint"));
    work
}

#[test]
fn work_contract_runtime_frame_diff_and_bytes_written() {
    // The cells the diff hands over and the bytes the writer accepts for one
    // frame, in a fixed terminal of every cell drawn as '#'. The first frame
    // draws all of them. A frame that changes nothing hands over no cell and
    // still ends with the same 25 bytes of style reset, and a one-cell change
    // adds a cursor move and a glyph to that (32 bytes), whatever the size.
    // `draw_frame`'s synchronized-update markers are written outside `paint`
    // and are not in these counts.
    let cases = [(40u16, 10u16, 486u64), (80, 24, 2_104), (200, 60, 12_436)];
    for (w, h, first_bytes) in cases {
        let cells = u64::from(w) * u64::from(h);
        let mut terminal = wall_terminal(w, h);
        let mut app = Wall { marker: '#' };

        let first = paint_wall(&mut terminal, &mut app);
        assert_exact(
            &first,
            "first frame",
            &[
                (Metric::DiffCellsVisited, cells),
                (Metric::TerminalBytesWritten, first_bytes),
            ],
        );

        let same = paint_wall(&mut terminal, &mut app);
        assert_exact(
            &same,
            "unchanged frame",
            &[(Metric::TerminalBytesWritten, 25)],
        );

        app.marker = '@';
        let one = paint_wall(&mut terminal, &mut app);
        assert_exact(
            &one,
            "one changed cell",
            &[
                (Metric::DiffCellsVisited, 1),
                (Metric::TerminalBytesWritten, 32),
            ],
        );
    }
}

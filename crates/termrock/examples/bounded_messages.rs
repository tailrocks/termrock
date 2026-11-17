//! Background messages through a bounded channel. The runtime hands the app at
//! most `ExternalPolicy::max_messages` per turn within `max_duration`; a
//! producer that finds the queue full gets its message back and decides what
//! to do. This example retries it. `send` is the lossy alternative.
//!
//! Each turn is the runtime's own: `runtime::pump_external` is the function
//! the event loop calls between two reads of terminal input, so the count
//! budget, the time budget and the `exhausted` flag shown here are the ones a
//! real app gets. The example only supplies what a terminal would: a tiny
//! `Application` that records what it handles, and a fake clock that each
//! handler advances, so the time budget needs no sleep.
//!
//! `cargo run -p termrock --example bounded_messages`

use std::cell::Cell;
use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::time::Duration;

use ratatui::Frame;
use termrock::core::event::{External, Input, Outcome};
use termrock::runtime::{
    Application, ExternalPolicy, ExternalReceiver, ExternalSendError, ExternalSender, ExternalTurn,
    bounded_external_channel, pump_external,
};

/// Queue room 4; a turn handles at most 3 messages in under a second.
const CAPACITY: usize = 4;
const COUNT_BUDGET: ExternalPolicy = ExternalPolicy {
    max_messages: 3,
    max_duration: Duration::from_secs(1),
};
/// A count that never binds, so the second per turn does. Each handler
/// charges `HANDLER_COST` to the fake clock.
const TIME_BUDGET: ExternalPolicy = ExternalPolicy {
    max_messages: 100,
    max_duration: Duration::from_secs(1),
};
const HANDLER_COST: Duration = Duration::from_millis(400);

/// A producer that keeps what the queue refused and offers it again later.
struct Producer {
    sender: ExternalSender,
    refused: VecDeque<External>,
}

impl Producer {
    /// Nothing is lost: a refused message waits behind the earlier ones.
    fn send(&mut self, n: u32) {
        self.refused.push_back(External::new(n));
        self.retry();
    }

    fn retry(&mut self) {
        while let Some(message) = self.refused.pop_front() {
            match self.sender.try_send(message) {
                Ok(()) => {}
                Err(ExternalSendError::Full(message) | ExternalSendError::Closed(message)) => {
                    self.refused.push_front(message);
                    return;
                }
            }
        }
    }
}

/// The smallest application the runtime's turn can drive: it owns the
/// receiver, records each `u32` it is handed and charges `cost` to `clock`.
struct Feed {
    receiver: ExternalReceiver,
    handled: Vec<u32>,
    clock: Rc<Cell<Duration>>,
    cost: Duration,
}

impl Application for Feed {
    fn handle(&mut self, input: Input) -> Outcome {
        let Input::External(message) = input else {
            return Outcome::Ignored;
        };
        self.handled.extend(message.downcast_ref::<u32>());
        self.clock.set(self.clock.get() + self.cost);
        Outcome::Changed
    }

    fn render(&mut self, _frame: &mut Frame) {}

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

impl Feed {
    fn new(receiver: ExternalReceiver, cost: Duration) -> Self {
        Self {
            receiver,
            handled: Vec::new(),
            clock: Rc::new(Cell::new(Duration::ZERO)),
            cost,
        }
    }

    /// One runtime turn over the receiver's own policy, on a clock that
    /// starts at zero. Returns what the turn handled and how it ended.
    fn turn(&mut self) -> (Vec<u32>, ExternalTurn) {
        self.clock.set(Duration::ZERO);
        self.handled.clear();
        let clock = Rc::clone(&self.clock);
        let policy = self.receiver.policy();
        let ended = pump_external(self, policy, move || clock.get());
        (std::mem::take(&mut self.handled), ended)
    }
}

/// Every turn until one finds the queue empty, and what the producer held back.
struct Run {
    held_back: usize,
    turns: Vec<(Vec<u32>, ExternalTurn)>,
}

/// Sends `1..=messages` into a queue of `CAPACITY`, then runs turns over
/// `policy` with handlers that each cost `cost`, retrying refused messages
/// after every turn as the runtime's wake-up would.
fn run(messages: u32, policy: ExternalPolicy, cost: Duration) -> Run {
    let capacity = NonZeroUsize::new(CAPACITY).expect("nonzero");
    let (sender, receiver) = bounded_external_channel(capacity, policy);
    let mut producer = Producer {
        sender,
        refused: VecDeque::new(),
    };
    (1..=messages).for_each(|n| producer.send(n));
    let held_back = producer.refused.len();

    let mut feed = Feed::new(receiver, cost);
    let mut turns = Vec::new();
    while !turns
        .last()
        .is_some_and(|(handled, _): &(Vec<u32>, _)| handled.is_empty())
    {
        turns.push(feed.turn());
        producer.retry();
    }
    Run { held_back, turns }
}

fn show(title: &str, run: &Run) {
    println!("{title}: {} held back by the full queue", run.held_back);
    for (i, (handled, ended)) in run.turns.iter().enumerate() {
        println!(
            "  turn {}: {handled:?} more pending: {}",
            i + 1,
            ended.exhausted
        );
    }
}

fn main() {
    show("count budget", &run(6, COUNT_BUDGET, Duration::ZERO));
    show("time budget", &run(7, TIME_BUDGET, HANDLER_COST));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handled(run: &Run) -> Vec<Vec<u32>> {
        run.turns.iter().map(|(h, _)| h.clone()).collect()
    }

    #[test]
    fn the_count_budget_caps_a_turn_and_the_overflow_waits_in_order() {
        let run = run(6, COUNT_BUDGET, Duration::ZERO);
        assert_eq!(run.held_back, 2, "6 messages into room for 4");
        assert_eq!(handled(&run), [vec![1, 2, 3], vec![4, 5, 6], vec![]]);
        let more_pending: Vec<bool> = run.turns.iter().map(|(_, t)| t.exhausted).collect();
        assert_eq!(
            more_pending,
            [true, true, false],
            "a full group says more may wait; the empty queue does not"
        );
        assert!(run.turns[..2].iter().all(|(_, t)| t.changed));
    }

    #[test]
    fn the_time_budget_stops_a_turn_before_the_count_does() {
        let run = run(7, TIME_BUDGET, HANDLER_COST);
        // 0 ms, 400 ms and 800 ms are under the second; at 1200 ms the turn
        // yields even though 97 of the 100 messages are unused.
        assert_eq!(
            handled(&run),
            [vec![1, 2, 3], vec![4, 5, 6], vec![7], vec![]]
        );
        assert!(run.turns[0].1.exhausted, "stopped by time, more is queued");
        assert!(run.turns[1].1.exhausted);
        assert!(
            !run.turns[2].1.exhausted,
            "one message left the queue dry before the clock ran out"
        );
    }

    #[test]
    fn a_zero_duration_still_handles_one_message_per_turn() {
        let policy = ExternalPolicy {
            max_messages: 100,
            max_duration: Duration::ZERO,
        };
        let run = run(3, policy, Duration::ZERO);
        assert_eq!(handled(&run), [vec![1], vec![2], vec![3], vec![]]);
    }

    #[test]
    fn the_lossy_send_reports_the_drop_and_a_closed_queue_returns_the_message() {
        let capacity = NonZeroUsize::new(1).expect("nonzero");
        let (sender, receiver) = bounded_external_channel(capacity, COUNT_BUDGET);
        assert_eq!(receiver.policy(), COUNT_BUDGET, "the receiver carries it");
        assert!(sender.send(1u32));
        assert!(
            !sender.send(2u32),
            "full: dropped, the caller learns only false"
        );
        let Err(ExternalSendError::Full(message)) = sender.try_send(External::new(2u32)) else {
            panic!("a full queue returns the message");
        };
        assert_eq!(message.downcast_ref::<u32>(), Some(&2));

        let mut feed = Feed::new(receiver, Duration::ZERO);
        assert_eq!(feed.turn().0, [1]);

        drop(feed);
        let Err(ExternalSendError::Closed(message)) = sender.try_send(External::new(3u32)) else {
            panic!("a dropped receiver closes the queue");
        };
        assert_eq!(message.downcast_ref::<u32>(), Some(&3));
    }
}

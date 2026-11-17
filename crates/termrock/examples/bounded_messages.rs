//! Background messages through a bounded channel. The runtime hands the app at
//! most `ExternalPolicy::max_messages` per turn within `max_duration`; a
//! producer that finds the queue full gets its message back and decides what
//! to do. This example retries it. `send` is the lossy alternative.
//!
//! The channel, its capacity, `try_send`'s `Full` and `Closed` errors, the
//! receiver's `policy` and `try_recv` are the library's. The runtime's own
//! per-turn loop (`pump_external`) is private and needs a terminal, so `turn`
//! below spells out the same count and time budget over the public receiver
//! to show what the runtime does between two reads of terminal input. The
//! runtime's loop is tested in `src/runtime.rs`.
//!
//! `cargo run -p termrock --example bounded_messages`

use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use termrock::core::event::External;
use termrock::runtime::{
    ExternalPolicy, ExternalReceiver, ExternalSendError, ExternalSender, bounded_external_channel,
};

/// Queue room 4; a turn handles at most 3 messages in under a second.
const CAPACITY: usize = 4;
const POLICY: ExternalPolicy = ExternalPolicy {
    max_messages: 3,
    max_duration: Duration::from_secs(1),
};

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

/// Stands in for one runtime turn: handle messages until the receiver's own
/// budget is spent, as the runtime's private `pump_external` does.
fn turn(receiver: &ExternalReceiver) -> Vec<u32> {
    let policy = receiver.policy();
    let start = Instant::now();
    let mut handled = Vec::new();
    while handled.len() < policy.max_messages && start.elapsed() < policy.max_duration {
        let Some(message) = receiver.try_recv() else {
            break;
        };
        handled.extend(message.downcast_ref::<u32>());
    }
    handled
}

fn run() -> (Vec<Vec<u32>>, usize) {
    let capacity = NonZeroUsize::new(CAPACITY).expect("nonzero");
    let (sender, receiver) = bounded_external_channel(capacity, POLICY);
    let mut producer = Producer {
        sender,
        refused: VecDeque::new(),
    };
    (1..=6).for_each(|n| producer.send(n));
    let held_back = producer.refused.len();

    let mut turns = Vec::new();
    while !turns.last().is_some_and(Vec::is_empty) {
        turns.push(turn(&receiver));
        producer.retry();
    }
    (turns, held_back)
}

fn main() {
    let (turns, held_back) = run();
    println!("held back by the full queue: {held_back}");
    for (i, handled) in turns.iter().enumerate() {
        println!("turn {}: {handled:?}", i + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_beyond_the_budget_wait_and_arrive_in_order() {
        let (turns, held_back) = run();
        assert_eq!(held_back, 2, "6 messages into room for 4");
        assert!(turns.iter().all(|t| t.len() <= POLICY.max_messages));
        assert_eq!(turns[..2], [vec![1, 2, 3], vec![4, 5, 6]]);
        let all: Vec<u32> = turns.concat();
        assert_eq!(all, [1, 2, 3, 4, 5, 6], "nothing lost, order kept");
    }

    #[test]
    fn the_lossy_send_reports_the_drop_and_a_closed_queue_returns_the_message() {
        let capacity = NonZeroUsize::new(1).expect("nonzero");
        let (sender, receiver) = bounded_external_channel(capacity, POLICY);
        assert_eq!(receiver.policy(), POLICY, "the receiver carries the budget");
        assert!(sender.send(1u32));
        assert!(
            !sender.send(2u32),
            "full: dropped, the caller learns only false"
        );
        assert_eq!(turn(&receiver), [1]);

        drop(receiver);
        let Err(ExternalSendError::Closed(message)) = sender.try_send(External::new(3u32)) else {
            panic!("a dropped receiver closes the queue");
        };
        assert_eq!(message.downcast_ref::<u32>(), Some(&3));
    }
}

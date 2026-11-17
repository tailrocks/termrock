//! A background producer for the Activity page's channel feed.
//!
//! It sends bursts larger than the channel's capacity through a bounded
//! external channel. On `Full` it keeps the rejected message, waits briefly and
//! retries the same one, so a burst is delivered in order and no message is
//! lost. It never blocks the UI, which only drains the queue a bounded group at
//! a time.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use termrock::core::event::External;
use termrock::runtime::{ExternalSendError, ExternalSender};

const RETRY_PAUSE: Duration = Duration::from_millis(2);
const PAUSE_STEP: Duration = Duration::from_millis(10);

/// One message of the feed, numbered from zero in send order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedLine {
    pub seq: u64,
}

#[derive(Debug, Default)]
struct Counters {
    sent: AtomicU64,
    full: AtomicU64,
    stop: AtomicBool,
}

/// A running producer. Dropping it, or [`Feed::stop`], ends the thread.
#[derive(Debug)]
pub struct Feed {
    counters: Arc<Counters>,
}

impl Feed {
    /// Starts sending `burst` messages, pausing `pause` between bursts.
    pub fn start(sender: ExternalSender, burst: usize, pause: Duration) -> Self {
        let counters = Arc::new(Counters::default());
        let shared = Arc::clone(&counters);
        std::thread::spawn(move || produce(&sender, burst, pause, &shared));
        Self { counters }
    }

    /// Messages the channel accepted.
    pub fn sent(&self) -> u64 {
        self.counters.sent.load(Ordering::Relaxed)
    }

    /// Times the channel answered `Full` and the producer retried.
    pub fn full(&self) -> u64 {
        self.counters.full.load(Ordering::Relaxed)
    }

    pub fn stop(&self) {
        self.counters.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Feed {
    fn drop(&mut self) {
        self.stop();
    }
}

fn produce(sender: &ExternalSender, burst: usize, pause: Duration, counters: &Counters) {
    let stopped = || counters.stop.load(Ordering::Relaxed);
    let mut seq = 0;
    while !stopped() {
        for _ in 0..burst {
            let mut message = External::new(FeedLine { seq });
            loop {
                match sender.try_send(message) {
                    Ok(()) => break,
                    Err(ExternalSendError::Full(back)) => {
                        counters.full.fetch_add(1, Ordering::Relaxed);
                        if stopped() {
                            return;
                        }
                        message = back;
                        std::thread::sleep(RETRY_PAUSE);
                    }
                    Err(ExternalSendError::Closed(_)) => return,
                }
            }
            counters.sent.fetch_add(1, Ordering::Relaxed);
            seq += 1;
        }
        let mut waited = Duration::ZERO;
        while waited < pause && !stopped() {
            std::thread::sleep(PAUSE_STEP);
            waited += PAUSE_STEP;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroUsize;
    use std::time::Instant;
    use termrock::runtime::{ExternalPolicy, bounded_external_channel};

    #[test]
    fn a_burst_over_capacity_retries_on_full_and_loses_nothing() {
        let capacity = NonZeroUsize::new(2).expect("nonzero");
        let (tx, rx) = bounded_external_channel(capacity, ExternalPolicy::default());
        let feed = Feed::start(tx, 10, Duration::from_secs(3600));

        let deadline = Instant::now() + Duration::from_secs(20);
        let mut seen = Vec::new();
        while seen.len() < 10 {
            assert!(Instant::now() < deadline, "feed stalled at {seen:?}");
            match rx.try_recv() {
                Some(message) => seen.push(message.downcast_ref::<FeedLine>().expect("feed").seq),
                None => std::thread::yield_now(),
            }
        }
        assert_eq!(seen, (0..10).collect::<Vec<_>>(), "in order, none dropped");
        assert!(feed.full() > 0, "a burst of 10 overflows a capacity of 2");
        assert_eq!(feed.sent(), 10);
    }

    #[test]
    fn a_dropped_receiver_ends_the_producer() {
        let capacity = NonZeroUsize::new(1).expect("nonzero");
        let (tx, rx) = bounded_external_channel(capacity, ExternalPolicy::default());
        drop(rx);
        let feed = Feed::start(tx, 5, Duration::ZERO);
        let deadline = Instant::now() + Duration::from_secs(20);
        while Arc::strong_count(&feed.counters) > 1 {
            assert!(Instant::now() < deadline, "producer still running");
            std::thread::yield_now();
        }
        assert_eq!(feed.sent(), 0);
    }
}

//! Work contract for the simulated transport feeding a terminal pane (#56,
//! #83).
//!
//! The scripted program queues one oversized output chunk. Each pane tick
//! drains the link with the pane's feed budget and feeds what it gets to the
//! emulator, so the bytes fed per tick are the transport's cost. They must
//! equal the budget, never exceed it, and sum to the stream exactly once.

use junie_tui::metrics::harness::assert_exact;
use junie_tui::metrics::{Metric, measure};

use super::*;

/// Stream of `len` printable bytes in 40-byte lines.
fn stream(len: usize) -> Vec<u8> {
    let mut bytes: Vec<u8> = (0..len / 40 + 1)
        .flat_map(|i| format!("{i:06} scripted output line of the sim\r\n").into_bytes())
        .collect();
    bytes.truncate(len);
    bytes
}

#[test]
fn work_contract_jackin_scripted_output_budget() {
    const STREAM: usize = 200_000;
    // (feed budget in bytes per tick, ticks until the stream is fed).
    for (budget, ticks) in [(1_024usize, 196usize), (4_096, 49), (65_536, 4)] {
        let mut pane = pane_over(stream(STREAM), budget);
        let mut total = 0u64;
        for tick in 0..ticks {
            let remaining = STREAM - budget * tick;
            let fed = remaining.min(budget) as u64;
            let (_, work) = measure(|| pane.tick(0));
            assert_exact(
                &work,
                &format!("tick {tick} at budget {budget}"),
                &[(Metric::TerminalBytesFed, fed)],
            );
            total += fed;
        }
        assert_eq!(total, STREAM as u64, "the stream is fed exactly once");
        let (_, idle) = measure(|| pane.tick(0));
        assert_exact(&idle, "tick after the stream", &[]);
    }
}

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::Range;

use super::Diagnostic;
use crate::core::text::TextBuffer;
use crate::theme::SyntaxTone;

#[cfg(test)]
pub(super) mod work {
    use std::cell::Cell;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Counts {
        pub syntax_builds: usize,
        pub diagnostic_builds: usize,
        pub find_scans: usize,
        pub bracket_builds: usize,
        pub segment_builds: usize,
        pub rows: usize,
        pub graphemes: usize,
    }

    thread_local! {
        static COUNTS: Cell<Counts> = const { Cell::new(Counts {
            syntax_builds: 0,
            diagnostic_builds: 0,
            find_scans: 0,
            bracket_builds: 0,
            segment_builds: 0,
            rows: 0,
            graphemes: 0,
        }) };
    }

    pub fn tally(update: impl FnOnce(&mut Counts)) {
        COUNTS.with(|c| {
            let mut counts = c.get();
            update(&mut counts);
            c.set(counts);
        });
    }

    pub fn counts() -> Counts {
        COUNTS.with(Cell::get)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ProviderKey {
    pub text: u64,
    pub provider: u64,
}

pub(super) fn next_generation(value: u64) -> u64 {
    value.checked_add(1).expect("provider generation exhausted")
}

#[derive(Clone, Copy, Debug)]
struct Run {
    start: usize,
    end: usize,
    ordinal: usize,
}

#[derive(Clone, Debug, Default)]
pub(super) struct RunIndex {
    runs: Vec<Run>,
}

impl RunIndex {
    pub fn build<'a>(ranges: impl Iterator<Item = &'a Range<usize>>) -> Self {
        let mut events = Vec::new();
        for (ordinal, range) in ranges.enumerate() {
            if range.start < range.end {
                events.push((range.start, true, ordinal));
                events.push((range.end, false, ordinal));
            }
        }
        events.sort_unstable_by_key(|&(offset, add, _)| (offset, add));
        let mut active = BTreeSet::new();
        let mut runs = Vec::new();
        let mut i = 0;
        while i < events.len() {
            let offset = events[i].0;
            while i < events.len() && events[i].0 == offset {
                let (_, add, ordinal) = events[i];
                if add {
                    active.insert(ordinal);
                } else {
                    active.remove(&ordinal);
                }
                i += 1;
            }
            if let (Some(&ordinal), Some(&(end, _, _))) = (active.first(), events.get(i)) {
                runs.push(Run {
                    start: offset,
                    end,
                    ordinal,
                });
            }
        }
        Self { runs }
    }

    pub fn at(&self, offset: usize) -> Option<usize> {
        let i = self
            .runs
            .partition_point(|r| r.start <= offset)
            .checked_sub(1)?;
        let run = &self.runs[i];
        (offset < run.end).then_some(run.ordinal)
    }
}

#[derive(Clone, Debug)]
pub(super) struct SyntaxCache {
    pub key: ProviderKey,
    tones: Vec<SyntaxTone>,
    runs: RunIndex,
}

impl SyntaxCache {
    pub fn build(key: ProviderKey, spans: Vec<(Range<usize>, SyntaxTone)>) -> Self {
        #[cfg(test)]
        work::tally(|c| c.syntax_builds += 1);
        let runs = RunIndex::build(spans.iter().map(|(r, _)| r));
        let tones = spans.into_iter().map(|(_, tone)| tone).collect();
        Self { key, tones, runs }
    }

    pub fn tone_at(&self, offset: usize) -> SyntaxTone {
        self.runs
            .at(offset)
            .map_or(SyntaxTone::Plain, |ordinal| self.tones[ordinal])
    }
}

#[derive(Clone, Debug)]
pub(super) struct DiagnosticIndex {
    pub text: u64,
    pub revision: u64,
    runs: RunIndex,
    exact: BTreeMap<usize, usize>,
    gutter: BTreeMap<usize, usize>,
    nearest: Vec<(usize, usize)>,
}

impl DiagnosticIndex {
    pub fn build(
        text: u64,
        revision: u64,
        diagnostics: &[Diagnostic],
        buffer: &TextBuffer,
    ) -> Self {
        #[cfg(test)]
        work::tally(|c| c.diagnostic_builds += 1);
        let runs = RunIndex::build(diagnostics.iter().map(|d| &d.range));
        let mut exact = BTreeMap::new();
        let mut starts: BTreeMap<usize, usize> = BTreeMap::new();
        let mut gutter = BTreeMap::new();
        for (ordinal, d) in diagnostics.iter().enumerate() {
            if d.range.is_empty() {
                exact.entry(d.range.start).or_insert(ordinal);
            }
            starts.entry(d.range.start).or_insert(ordinal);
            let line = buffer.line_at_offset(d.range.start);
            if buffer
                .line_range(line)
                .is_some_and(|r| d.range.start <= r.end)
            {
                gutter.entry(line).or_insert(ordinal);
            }
        }
        Self {
            text,
            revision,
            runs,
            exact,
            gutter,
            nearest: starts.into_iter().collect(),
        }
    }

    pub fn at(&self, offset: usize) -> Option<usize> {
        let range = self.runs.at(offset);
        let exact = self.exact.get(&offset).copied();
        match (range, exact) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    pub fn gutter(&self, line: usize) -> Option<usize> {
        self.gutter.get(&line).copied()
    }

    pub fn nearest(&self, offset: usize) -> Option<usize> {
        let above = self.nearest.partition_point(|&(start, _)| start < offset);
        let before = above.checked_sub(1).map(|i| self.nearest[i]);
        let after = self.nearest.get(above).copied();
        match (before, after) {
            (Some(b), Some(a)) => {
                let (db, da) = (offset.abs_diff(b.0), offset.abs_diff(a.0));
                Some(if da < db || (da == db && a.1 < b.1) {
                    a.1
                } else {
                    b.1
                })
            }
            (Some(x), None) | (None, Some(x)) => Some(x.1),
            (None, None) => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct BracketMap {
    pub text: u64,
    partner: HashMap<usize, usize>,
}

impl BracketMap {
    pub fn build(text: u64, source: &str) -> Self {
        #[cfg(test)]
        work::tally(|c| c.bracket_builds += 1);
        let mut partner = HashMap::new();
        let (mut round, mut square) = (Vec::new(), Vec::new());
        for (i, b) in source.bytes().enumerate() {
            let (stack, open) = match b {
                b'(' => (&mut round, true),
                b')' => (&mut round, false),
                b'[' => (&mut square, true),
                b']' => (&mut square, false),
                _ => continue,
            };
            if open {
                stack.push(i);
            } else if let Some(o) = stack.pop() {
                partner.insert(o, i);
                partner.insert(i, o);
            }
        }
        Self { text, partner }
    }

    pub fn pair_at(&self, cursor: usize) -> Option<(usize, usize)> {
        [cursor, cursor.wrapping_sub(1)]
            .into_iter()
            .find_map(|p| self.partner.get(&p).map(|&q| (p.min(q), p.max(q))))
    }
}

#[derive(Clone, Debug)]
pub(super) struct BlockPoint {
    offset: usize,
    at: Option<usize>,
    after: Option<usize>,
}

#[derive(Clone, Debug)]
pub(super) struct SegmentSnapshot {
    pub key: ProviderKey,
    pub blocks: Vec<Range<usize>>,
    points: Vec<BlockPoint>,
    fallback: Vec<(usize, usize)>,
}

impl SegmentSnapshot {
    pub fn build(key: ProviderKey, blocks: Vec<Range<usize>>) -> Self {
        #[cfg(test)]
        work::tally(|c| c.segment_builds += 1);
        let mut events: BTreeMap<usize, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
        for (ordinal, block) in blocks.iter().enumerate() {
            if block.start <= block.end {
                events.entry(block.start).or_default().0.push(ordinal);
                events.entry(block.end).or_default().1.push(ordinal);
            }
        }
        let mut active = BTreeSet::new();
        let mut points = Vec::with_capacity(events.len());
        for (offset, (starts, ends)) in events {
            for ordinal in starts {
                active.insert(ordinal);
            }
            let at = active.first().copied();
            for ordinal in ends {
                active.remove(&ordinal);
            }
            points.push(BlockPoint {
                offset,
                at,
                after: active.first().copied(),
            });
        }
        let mut fallback: Vec<_> = blocks
            .iter()
            .enumerate()
            .map(|(ordinal, block)| (block.end, ordinal))
            .collect();
        fallback.sort_unstable();
        let mut max_ordinal = 0;
        for (_, ordinal) in &mut fallback {
            max_ordinal = max_ordinal.max(*ordinal);
            *ordinal = max_ordinal;
        }
        Self {
            key,
            blocks,
            points,
            fallback,
        }
    }

    pub fn block_at(&self, cursor: usize) -> Option<Range<usize>> {
        let containing = self
            .points
            .partition_point(|p| p.offset <= cursor)
            .checked_sub(1)
            .and_then(|i| {
                let point = &self.points[i];
                if point.offset == cursor {
                    point.at
                } else {
                    point.after
                }
            });
        let ordinal = containing.or_else(|| {
            self.fallback
                .partition_point(|entry| entry.0 <= cursor)
                .checked_sub(1)
                .map(|i| self.fallback[i].1)
        })?;
        Some(self.blocks[ordinal].clone())
    }
}

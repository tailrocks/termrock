use std::time::{Duration, Instant};

use crate::core::event::Key;
use ratatui::crossterm::event::KeyCode;

#[derive(Debug, Clone)]
pub struct TypeAhead {
    buf: String,
    last: Option<Instant>,

    pub timeout: Duration,
}

impl Default for TypeAhead {
    fn default() -> Self {
        Self {
            buf: String::new(),
            last: None,
            timeout: Self::DEFAULT_TIMEOUT,
        }
    }
}

impl TypeAhead {
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(1000);

    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            timeout,
            ..Self::default()
        }
    }

    pub fn char_of(key: &Key) -> Option<char> {
        match key.code {
            KeyCode::Char(c) if !key.chord() && c.is_alphanumeric() => Some(c),
            _ => None,
        }
    }

    pub fn feed(&mut self, c: char) -> &str {
        self.feed_at(c, Instant::now())
    }

    pub fn feed_at(&mut self, c: char, now: Instant) -> &str {
        if self
            .last
            .is_none_or(|t| now.duration_since(t) > self.timeout)
        {
            self.buf.clear();
        }
        self.last = Some(now);
        self.buf.extend(c.to_lowercase());
        &self.buf
    }

    pub fn reset(&mut self) {
        self.buf.clear();
        self.last = None;
    }

    pub fn find<'a>(
        prefix: &str,
        labels: impl ExactSizeIterator<Item = &'a str> + Clone,
        from: usize,
        eligible: impl Fn(usize) -> bool,
    ) -> Option<usize> {
        let n = labels.len();
        if n == 0 || prefix.is_empty() {
            return None;
        }
        let mut chars = prefix.chars();
        let first = chars.next()?;
        let repeated = chars.all(|c| c == first);
        let (needle, start) = if repeated && prefix.chars().count() > 1 {
            (first.to_string(), from + 1)
        } else {
            (prefix.to_owned(), from)
        };
        let all: Vec<&str> = labels.collect();
        (0..n)
            .map(|k| (start + k) % n)
            .find(|&i| eligible(i) && all[i].to_lowercase().starts_with(&needle))
    }
}

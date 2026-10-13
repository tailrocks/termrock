// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Immediately-ready one-shot subscription for runtime adapters and tests.

/// Result of polling a [`ReadySubscription`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReadySubscriptionPoll<T> {
    /// The subscription yielded its sole value.
    Ready(T),
    /// The value was already consumed.
    Closed,
}

/// Fused one-shot subscription whose value is ready on the first poll.
///
/// This small adapter is runtime-neutral: it does not spawn, block, wake, or
/// depend on an async executor. Hosts can translate [`ReadySubscriptionPoll`]
/// into their application loop's subscription vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadySubscription<T> {
    value: Option<T>,
}

impl<T> ReadySubscription<T> {
    /// Creates a subscription that yields `value` immediately and once.
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self { value: Some(value) }
    }

    /// Yields the value on the first call and reports `Closed` thereafter.
    pub fn poll_next(&mut self) -> ReadySubscriptionPoll<T> {
        self.value
            .take()
            .map_or(ReadySubscriptionPoll::Closed, ReadySubscriptionPoll::Ready)
    }

    /// Reports whether the one-shot value has been consumed.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        self.value.is_none()
    }
}

/// Creates an immediately-ready one-shot subscription.
#[must_use]
pub const fn ready_subscription<T>(value: T) -> ReadySubscription<T> {
    ReadySubscription::new(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_poll_is_ready_then_closed() {
        let mut subscription = ReadySubscription::new("done");
        assert_eq!(
            subscription.poll_next(),
            ReadySubscriptionPoll::Ready("done")
        );
        assert!(subscription.is_closed());
        assert_eq!(subscription.poll_next(), ReadySubscriptionPoll::Closed);
    }

    #[test]
    fn helper_constructs_the_same_one_shot() {
        let mut subscription = ready_subscription(42);
        assert_eq!(subscription.poll_next(), ReadySubscriptionPoll::Ready(42));
        assert_eq!(subscription.poll_next(), ReadySubscriptionPoll::Closed);
    }
}

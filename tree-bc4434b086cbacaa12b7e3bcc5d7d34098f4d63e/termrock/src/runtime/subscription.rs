use std::sync::mpsc::{Receiver, TryRecvError};

#[derive(Debug, Clone, PartialEq, Eq)]
/// Available `SubscriptionPoll` choices.
pub enum SubscriptionPoll<Event> {
    /// Selects the `Ready` behavior.
    Ready(Event),
    /// Selects the `Pending` behavior.
    Pending,
    /// Selects the `Closed` behavior.
    Closed,
}
impl<Event> SubscriptionPoll<Event> {
    #[must_use]
    /// Returns whether `pending`.
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }
}

/// Documentation for `item`.
pub trait Subscription {
    /// The `Output;` value produced by this contract.
    type Output;
    /// Performs the `poll_next` operation.
    fn poll_next(&mut self) -> SubscriptionPoll<Self::Output>;
}

/// Data carried by `ClosureSubscription`.
pub struct ClosureSubscription<F>(pub F);
impl<Event, F> Subscription for ClosureSubscription<F>
where
    F: FnMut() -> SubscriptionPoll<Event>,
{
    type Output = Event;
    fn poll_next(&mut self) -> SubscriptionPoll<Event> {
        (self.0)()
    }
}

/// Data carried by `StdSubscription`.
pub struct StdSubscription<Event>(pub Receiver<Event>);
impl<Event> Subscription for StdSubscription<Event> {
    type Output = Event;
    fn poll_next(&mut self) -> SubscriptionPoll<Event> {
        match self.0.try_recv() {
            Ok(event) => SubscriptionPoll::Ready(event),
            Err(TryRecvError::Empty) => SubscriptionPoll::Pending,
            Err(TryRecvError::Disconnected) => SubscriptionPoll::Closed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closure_adapter_supports_foreign_sources() {
        let mut value = Some(7);
        let mut subscription = ClosureSubscription(|| {
            value
                .take()
                .map_or(SubscriptionPoll::Closed, SubscriptionPoll::Ready)
        });
        assert_eq!(subscription.poll_next(), SubscriptionPoll::Ready(7));
        assert_eq!(subscription.poll_next(), SubscriptionPoll::Closed);
    }
}

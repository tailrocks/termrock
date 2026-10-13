use std::sync::mpsc::{Receiver, TryRecvError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionPoll<Event> {
    Event(Event),
    Pending,
    Closed,
}

pub trait Subscription {
    type Event;
    fn poll(&mut self) -> SubscriptionPoll<Self::Event>;
}

pub struct ClosureSubscription<F>(pub F);
impl<Event, F> Subscription for ClosureSubscription<F>
where
    F: FnMut() -> SubscriptionPoll<Event>,
{
    type Event = Event;
    fn poll(&mut self) -> SubscriptionPoll<Event> {
        (self.0)()
    }
}

pub struct StdSubscription<Event>(pub Receiver<Event>);
impl<Event> Subscription for StdSubscription<Event> {
    type Event = Event;
    fn poll(&mut self) -> SubscriptionPoll<Event> {
        match self.0.try_recv() {
            Ok(event) => SubscriptionPoll::Event(event),
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
                .map_or(SubscriptionPoll::Closed, SubscriptionPoll::Event)
        });
        assert_eq!(subscription.poll(), SubscriptionPoll::Event(7));
        assert_eq!(subscription.poll(), SubscriptionPoll::Closed);
    }
}

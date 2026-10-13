//! Executor-neutral component and rendering contracts.

mod contract;
mod frame;
mod subscription;

pub use contract::{Component, Dirty, NoEffect, UpdateResult, View};
pub use frame::{drive_frame, drive_render};
pub use subscription::{ClosureSubscription, StdSubscription, Subscription, SubscriptionPoll};

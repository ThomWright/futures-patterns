//! A simple two-state future for learning state machine patterns.
//!
//! This is a custom example (not from tokio) that demonstrates building a
//! state machine from scratch. It's simpler than MaybeDone and helps understand
//! the fundamental concepts before moving to more complex patterns.
//!
//! # State machine
//!
//! ```text
//! new(n) --> Waiting --poll(), remaining > 0--> Waiting   Pending, wakes the task
//!                    --poll(), remaining == 0-> Ready     Ready(n)
//! ```
//!
//! The future starts in the `Waiting` state with `remaining` set to `n`. Each poll
//! decrements `remaining` and returns `Pending`, so the future is pending for `n`
//! polls. The next poll transitions to `Ready` and yields `n`.
//!
//! `Waiting` carries the original `n` alongside `remaining` because the output is
//! the original count, which is no longer recoverable once `remaining` has been
//! decremented.
//!
//! # When to use
//!
//! This pattern demonstrates:
//! - How to design a custom state machine
//! - How state transitions work in practice
//! - How to store data needed for state transitions
//! - The basics before moving to more complex patterns
//!
//! # Example
//!
//! ```
//! use futures_patterns::state_machine::two_state::CountDown;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let future = CountDown::new(3);
//! let result = future.await;
//! assert_eq!(result, 3);
//! # }
//! ```

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// A future that counts down from a specified number.
///
/// This demonstrates a simple two-state machine where the future transitions
/// from `Waiting` to `Ready` after a certain number of polls.
#[derive(Debug)]
pub enum CountDown {
    /// Waiting state - counting down with each poll.
    ///
    /// `remaining` is how many more polls are needed before completion. `total`
    /// is the original count, preserved so it can be yielded on completion.
    Waiting { remaining: usize, total: usize },

    /// Ready state - the countdown is complete.
    ///
    /// The stored value is the original count the future was created with.
    Ready { total: usize },
}

impl CountDown {
    /// Creates a new countdown future.
    ///
    /// The future will return `Poll::Pending` for `count` polls, then
    /// return `Poll::Ready(count)` on the final poll.
    ///
    /// # Example
    ///
    /// ```
    /// use futures_patterns::state_machine::two_state::CountDown;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let future = CountDown::new(5);
    /// let total = future.await;
    /// assert_eq!(total, 5);
    /// # }
    /// ```
    pub fn new(count: usize) -> Self {
        CountDown::Waiting {
            remaining: count,
            total: count,
        }
    }
}

impl Future for CountDown {
    type Output = usize;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match *self {
            CountDown::Waiting { remaining: 0, total } => {
                *self = CountDown::Ready { total };
                Poll::Ready(total)
            }
            CountDown::Waiting { remaining, total } => {
                *self = CountDown::Waiting {
                    remaining: remaining - 1,
                    total,
                };

                // Returning Pending without arranging a wake would hang the task
                // forever, so wake immediately to request another poll. Real futures
                // only wake once actual progress is possible.
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            CountDown::Ready { total } => {
                // Already completed - yield the same value again. CountDown is
                // deliberately idempotent here; Ready and Map instead panic.
                Poll::Ready(total)
            }
        }
    }
}

// No manual `Unpin` impl is needed: both variants hold only `usize`, so the compiler
// derives `Unpin` already.

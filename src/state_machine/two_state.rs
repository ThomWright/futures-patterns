//! A future that is pending for a fixed number of polls.
//!
//! Invented for this crate rather than taken from tokio, so the state machine stays
//! small enough to read in one go.
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
//! # Writing a state machine by hand
//!
//! - Writing the states out as an enum, and moving between them inside `poll`
//! - Carrying the data a later state will need
//! - Asking to be polled again, rather than being woken from outside
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

// CountDown's contract is entirely about how many times it is polled and what it
// wakes, so these drive it by hand rather than with `.await`.
#[cfg(test)]
mod tests {
    use super::CountDown;
    use crate::testing::{CountingWaker, poll_once, poll_until_ready};
    use std::task::Poll;

    #[test]
    fn yields_the_original_count_not_the_remaining_one() {
        let (output, _) = poll_until_ready(Box::pin(CountDown::new(3)).as_mut(), 10);
        assert_eq!(output, 3);
    }

    #[test]
    fn is_pending_for_exactly_count_polls() {
        let (output, polls) = poll_until_ready(Box::pin(CountDown::new(3)).as_mut(), 10);
        // Documented contract: Pending for `count` polls, then Ready on the next.
        assert_eq!(polls, 4, "expected 3 pending polls then 1 ready poll");
        assert_eq!(output, 3);
    }

    #[test]
    fn wakes_the_task_on_every_pending_poll() {
        let waker = CountingWaker::new();
        let mut future = Box::pin(CountDown::new(2));

        assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Pending);
        assert_eq!(waker.count(), 1, "a Pending poll must arrange to be re-polled");

        assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Pending);
        assert_eq!(waker.count(), 2);

        assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Ready(2));
        assert_eq!(waker.count(), 2, "a Ready poll must not wake the task");
    }

    #[test]
    fn zero_is_ready_immediately_without_waking() {
        let waker = CountingWaker::new();
        let mut future = Box::pin(CountDown::new(0));

        assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Ready(0));
        assert_eq!(waker.count(), 0);
    }

    #[test]
    fn stays_ready_when_polled_after_completion() {
        let mut future = Box::pin(CountDown::new(1));
        let waker = CountingWaker::new();

        assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Pending);
        assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Ready(1));
        // CountDown documents itself as idempotent, unlike Ready and Map which panic.
        assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Ready(1));
        assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Ready(1));
    }

    #[test]
    fn is_unpin_without_a_manual_impl() {
        // Both variants hold only `usize`, so the derived impl covers this.
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<CountDown>();
    }
}

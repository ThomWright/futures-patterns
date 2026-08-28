//! Race two futures, returning the first to complete.
//!
//! `Race` drives two futures at once and yields whichever finishes first, wrapped in
//! [`Either`] so the caller can tell which one it was.
//!
//! # Polling order
//!
//! `left` is polled first, and if it is ready `right` is not polled at all. So:
//!
//! - If both are ready, `left` wins, which is a way to express a priority.
//! - A `left` that is ready on every poll starves `right` completely. Tokio's `select!`
//!   rotates its branch order to avoid this; `Race` does not.
//! - [`timeout`](crate::time::timeout) depends on the order, polling the operation
//!   before the timer so a fast operation never reports a spurious timeout.
//!
//! # When to use
//!
//! Use this pattern for:
//! - Timeouts, racing the operation against a timer
//! - Fallbacks, trying A and taking B if it answers first
//! - Cancellation, racing against a signal that means stop
//!
//! # Example
//!
//! ```
//! use futures_patterns::composition::race::{race, Either};
//!
//! # #[tokio::main]
//! # async fn main() {
//! let fast = async { 42 };
//! let slow = async {
//!     tokio::time::sleep(std::time::Duration::from_secs(1)).await;
//!     100
//! };
//!
//! match race(fast, slow).await {
//!     Either::Left(value) => assert_eq!(value, 42),
//!     Either::Right(_) => panic!("slow future won!"),
//! }
//! # }
//! ```

use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// The output of a race operation.
///
/// Indicates which future completed first and contains its output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Either<L, R> {
    /// The left future completed first.
    Left(L),
    /// The right future completed first.
    Right(R),
}

pin_project! {
    /// Future for racing two futures to completion.
    ///
    /// Polls both futures and returns the output of whichever completes first.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Race<L, R> {
        // Both futures need to be pinned because we'll be polling them
        #[pin]
        left: L,
        #[pin]
        right: R,
    }
}

impl<L, R> Race<L, R> {
    /// Creates a new `Race` future.
    ///
    /// The two futures will be polled concurrently, and the first to complete
    /// will determine the result.
    ///
    /// # Example
    ///
    /// ```
    /// use futures_patterns::composition::race::Race;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// use futures_patterns::composition::race::Either;
    ///
    /// let future_a = async { 1 };
    /// let future_b = async { 2 };
    /// // Both are immediately ready, and left is polled first, so left wins.
    /// assert_eq!(Race::new(future_a, future_b).await, Either::Left(1));
    /// # }
    /// ```
    pub fn new(left: L, right: R) -> Self {
        Race { left, right }
    }
}

impl<L, R> Future for Race<L, R>
where
    L: Future,
    R: Future,
{
    type Output = Either<L::Output, R::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        // Poll the left future first
        // If it's ready, return immediately without polling right
        if let Poll::Ready(output) = this.left.poll(cx) {
            return Poll::Ready(Either::Left(output));
        }

        // Left is pending, try right
        if let Poll::Ready(output) = this.right.poll(cx) {
            return Poll::Ready(Either::Right(output));
        }

        // Both are pending
        // The wakers have been registered by the poll calls above,
        // so the runtime will wake us when either future makes progress
        Poll::Pending
    }
}

/// Race two futures, returning the first to complete.
///
/// This is a convenience function for creating a `Race` combinator.
/// The left future is polled first, so if both are ready simultaneously,
/// the left one wins.
///
/// # Example
///
/// ```
/// use futures_patterns::composition::race::{race, Either};
/// use futures_patterns::basic::ready::ready;
///
/// # #[tokio::main]
/// # async fn main() {
/// let result = race(ready(1), ready(2)).await;
/// match result {
///     Either::Left(v) => assert_eq!(v, 1),
///     Either::Right(v) => assert_eq!(v, 2),
/// }
/// # }
/// ```
pub fn race<L, R>(left: L, right: R) -> Race<L, R>
where
    L: Future,
    R: Future,
{
    Race::new(left, right)
}

// Focused on Race's documented left-bias, which is a behavioural promise rather
// than an implementation detail.
#[cfg(test)]
mod tests {
    use super::{Either, race};
    use crate::basic::pending::pending;
    use crate::basic::poll_fn::poll_fn;
    use crate::basic::ready::ready;
    use crate::state_machine::two_state::CountDown;
    use crate::testing::{CountingWaker, poll_once};
    use std::task::{Poll, Waker};

    #[test]
    fn left_wins_when_both_are_ready() {
        let mut fut = Box::pin(race(ready(1), ready(2)));
        assert_eq!(
            poll_once(fut.as_mut(), Waker::noop()),
            Poll::Ready(Either::Left(1))
        );
    }

    #[test]
    fn does_not_poll_the_right_future_when_the_left_is_ready() {
        // The documented consequence of left-bias: `right` is never touched at all.
        // A closure that panics on poll makes that observable.
        let right = poll_fn(|_cx| -> Poll<i32> { panic!("right must not be polled") });
        let mut fut = Box::pin(race(ready(1), right));
        assert_eq!(
            poll_once(fut.as_mut(), Waker::noop()),
            Poll::Ready(Either::Left(1))
        );
    }

    #[test]
    fn right_wins_when_the_left_is_pending() {
        let mut fut = Box::pin(race(pending::<i32>(), ready(2)));
        assert_eq!(
            poll_once(fut.as_mut(), Waker::noop()),
            Poll::Ready(Either::Right(2))
        );
    }

    #[test]
    fn is_pending_while_both_are_pending() {
        let mut fut = Box::pin(race(pending::<i32>(), pending::<i32>()));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
    }

    #[test]
    fn polls_both_futures_while_both_are_pending() {
        // Each CountDown wakes once per pending poll, so two wakes from a single poll
        // of Race proves both branches were polled and both registered the waker.
        let waker = CountingWaker::new();
        let mut fut = Box::pin(race(CountDown::new(5), CountDown::new(5)));

        assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Pending);
        assert_eq!(waker.count(), 2);
    }

    #[test]
    fn right_wins_when_it_completes_first() {
        // Left needs three polls, right needs one, so right wins despite being second.
        let mut fut = Box::pin(race(CountDown::new(3), CountDown::new(1)));
        let waker = CountingWaker::new();

        assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Pending);
        assert_eq!(
            poll_once(fut.as_mut(), &waker.waker()),
            Poll::Ready(Either::Right(1))
        );
    }

    #[test]
    fn branches_may_have_different_output_types() {
        let mut fut = Box::pin(race(ready("left"), ready(2u8)));
        assert_eq!(
            poll_once(fut.as_mut(), Waker::noop()),
            Poll::Ready(Either::Left("left"))
        );
    }
}

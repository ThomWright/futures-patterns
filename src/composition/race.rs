//! Race two futures, returning the first to complete.
//!
//! The `Race` pattern polls two futures concurrently and returns the output of
//! whichever completes first. This is a fundamental pattern for implementing
//! timeouts, cancellation, and alternative execution paths.
//!
//! # Pattern overview
//!
//! Race demonstrates:
//! - Polling multiple futures in a single poll call
//! - Using enums to represent which future won
//! - Pin projection with multiple pinned fields
//! - How to coordinate independent async operations
//!
//! # Polling order
//!
//! This implementation polls `left` first, then `right`. If `left` is ready,
//! we return immediately without polling `right`. This means:
//! - If both futures are ready, `left` wins
//! - This can be useful for prioritization
//! - Timeout patterns typically rely on this behavior
//!
//! # When to use
//!
//! Use this pattern for:
//! - Implementing timeouts (race with a timer)
//! - Providing alternative paths (try A, fall back to B)
//! - Implementing cancellation (race with a cancel signal)
//! - Building select/choice operations
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
//!     Either::Right(value) => panic!("slow future won!"),
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

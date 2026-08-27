//! A future that may have completed.
//!
//! This is a fundamental state machine pattern that wraps another future and
//! tracks whether it has completed. It uses an enum with three states to manage
//! the lifecycle:
//!
//! - `Future(Fut)` - The wrapped future hasn't completed yet
//! - `Done(Fut::Output)` - The future completed, and we're storing its output
//! - `Gone` - The output has been extracted
//!
//! # State transitions
//!
//! ```text
//! Future -> Done -> Gone
//!   ^       ^       ^
//!   |       |       |
//!   poll()  |    take_output()
//!           |
//!      poll() returns Ready
//! ```
//!
//! # Why three states?
//!
//! The `Gone` state exists to handle the case where someone calls `take_output()`
//! to extract the value. Without it, we'd need to use `Option<Fut::Output>` in the
//! `Done` variant, which would require `Fut::Output: Default` or similar constraints.
//!
//! # When to use
//!
//! This pattern is useful for:
//! - Implementing join/select operations that need to check completion status
//! - Building futures that need to poll multiple sub-futures
//! - Caching future results without requiring Clone
//! - Implementing try_join where you need to store successful results
//!
//! # Example
//!
//! ```
//! use futures_patterns::state_machine::maybe_done::{maybe_done, MaybeDone};
//! use std::pin::Pin;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let future = async { 42 };
//! let mut wrapped = Box::pin(maybe_done(future));
//!
//! // Poll it to completion
//! wrapped.as_mut().await;
//!
//! // Extract the output
//! let value = wrapped.as_mut().take_output();
//! assert_eq!(value, Some(42));
//!
//! // After extraction, the state is Gone
//! let value = wrapped.as_mut().take_output();
//! assert_eq!(value, None);
//! # }
//! ```

use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};

/// A future that may have completed.
///
/// This enum represents three distinct states in the lifecycle of an async operation.
#[derive(Debug)]
pub enum MaybeDone<Fut: Future> {
    /// A not-yet-completed future.
    ///
    /// The wrapped future is still being polled.
    Future(Fut),

    /// The output of the completed future.
    ///
    /// The future has completed successfully and we're holding its result.
    Done(Fut::Output),

    /// The empty variant after the result has been taken.
    ///
    /// This state is reached after calling `take_output()`. Polling in this
    /// state will panic.
    Gone,
}

// SAFETY: We never generate `Pin<&mut Fut::Output>`, so it's safe to implement
// Unpin when Fut is Unpin. The Output doesn't need to be pinned.
impl<Fut: Future + Unpin> Unpin for MaybeDone<Fut> {}

/// Wraps a future into a `MaybeDone`.
///
/// This allows tracking whether the future has completed and extracting its
/// output after completion.
///
/// # Example
///
/// ```
/// use futures_patterns::state_machine::maybe_done::maybe_done;
///
/// # #[tokio::main]
/// # async fn main() {
/// let mut wrapped = Box::pin(maybe_done(async { "done" }));
///
/// // Polling to completion yields `()`; the output is retrieved separately.
/// wrapped.as_mut().await;
/// assert_eq!(wrapped.as_mut().take_output(), Some("done"));
/// # }
/// ```
pub fn maybe_done<Fut: Future>(future: Fut) -> MaybeDone<Fut> {
    MaybeDone::Future(future)
}

impl<Fut: Future> MaybeDone<Fut> {
    /// Returns a mutable reference to the output of the future.
    ///
    /// Returns `Some` if and only if the inner future has completed and
    /// `take_output()` has not yet been called.
    ///
    /// # Example
    ///
    /// ```
    /// use futures_patterns::state_machine::maybe_done::maybe_done;
    /// use std::pin::Pin;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut wrapped = Box::pin(maybe_done(async { 42 }));
    /// wrapped.as_mut().await;
    ///
    /// if let Some(output) = wrapped.as_mut().output_mut() {
    ///     *output += 1;
    /// }
    ///
    /// assert_eq!(wrapped.as_mut().take_output(), Some(43));
    /// # }
    /// ```
    pub fn output_mut(self: Pin<&mut Self>) -> Option<&mut Fut::Output> {
        // SAFETY: We're only accessing the Done variant's output, not the Future variant.
        // We never create a Pin<&mut Fut::Output>, only &mut Fut::Output.
        unsafe {
            let this = self.get_unchecked_mut();
            match this {
                MaybeDone::Done(res) => Some(res),
                _ => None,
            }
        }
    }

    /// Attempts to take the output of a `MaybeDone` without driving it
    /// towards completion.
    ///
    /// Returns `Some(output)` if the future is in the `Done` state, and
    /// transitions to the `Gone` state. Returns `None` if the future is
    /// still running or already `Gone`.
    ///
    /// # Example
    ///
    /// ```
    /// use futures_patterns::state_machine::maybe_done::maybe_done;
    /// use std::pin::Pin;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut wrapped = Box::pin(maybe_done(async { 42 }));
    ///
    /// // Before completion
    /// assert_eq!(wrapped.as_mut().take_output(), None);
    ///
    /// // Complete the future
    /// wrapped.as_mut().await;
    ///
    /// // After completion
    /// assert_eq!(wrapped.as_mut().take_output(), Some(42));
    ///
    /// // After extraction
    /// assert_eq!(wrapped.as_mut().take_output(), None);
    /// # }
    /// ```
    #[inline]
    pub fn take_output(self: Pin<&mut Self>) -> Option<Fut::Output> {
        // SAFETY: We're replacing the enum variant, which is safe because we have
        // exclusive mutable access via Pin<&mut Self>.
        unsafe {
            let this = self.get_unchecked_mut();
            match this {
                MaybeDone::Done(_) => {}
                MaybeDone::Future(_) | MaybeDone::Gone => return None,
            };

            // Replace the Done variant with Gone, extracting the output
            if let MaybeDone::Done(output) = mem::replace(this, MaybeDone::Gone) {
                Some(output)
            } else {
                // SAFETY: We just matched on Done above, so this is unreachable
                unreachable!()
            }
        }
    }
}

impl<Fut: Future> Future for MaybeDone<Fut> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: We need to poll the inner future, which requires pinning it.
        // Since we're pinned and the Future variant contains the future, the future
        // is also pinned.
        let res = unsafe {
            match self.as_mut().get_unchecked_mut() {
                MaybeDone::Future(fut) => {
                    // Poll the inner future
                    // SAFETY: fut is pinned because self is pinned
                    match Pin::new_unchecked(fut).poll(cx) {
                        Poll::Ready(output) => output,
                        Poll::Pending => return Poll::Pending,
                    }
                }
                MaybeDone::Done(_) => {
                    // Already completed, return Ready immediately
                    return Poll::Ready(());
                }
                MaybeDone::Gone => {
                    // Polling after the value has been taken is a logic error
                    panic!("MaybeDone polled after value taken");
                }
            }
        };

        // Transition from Future to Done state
        self.set(MaybeDone::Done(res));
        Poll::Ready(())
    }
}

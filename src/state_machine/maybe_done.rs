//! A future that may have completed.
//!
//! Wraps another future and tracks whether it has finished, using three states:
//!
//! - `Future(Fut)` - The wrapped future hasn't completed yet.
//! - `Done(Fut::Output)` - The future completed, and we're storing its output.
//! - `Gone` - The output has been extracted.
//!
//! # State transitions
//!
//! ```text
//! Future --poll() returns Pending--> Future   (inner future still running)
//!        --poll() returns Ready----> Done     (output stored)
//!
//! Done   --poll()---------------> Done     (absorbed; inner is NOT re-polled)
//!        --take_output()--------> Gone     (output handed over)
//!
//! Gone   --poll()---------------> panic
//!        --take_output()--------> None
//! ```
//!
//! # `is_terminated` is deliberately blunt
//!
//! [`FusedFuture::is_terminated`] reports
//! `true` for both `Done` and `Gone`, even though polling those differs sharply:
//! `Done` absorbs the poll harmlessly, while `Gone` panics. It answers "should you
//! poll this?", not "what happens if you do", and for both the answer is no.
//!
//! The trait is written to allow this. `futures-core` says `is_terminated` may also
//! return `true` where a future "has become inactive and can no longer make progress
//! and should be ignored or dropped rather than being `poll`ed again", which describes
//! `Gone` exactly.
//!
//! [`FusedFuture::is_terminated`]: crate::fused::FusedFuture::is_terminated
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
//! - Implementing join/select operations that need to check completion status; see
//!   [`composition::join`](crate::composition::join), which is built from this and
//!   whose three problems map one to one onto these three states
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
//!
//! Follows `futures-util/src/future/maybe_done.rs`; see NOTICE.md.

use crate::fused::FusedFuture;
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

// Deliberately broader than the derived impl, which would also require
// `Fut::Output: Unpin` because the `Done` variant holds an output.
//
// SAFETY: the output is never structurally pinned -- no `Pin<&mut Fut::Output>` is
// ever created, and `take_output` moves it out freely -- so only `Fut` needs to be
// `Unpin` for the whole enum to be safely movable.
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
                // Unreachable: the match above returned for every variant but Done.
                unreachable!()
            }
        }
    }
}

impl<Fut: Future> FusedFuture for MaybeDone<Fut> {
    fn is_terminated(&self) -> bool {
        match self {
            MaybeDone::Future(_) => false,
            MaybeDone::Done(_) | MaybeDone::Gone => true,
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

// MaybeDone splits "has it finished?" from "give me the value", because those happen
// at different times when coordinating several futures. `join2` below shows why.
#[cfg(test)]
mod tests {
    use super::{MaybeDone, maybe_done};
    use crate::basic::ready::{Ready, ready};
    use crate::state_machine::two_state::CountDown;
    use crate::testing::poll_once;
    use std::marker::PhantomPinned;
    use std::pin::Pin;
    use std::task::{Poll, Waker};

    #[test]
    fn polling_to_completion_yields_unit_not_the_output() {
        // The output is deliberately not the inner future's; it is fetched separately.
        let mut fut = Box::pin(maybe_done(ready(42)));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(()));
    }

    #[test]
    fn take_output_returns_none_before_completion() {
        let mut fut = Box::pin(maybe_done(CountDown::new(3)));
        assert_eq!(fut.as_mut().take_output(), None);
    }

    #[test]
    fn take_output_yields_the_value_once_then_none() {
        let mut fut = Box::pin(maybe_done(ready(42)));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(()));

        assert_eq!(fut.as_mut().take_output(), Some(42));
        // Now Gone: the value has been taken and cannot be taken again.
        assert_eq!(fut.as_mut().take_output(), None);
    }

    #[test]
    fn done_absorbs_further_polls_without_touching_the_inner_future() {
        // This is what makes a uniform "poll every branch" loop legal in `join!`.
        // `ready` panics if polled twice, so completing without a panic proves the
        // inner future was not re-polled.
        let mut fut = Box::pin(maybe_done(ready(42)));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(()));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(()));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(()));
        assert_eq!(fut.as_mut().take_output(), Some(42));
    }

    #[test]
    #[should_panic(expected = "MaybeDone polled after value taken")]
    fn polling_after_the_value_is_taken_panics() {
        let mut fut = Box::pin(maybe_done(ready(42)));
        let _ = poll_once(fut.as_mut(), Waker::noop());
        let _ = fut.as_mut().take_output();
        let _ = poll_once(fut.as_mut(), Waker::noop());
    }

    #[test]
    fn output_mut_allows_editing_the_stored_value() {
        let mut fut = Box::pin(maybe_done(ready(42)));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(()));

        *fut.as_mut().output_mut().expect("should be Done") += 1;
        assert_eq!(fut.as_mut().take_output(), Some(43));
    }

    #[test]
    fn output_mut_is_none_unless_done() {
        let mut fut = Box::pin(maybe_done(CountDown::new(3)));
        assert!(fut.as_mut().output_mut().is_none());
    }

    #[test]
    fn is_terminated_covers_both_finished_states() {
        use crate::fused::FusedFuture;

        let mut fut = Box::pin(maybe_done(ready(42)));
        assert!(!fut.is_terminated(), "still running");

        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(()));
        assert!(fut.is_terminated(), "Done: safe to poll, but pointless");

        assert_eq!(fut.as_mut().take_output(), Some(42));
        assert!(fut.is_terminated(), "Gone: polling would now panic");
    }

    #[test]
    fn unpin_is_broader_than_the_derived_impl_would_be() {
        // The manual impl requires only `Fut: Unpin`. A derived one would also demand
        // `Fut::Output: Unpin`, because the `Done` variant holds an output.
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<MaybeDone<Ready<PhantomPinned>>>();
    }

    #[test]
    fn can_be_polled_through_a_plain_pin_when_unpin() {
        // A consequence of that Unpin impl: no boxing needed.
        let mut fut = maybe_done(ready(1));
        let pinned = Pin::new(&mut fut);
        assert_eq!(poll_once(pinned, Waker::noop()), Poll::Ready(()));
    }
}

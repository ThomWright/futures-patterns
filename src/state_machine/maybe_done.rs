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
use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// A future that may have completed.
    ///
    /// Only the inner future is `#[pin]`, so the stored output is ordinary data: it can
    /// be handed out as `&mut` and moved out by `take_output`, and the generated `Unpin`
    /// impl asks only whether `Fut` is `Unpin`, never `Fut::Output`.
    #[project = MaybeDoneProj]
    #[project_replace = MaybeDoneProjReplace]
    #[derive(Debug)]
    pub enum MaybeDone<Fut: Future> {
        /// A not-yet-completed future.
        ///
        /// The wrapped future is still being polled.
        Future {
            #[pin]
            future: Fut,
        },

        /// The output of the completed future.
        ///
        /// The future has completed successfully and we're holding its result.
        Done { output: Fut::Output },

        /// The empty variant after the result has been taken.
        ///
        /// This state is reached after calling `take_output()`. Polling in this
        /// state will panic.
        Gone,
    }
}

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
    MaybeDone::Future { future }
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
        // `output` is not a `#[pin]` field, so projection hands back a plain `&mut`.
        match self.project() {
            MaybeDoneProj::Done { output } => Some(output),
            MaybeDoneProj::Future { .. } | MaybeDoneProj::Gone => None,
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
        // Checking first is what keeps a running future running: `project_replace`
        // overwrites whatever is there, dropping a `#[pin]` field in place rather than
        // returning it, so calling it in the `Future` state would destroy the future
        // and hand back nothing.
        if !matches!(&*self, MaybeDone::Done { .. }) {
            return None;
        }

        match self.project_replace(MaybeDone::Gone) {
            MaybeDoneProjReplace::Done { output } => Some(output),
            // Unreachable: the guard above returned for every other variant.
            _ => unreachable!(),
        }
    }
}

impl<Fut: Future> FusedFuture for MaybeDone<Fut> {
    fn is_terminated(&self) -> bool {
        match self {
            MaybeDone::Future { .. } => false,
            MaybeDone::Done { .. } | MaybeDone::Gone => true,
        }
    }
}

impl<Fut: Future> Future for MaybeDone<Fut> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let output = match self.as_mut().project() {
            // `future` is a `#[pin]` field, so projection hands back the
            // `Pin<&mut Fut>` that polling it requires.
            MaybeDoneProj::Future { future } => match future.poll(cx) {
                Poll::Ready(output) => output,
                Poll::Pending => return Poll::Pending,
            },
            // Absorb the poll rather than forwarding it: the inner future has already
            // completed and must not be polled again.
            MaybeDoneProj::Done { .. } => return Poll::Ready(()),
            MaybeDoneProj::Gone => panic!("MaybeDone polled after value taken"),
        };

        self.set(MaybeDone::Done { output });
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
    fn unpin_does_not_depend_on_the_output_type() {
        // `pin_project!` builds the `Unpin` impl from the `#[pin]` fields alone, so the
        // output held by `Done` does not constrain it -- only `Fut` does.
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

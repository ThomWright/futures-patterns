//! Let a finished future be polled harmlessly.
//!
//! `Fuse` wraps a future so that polling it after completion is well defined: it
//! returns `Poll::Pending` rather than panicking or re-polling something that has
//! already finished.
//!
//! # What happens, and when
//!
//! Two different polls are involved, which "always pending afterwards" glosses over:
//!
//! - On the poll where the inner future completes, `Fuse` returns
//!   `Ready(inner_output)` and drops the inner future. The output passes straight
//!   through, unchanged, and is delivered exactly once.
//! - On every poll after that, there is nothing left to poll, and the answer is
//!   `Pending`.
//!
//! # This `Pending` registers no waker
//!
//! Nothing is watching, because nothing can ever make progress. So awaiting a finished
//! `Fuse` on its own parks the task forever, exactly like
//! [`basic::pending`](crate::basic::pending).
//!
//! `Fuse` is therefore a component for `select!`-style loops rather than a general
//! "safe to poll again" wrapper. Inside such a loop the other branches register wakers
//! and the task still gets woken; alone, it hangs.
//!
//! # Ask before polling
//!
//! [`FusedFuture::is_terminated`] lets a loop skip a finished branch instead of
//! polling it and discarding the `Pending`. That is the intended mechanism; the
//! graceful `Pending` is the safety net for when a branch gets polled anyway.
//!
//! # Compared with `MaybeDone`
//!
//! Both make polling after completion safe, for opposite consumers.
//! [`MaybeDone`](crate::state_machine::maybe_done) withholds the output at completion
//! and keeps it for `take_output`, so a join can learn every branch is done before
//! collecting anything. `Fuse` hands the output over at completion and keeps nothing,
//! so a select loop gets the value the moment it appears and the branch then falls
//! silent.
//!
//! Neither generalises the other: a join cannot store outputs in a `Fuse`, and a select
//! loop over `MaybeDone` would spin on branches answering `Ready(())`.
//!
//! # Relationship to `OptionFuture`
//!
//! The shape is [`OptionFuture`](crate::basic::wrapper::OptionFuture) -- a `#[pin]`
//! field holding `Option<Fut>` -- and completion clears the slot with `Pin::set`, the
//! same move as
//! [`OptionFuture::clear`](crate::basic::wrapper::OptionFuture::clear). Only the empty
//! case differs: `OptionFuture` reports `Ready(None)`, while `Fuse` reports `Pending`.
//!
//! # Example
//!
//! ```
//! use futures_patterns::composition::fuse::fuse;
//! use futures_patterns::fused::FusedFuture;
//! use futures_patterns::testing::poll_once;
//! use std::task::{Poll, Waker};
//!
//! let mut fused = Box::pin(fuse(async { 42 }));
//! assert!(!fused.is_terminated());
//!
//! assert_eq!(poll_once(fused.as_mut(), Waker::noop()), Poll::Ready(42));
//! assert!(fused.is_terminated());
//!
//! // Polling again is harmless, and yields nothing ever again.
//! assert_eq!(poll_once(fused.as_mut(), Waker::noop()), Poll::Pending);
//! ```
//!
//! Follows `futures-util/src/future/future/fuse.rs`; see NOTICE.md.

use crate::fused::FusedFuture;
use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// Future for the [`fuse`] function.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Fuse<Fut> {
        // Emptied on completion, which both drops the finished future and records
        // that there is nothing left to poll.
        #[pin]
        inner: Option<Fut>,
    }
}

/// Wraps a future so it can be polled after completion.
pub fn fuse<Fut: Future>(future: Fut) -> Fuse<Fut> {
    Fuse {
        inner: Some(future),
    }
}

impl<Fut> Fuse<Fut> {
    /// Creates a `Fuse` that is already finished.
    ///
    /// Useful as a placeholder branch in a `select!` loop: it never fires, and can be
    /// replaced later with a real future via [`Pin::set`].
    pub fn terminated() -> Self {
        Fuse { inner: None }
    }
}

impl<Fut: Future> FusedFuture for Fuse<Fut> {
    fn is_terminated(&self) -> bool {
        self.inner.is_none()
    }
}

impl<Fut: Future> Future for Fuse<Fut> {
    type Output = Fut::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Fut::Output> {
        match self.as_mut().project().inner.as_pin_mut() {
            Some(inner) => inner.poll(cx).map(|output| {
                // Clearing here is what makes a later poll safe, and it drops the
                // finished future rather than holding it until `Fuse` itself is
                // dropped. `set` replaces in place, so nothing is moved out of the pin.
                self.project().inner.set(None);
                output
            }),
            // Deliberately no `cx` use: nothing can make progress, so there is nothing
            // to register a waker for. See the module docs.
            None => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Fuse, fuse};
    use crate::basic::ready::ready;
    use crate::fused::FusedFuture;
    use crate::state_machine::two_state::CountDown;
    use crate::testing::{CountingWaker, poll_once};
    use std::sync::Arc;
    use std::task::{Poll, Waker};

    #[test]
    fn the_completing_poll_hands_the_output_through() {
        let mut fut = Box::pin(fuse(ready(42)));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(42));
    }

    #[test]
    fn every_later_poll_is_pending() {
        let mut fut = Box::pin(fuse(ready(42)));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(42));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
    }

    #[test]
    fn a_later_poll_registers_no_waker() {
        // Which is why awaiting a finished Fuse on its own hangs: nothing has been
        // asked to wake the task, and nothing ever will.
        let waker = CountingWaker::new();
        let mut fut = Box::pin(fuse(ready(42)));

        assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Ready(42));
        assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Pending);
        assert_eq!(waker.count(), 0);
    }

    #[test]
    fn does_not_repoll_the_inner_future() {
        // `ready` panics if polled twice, so surviving proves the slot was emptied
        // rather than the finished future being polled again.
        let mut fut = Box::pin(fuse(ready(42)));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(42));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
    }

    #[test]
    fn forwards_pending_from_the_inner_future() {
        let waker = CountingWaker::new();
        let mut fut = Box::pin(fuse(CountDown::new(2)));

        assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Pending);
        assert_eq!(waker.count(), 1, "the inner future's wake is not swallowed");
        assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Pending);
        assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Ready(2));
    }

    #[test]
    fn is_terminated_flips_on_completion() {
        let mut fut = Box::pin(fuse(ready(42)));
        assert!(!fut.is_terminated());

        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(42));
        assert!(fut.is_terminated());
    }

    #[test]
    fn terminated_starts_finished() {
        let mut fut = Box::pin(Fuse::<crate::basic::ready::Ready<i32>>::terminated());
        assert!(fut.is_terminated());
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
    }

    #[test]
    fn completion_drops_the_inner_future() {
        // The Arc clone held by the inner future is released at completion, not when
        // the Fuse itself is dropped.
        let tracker = Arc::new(());
        let held = Arc::clone(&tracker);
        let inner = async move {
            let _held = held;
            1
        };

        let mut fut = Box::pin(fuse(inner));
        assert_eq!(Arc::strong_count(&tracker), 2);

        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(1));
        assert_eq!(Arc::strong_count(&tracker), 1, "dropped on completion");
    }

    #[test]
    fn a_loop_can_skip_a_terminated_branch() {
        // The intended use: ask rather than poll and discard the Pending.
        let mut a = Box::pin(fuse(CountDown::new(1)));
        let mut b = Box::pin(fuse(CountDown::new(3)));
        let mut finished = Vec::new();

        for _ in 0..10 {
            if !a.is_terminated()
                && let Poll::Ready(v) = poll_once(a.as_mut(), Waker::noop())
            {
                finished.push(("a", v));
            }
            if !b.is_terminated()
                && let Poll::Ready(v) = poll_once(b.as_mut(), Waker::noop())
            {
                finished.push(("b", v));
            }
        }

        assert_eq!(finished, vec![("a", 1), ("b", 3)]);
    }
}

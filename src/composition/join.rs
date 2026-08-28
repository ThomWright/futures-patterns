//! Wait for two futures to finish, and collect both outputs.
//!
//! Where [`race`](crate::composition::race) returns as soon as one branch finishes,
//! `Join` waits for every branch and yields all of their outputs together.
//!
//! # Why this needs `MaybeDone`
//!
//! Awaiting the branches one after another -- `(a.await, b.await)` -- runs them in
//! sequence, which defeats the point. To run them concurrently, `Join` must poll both
//! on every wakeup, and that creates three problems at once:
//!
//! 1. A branch that has already finished must not be polled again, but the loop still
//!    has to poll *something* for it each round.
//! 2. Its output has to be kept somewhere, because the result tuple cannot be
//!    returned until the slowest branch lands.
//! 3. All the outputs have to be harvested at the end, in one go.
//!
//! [`MaybeDone`](crate::state_machine::maybe_done) exists for exactly this: its `Done`
//! state absorbs further polls without touching the inner future, holds the output in
//! the meantime, and hands it over via `take_output`. Those three states map one to
//! one onto the three problems above.
//!
//! # Why polling a finished branch is allowed here
//!
//! `Join` polls both branches every round, including ones that have already finished,
//! which looks like the thing `Future`'s docs warn against. It is not: the branches
//! are [`MaybeDone`](crate::state_machine::maybe_done), and that type documents a poll
//! in its `Done` state as a harmless `Ready(())`. The base contract is a floor, and a
//! concrete type may promise more; see [`fused`](crate::fused).
//!
//! A `select!` loop cannot do this, because it is generic over branches whose
//! behaviour after completion is unspecified, so it asks
//! [`FusedFuture::is_terminated`](crate::fused::FusedFuture::is_terminated) first.
//! `Join` needs no such question, and neither `futures` nor `tokio` asks it in their
//! own join implementations.
//!
//! # The short-circuit trap
//!
//! Both branches must be polled every round. It is easy to write this instead:
//!
//! ```text
//! if a.poll(cx).is_ready() && b.poll(cx).is_ready() { ... }
//! ```
//!
//! `&&` short-circuits, so while `a` is pending, `b` is never polled at all -- it
//! never registers a waker, and never makes progress. The join then completes only as
//! fast as `a` allows, and hangs outright if `b` was the branch that would have woken
//! the task. Polling into separate bindings first, as below, avoids it.
//!
//! # How this differs from `tokio::join!`
//!
//! Tokio's version is a macro, so it handles any number of branches rather than
//! exactly two, and it rotates which branch it polls first on each pass. That rotation
//! matters under load: a branch that always exhausts the cooperative budget would
//! otherwise starve everything declared after it. This implementation always polls
//! `a` before `b`, which is simpler and fine when no branch monopolises the budget.
//!
//! # Example
//!
//! ```
//! use futures_patterns::composition::join::join;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let (a, b) = join(async { 1 }, async { "two" }).await;
//! assert_eq!((a, b), (1, "two"));
//! # }
//! ```

use crate::state_machine::maybe_done::{MaybeDone, maybe_done};
use pin_project_lite::pin_project;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// Future for the [`join`] function.
    ///
    /// Completes once both branches have completed, yielding both outputs.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Join<A, B>
    where
        A: Future,
        B: Future,
    {
        // Each branch is wrapped so that a completed one can be polled harmlessly
        // and its output kept until the other finishes.
        #[pin]
        a: MaybeDone<A>,
        #[pin]
        b: MaybeDone<B>,
    }
}

// Written out rather than derived. `#[derive(Debug)]` would bound only `A: Debug`,
// but the fields are `MaybeDone<A>`, whose own Debug additionally needs
// `A::Output: Debug`. Derive cannot infer bounds on associated types.
impl<A, B> fmt::Debug for Join<A, B>
where
    A: Future + fmt::Debug,
    B: Future + fmt::Debug,
    A::Output: fmt::Debug,
    B::Output: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Join")
            .field("a", &self.a)
            .field("b", &self.b)
            .finish()
    }
}

/// Waits for both futures to complete, returning both outputs.
///
/// The branches are polled concurrently rather than in sequence, so the join takes
/// as long as the slower branch rather than the sum of the two.
///
/// # Example
///
/// ```
/// use futures_patterns::composition::join::join;
/// use futures_patterns::state_machine::two_state::CountDown;
///
/// # #[tokio::main]
/// # async fn main() {
/// // Three polls and five polls, run concurrently rather than one after the other.
/// let (a, b) = join(CountDown::new(3), CountDown::new(5)).await;
/// assert_eq!((a, b), (3, 5));
/// # }
/// ```
pub fn join<A, B>(a: A, b: B) -> Join<A, B>
where
    A: Future,
    B: Future,
{
    Join {
        a: maybe_done(a),
        b: maybe_done(b),
    }
}

impl<A, B> Future for Join<A, B>
where
    A: Future,
    B: Future,
{
    type Output = (A::Output, B::Output);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        // Poll into bindings first, so that both branches are polled whatever the
        // first one returns. See the note on short-circuiting in the module docs.
        let a_done = this.a.as_mut().poll(cx).is_ready();
        let b_done = this.b.as_mut().poll(cx).is_ready();

        if !(a_done && b_done) {
            // Any branch that returned Pending has registered the waker, so there is
            // nothing further to arrange here.
            return Poll::Pending;
        }

        // Both are Done, so both outputs are waiting to be collected. This is the
        // only point at which either is moved out.
        Poll::Ready((
            this.a.take_output().expect("branch a reported ready"),
            this.b.take_output().expect("branch b reported ready"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::join;
    use crate::basic::pending::pending;
    use crate::basic::poll_fn::poll_fn;
    use crate::basic::ready::ready;
    use crate::state_machine::two_state::CountDown;
    use crate::testing::{CountingWaker, poll_once, poll_until_ready};
    use std::task::{Poll, Waker};

    #[test]
    fn yields_both_outputs() {
        let mut fut = Box::pin(join(ready(1), ready(2)));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready((1, 2)));
    }

    #[test]
    fn branches_may_have_different_output_types() {
        let mut fut = Box::pin(join(ready(1), ready("two")));
        assert_eq!(
            poll_once(fut.as_mut(), Waker::noop()),
            Poll::Ready((1, "two"))
        );
    }

    #[test]
    fn is_pending_until_the_slower_branch_finishes() {
        let mut fut = Box::pin(join(ready(1), CountDown::new(2)));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready((1, 2)));
    }

    #[test]
    fn polls_the_second_branch_even_while_the_first_is_pending() {
        // The short-circuit trap: with `&&` between the two polls, `b` would never be
        // polled at all while `a` is pending. A branch that panics on poll makes the
        // difference observable.
        let mut polled_b = false;
        {
            let b = poll_fn(|_cx| {
                polled_b = true;
                Poll::Pending::<u8>
            });
            let mut fut = Box::pin(join(pending::<u8>(), b));
            assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
        }
        assert!(polled_b, "both branches must be polled every round");
    }

    #[test]
    fn both_branches_register_the_waker() {
        // Each CountDown wakes once per pending poll, so two wakes from a single poll
        // of Join proves both branches were polled and both registered.
        let waker = CountingWaker::new();
        let mut fut = Box::pin(join(CountDown::new(5), CountDown::new(5)));

        assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Pending);
        assert_eq!(waker.count(), 2);
    }

    #[test]
    fn does_not_repoll_a_branch_that_has_already_finished() {
        // `ready` panics if polled twice, so surviving the remaining rounds proves
        // MaybeDone's Done state is absorbing the polls rather than forwarding them.
        let mut fut = Box::pin(join(ready(1), CountDown::new(3)));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready((1, 3)));
    }

    #[test]
    fn runs_the_branches_concurrently_not_in_sequence() {
        // Three polls and five polls. Concurrently that is max(3, 5) + 1 rounds; in
        // sequence it would be 9.
        let (output, polls) =
            poll_until_ready(Box::pin(join(CountDown::new(3), CountDown::new(5))).as_mut(), 20);
        assert_eq!(output, (3, 5));
        assert_eq!(polls, 6);
    }

    #[tokio::test]
    async fn completes_when_awaited_normally() {
        assert_eq!(join(async { 1 }, async { "two" }).await, (1, "two"));
    }
}

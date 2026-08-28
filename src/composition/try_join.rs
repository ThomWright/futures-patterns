//! Wait for two fallible futures, or return the first error.
//!
//! Like [`join`](crate::composition::join), but for branches producing `Result`. If
//! both succeed the outputs are returned together; if either fails, that error is
//! returned immediately, without waiting for the other branch.
//!
//! # Abandoning the other branch
//!
//! Short-circuiting is the whole point, and it has a consequence worth being explicit
//! about: when one branch fails, the other is simply *dropped*, wherever it had got
//! to. There is no signal to it and no chance for it to finish.
//!
//! For an in-memory computation that is free. For a branch holding a database
//! transaction or a half-written request it is not, and whether dropping is safe is
//! exactly the question of cancellation safety. Dropping a future at an arbitrary
//! await point is how all cancellation in Rust works -- `timeout` does the same thing
//! when its deadline passes.
//!
//! # Why `output_mut` exists
//!
//! A branch that completes has to be *inspected* before the join knows what to do:
//!
//! - if it produced an error, take it and return
//! - if it produced a value, leave the value parked for the final harvest
//!
//! That is a peek, not a move, which is what
//! [`MaybeDone::output_mut`](crate::state_machine::maybe_done::MaybeDone::output_mut)
//! is for. `take_output` would move the value out, and an `Ok` value moved out early
//! has nowhere to live until the other branch finishes.
//!
//! # Error bias
//!
//! Branches are polled in order, and `a`'s error is returned if both fail on the same
//! poll -- the same left-bias [`race`](crate::composition::race) has, for the same
//! reason. When `a` fails, `b` is not polled at all on that round.
//!
//! # Example
//!
//! ```
//! use futures_patterns::composition::try_join::try_join;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let ok = try_join(async { Ok::<_, &str>(1) }, async { Ok::<_, &str>(2) }).await;
//! assert_eq!(ok, Ok((1, 2)));
//!
//! let failed = try_join(async { Ok::<i32, &str>(1) }, async { Err::<i32, _>("nope") }).await;
//! assert_eq!(failed, Err("nope"));
//! # }
//! ```

use crate::state_machine::maybe_done::{MaybeDone, maybe_done};
use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// Future for the [`try_join`] function.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct TryJoin<A, B>
    where
        A: Future,
        B: Future,
    {
        #[pin]
        a: MaybeDone<A>,
        #[pin]
        b: MaybeDone<B>,
    }
}

/// Waits for both futures to succeed, or returns the first error.
///
/// Both branches must fail with the same error type.
pub fn try_join<A, B, T, U, E>(a: A, b: B) -> TryJoin<A, B>
where
    A: Future<Output = Result<T, E>>,
    B: Future<Output = Result<U, E>>,
{
    TryJoin {
        a: maybe_done(a),
        b: maybe_done(b),
    }
}

/// Returns the branch's error if it has completed with one, leaving an `Ok` parked.
///
/// The peek through `output_mut` matters: taking the output unconditionally would
/// move an `Ok` value out with nowhere to put it until the other branch lands.
fn take_error<Fut, T, E>(mut branch: Pin<&mut MaybeDone<Fut>>) -> Option<E>
where
    Fut: Future<Output = Result<T, E>>,
{
    if !matches!(branch.as_mut().output_mut(), Some(Err(_))) {
        return None;
    }

    match branch.take_output() {
        Some(Err(err)) => Some(err),
        _ => unreachable!("an error was observed on this branch"),
    }
}

impl<A, B, T, U, E> Future for TryJoin<A, B>
where
    A: Future<Output = Result<T, E>>,
    B: Future<Output = Result<U, E>>,
{
    type Output = Result<(T, U), E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        let a_done = this.a.as_mut().poll(cx).is_ready();
        if let Some(err) = take_error(this.a.as_mut()) {
            // Returning here abandons `b` wherever it had got to; it is dropped with
            // the TryJoin. See the module docs on cancellation.
            return Poll::Ready(Err(err));
        }

        let b_done = this.b.as_mut().poll(cx).is_ready();
        if let Some(err) = take_error(this.b.as_mut()) {
            return Poll::Ready(Err(err));
        }

        if !(a_done && b_done) {
            return Poll::Pending;
        }

        // Both succeeded, so both values are parked and can be collected.
        let (Some(Ok(a)), Some(Ok(b))) = (this.a.take_output(), this.b.take_output()) else {
            unreachable!("both branches completed successfully")
        };
        Poll::Ready(Ok((a, b)))
    }
}

#[cfg(test)]
mod tests {
    use super::try_join;
    use crate::basic::pending::pending;
    use crate::basic::poll_fn::poll_fn;
    use crate::basic::ready::ready;
    use crate::state_machine::two_state::CountDown;
    use crate::testing::poll_once;
    use std::sync::Arc;
    use std::task::{Poll, Waker};

    type E = &'static str;

    #[test]
    fn yields_both_values_when_both_succeed() {
        let mut fut = Box::pin(try_join(ready(Ok::<_, E>(1)), ready(Ok::<_, E>(2))));
        assert_eq!(
            poll_once(fut.as_mut(), Waker::noop()),
            Poll::Ready(Ok((1, 2)))
        );
    }

    #[test]
    fn returns_the_error_from_the_first_branch() {
        let mut fut = Box::pin(try_join(ready(Err::<i32, E>("bad")), ready(Ok::<_, E>(2))));
        assert_eq!(
            poll_once(fut.as_mut(), Waker::noop()),
            Poll::Ready(Err("bad"))
        );
    }

    #[test]
    fn returns_the_error_from_the_second_branch() {
        let mut fut = Box::pin(try_join(ready(Ok::<_, E>(1)), ready(Err::<i32, E>("bad"))));
        assert_eq!(
            poll_once(fut.as_mut(), Waker::noop()),
            Poll::Ready(Err("bad"))
        );
    }

    #[test]
    fn fails_fast_without_waiting_for_a_pending_branch() {
        // The point of short-circuiting: `b` never completes, but the error still
        // comes back on the first poll.
        let mut fut = Box::pin(try_join(ready(Err::<i32, E>("bad")), pending::<Result<i32, E>>()));
        assert_eq!(
            poll_once(fut.as_mut(), Waker::noop()),
            Poll::Ready(Err("bad"))
        );
    }

    #[test]
    fn does_not_poll_the_second_branch_once_the_first_has_failed() {
        let b = poll_fn(|_cx| -> Poll<Result<i32, E>> { panic!("b must not be polled") });
        let mut fut = Box::pin(try_join(ready(Err::<i32, E>("bad")), b));
        assert_eq!(
            poll_once(fut.as_mut(), Waker::noop()),
            Poll::Ready(Err("bad"))
        );
    }

    #[test]
    fn waits_for_both_branches_while_they_are_succeeding() {
        let mut fut = Box::pin(try_join(
            ready(Ok::<_, E>(1)),
            crate::composition::map::map(CountDown::new(2), Ok::<_, E>),
        ));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
        assert_eq!(
            poll_once(fut.as_mut(), Waker::noop()),
            Poll::Ready(Ok((1, 2)))
        );
    }

    #[test]
    fn the_abandoned_branch_is_dropped() {
        // Cancellation, made observable. The surviving branch holds an Arc clone; once
        // the failed TryJoin is dropped, so is the branch, and the count falls back.
        let tracker = Arc::new(());
        let held = Arc::clone(&tracker);

        let b = async move {
            let _held = held;
            std::future::pending::<Result<i32, E>>().await
        };

        let mut fut = Box::pin(try_join(ready(Err::<i32, E>("bad")), b));
        assert_eq!(
            poll_once(fut.as_mut(), Waker::noop()),
            Poll::Ready(Err("bad"))
        );
        assert_eq!(Arc::strong_count(&tracker), 2, "still alive inside the join");

        drop(fut);
        assert_eq!(Arc::strong_count(&tracker), 1, "dropped with the join");
    }

    #[tokio::test]
    async fn completes_when_awaited_normally() {
        assert_eq!(
            try_join(async { Ok::<_, E>(1) }, async { Ok::<_, E>(2) }).await,
            Ok((1, 2))
        );
    }
}

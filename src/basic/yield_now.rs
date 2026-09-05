//! A future that returns `Pending` once, then completes.
//!
//! Awaiting it hands control back to the executor: the task is re-queued,
//! whatever else is waiting gets a turn, and then it resumes and finishes. A
//! long CPU-bound loop inside a task starves every other task sharing that
//! thread, and a `yield_now().await` in the loop body is what breaks it up.
//!
//! # Why it needs to wake itself
//!
//! Returning `Pending` is a promise to arrange a wake. Usually something else
//! keeps that promise -- a timer, another thread -- and the future returns
//! `Pending` knowing it will be woken later. Nothing else knows this future
//! exists, so it wakes itself before returning: there is no condition to wait
//! for, only a turn to give up.
//!
//! [`crate::basic::pending`] also returns `Pending` and deliberately does not
//! wake, because no progress is ever possible there. Delete the `wake_by_ref`
//! below and this future becomes that -- a task nobody will ever poll again.
//!
//! # Example
//!
//! ```
//! use futures_patterns::basic::yield_now::yield_now;
//! use std::sync::Arc;
//! use std::sync::atomic::{AtomicBool, Ordering};
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() {
//! let ran = Arc::new(AtomicBool::new(false));
//!
//! let flag = ran.clone();
//! tokio::spawn(async move { flag.store(true, Ordering::SeqCst) });
//!
//! // Spawning queues the task; it cannot run while this one holds the thread.
//! assert!(!ran.load(Ordering::SeqCst));
//!
//! yield_now().await;
//!
//! assert!(ran.load(Ordering::SeqCst));
//! # }
//! ```

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for the [`yield_now`] function.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct YieldNow {
    yielded: bool,
}

/// Gives up the thread once, so anything else waiting to run can.
///
/// # Example
///
/// ```
/// use futures_patterns::basic::yield_now::yield_now;
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() {
/// yield_now().await;
/// # }
/// ```
pub fn yield_now() -> YieldNow {
    YieldNow { yielded: false }
}

impl Future for YieldNow {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.yielded {
            return Poll::Ready(());
        }
        self.yielded = true;

        // The `Pending` below is a promise to arrange a wake, and there is nobody else
        // to keep it.
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::yield_now;
    use crate::testing::{CountingWaker, poll_once};
    use std::task::Poll;

    #[test]
    fn is_pending_on_the_first_poll_and_wakes_the_task() {
        let waker = CountingWaker::new();
        let mut fut = Box::pin(yield_now());

        assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Pending);
        // Without the wake, the task is never polled again and hangs.
        assert_eq!(waker.count(), 1);
    }

    #[test]
    fn is_ready_on_the_second_poll_and_does_not_wake_again() {
        let waker = CountingWaker::new();
        let mut fut = Box::pin(yield_now());

        let _ = poll_once(fut.as_mut(), &waker.waker());
        assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Ready(()));
        // Waking here would ask for a poll that has nothing left to do.
        assert_eq!(waker.count(), 1);
    }

    #[test]
    fn stays_ready_when_polled_after_completion() {
        // Nothing is consumed on completion, so the extra poll has an answer to give.
        let waker = CountingWaker::new();
        let mut fut = Box::pin(yield_now());

        let _ = poll_once(fut.as_mut(), &waker.waker());
        assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Ready(()));
        assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Ready(()));
    }
}

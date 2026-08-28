//! A future woken by another thread.
//!
//! This builds a one-shot channel: a [`Sender`] hands over a single value, and a
//! [`Receiver`] is a future that completes when the value arrives. The value can be
//! sent from any thread, so unlike every other pattern here, its readiness comes from
//! outside.
//!
//! # The four rules
//!
//! Almost every leaf future obeys the same four rules, and each one exists because of
//! a specific way things break.
//!
//! ## 1. Store the waker before returning `Pending`
//!
//! `Poll::Pending` is a promise that the task will be woken later. The only way to
//! keep that promise is to have stored the waker somewhere the producer can find it.
//! Forget this and the task parks forever -- the bug [`basic::pending`] has by
//! design.
//!
//! ## 2. Check the state and store the waker without a gap
//!
//! This is the subtle one. Consider a consumer that checks first and registers after,
//! with no lock:
//!
//! ```text
//! consumer: reads state, finds it empty
//! producer:                              writes value, looks for a waker, finds none
//! consumer: stores its waker, returns Pending
//! ```
//!
//! The value is there and the task is asleep, with nothing left to wake it. This is a
//! lost wakeup: a genuine race rather than a coding slip, needing the two halves to
//! interleave at exactly that point.
//!
//! Holding one lock across both the check and the store closes the window, which is
//! what this implementation does. Doing it *without* a lock is harder, and is why
//! `tokio::sync::task::AtomicWaker` exists: it coordinates the two halves with a
//! two-bit state machine so a registering thread and a waking thread cannot lose each
//! other. Its docs give the rule as "consumers should call `register` before checking
//! the result of a computation and producers should call `wake` after producing" --
//! the ordering matters precisely because there is no lock to make it moot.
//!
//! ## 3. Keep the newest waker
//!
//! The same future can be polled from a *different* task than last time -- what
//! `AtomicWaker`'s docs call a consumer "in the process of being migrated to a new
//! logical task". So the waker handed in on this poll supersedes whatever was stored
//! before: waking a stale one notifies a task that is no longer waiting, while the task
//! that *is* waiting hears nothing.
//!
//! `AtomicWaker` states the same policy -- "if a new `Waker` instance is produced by
//! calling `register` before an existing one is consumed, then the existing one is
//! overwritten". [`Waker::will_wake`] lets you skip the clone when the task has not
//! actually changed, which is the common case.
//!
//! ## 4. Wake after releasing the lock
//!
//! This one is about contention rather than lost wakeups. Wake while still holding the
//! lock and the woken task can be scheduled immediately, only to block on the lock you
//! have not let go of yet.
//!
//! It can be worse than slow. If the executor polls the woken task inline on the
//! waking thread, that poll will try to take a `Mutex` the same thread already holds,
//! and `std`'s mutexes are not reentrant, so it deadlocks.
//!
//! Tokio's `Notify` avoids all this explicitly: it takes the waker out under the lock,
//! then calls `drop(waiters)` before `waker.wake()`.
//!
//! # What this simplifies
//!
//! One `Mutex` around the whole state, where production code uses atomics and a
//! lock-free waker cell. The rules above are the real ones; the lock is the shortcut.
//!
//! [`basic::pending`]: crate::basic::pending
//! [`Waker::will_wake`]: std::task::Waker::will_wake

use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

/// Error returned when the sender is dropped without sending a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Closed;

impl fmt::Display for Closed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sender dropped without sending a value")
    }
}

impl Error for Closed {}

/// The state both halves share.
///
/// Everything lives behind one `Mutex`, so that checking `value` and storing `waker`
/// cannot be interleaved with a send. See the module docs on rule 2.
#[derive(Debug)]
struct Shared<T> {
    /// The sent value, if it has arrived and not yet been taken.
    value: Option<T>,

    /// The waker of the task waiting on the value, if one is waiting.
    waker: Option<Waker>,

    /// Set when the sender is dropped, so the receiver can stop waiting for a value
    /// that will never arrive.
    closed: bool,
}

/// Creates a linked [`Sender`] and [`Receiver`].
///
/// # Example
///
/// ```
/// use futures_patterns::waking::shared_state::channel;
///
/// # #[tokio::main]
/// # async fn main() {
/// let (tx, rx) = channel::<u32>();
///
/// std::thread::spawn(move || {
///     tx.send(7);
/// });
///
/// assert_eq!(rx.await, Ok(7));
/// # }
/// ```
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let shared = Arc::new(Mutex::new(Shared {
        value: None,
        waker: None,
        closed: false,
    }));

    (
        Sender {
            shared: Arc::clone(&shared),
        },
        Receiver { shared },
    )
}

/// The producing half. Sends at most one value.
#[derive(Debug)]
pub struct Sender<T> {
    shared: Arc<Mutex<Shared<T>>>,
}

impl<T> Sender<T> {
    /// Sends the value, waking the receiving task if it is waiting.
    ///
    /// Takes `self` by value, so a channel carries at most one value.
    pub fn send(self, value: T) {
        // The waker is taken out under the lock but woken after it is released; see
        // rule 4. Waking while holding the lock invites the woken task to block on it.
        let waker = {
            let mut shared = self.lock();
            shared.value = Some(value);
            shared.waker.take()
        };

        if let Some(waker) = waker {
            waker.wake();
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Shared<T>> {
        // A poisoned lock means another thread panicked mid-update. Nothing here can
        // repair that, so propagate it rather than continue on torn state.
        self.shared.lock().expect("shared state poisoned")
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        // Without this, dropping the sender would leave the receiver parked forever
        // waiting for a value that can no longer arrive.
        let waker = {
            let mut shared = self.lock();
            shared.closed = true;
            shared.waker.take()
        };

        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

/// The consuming half: a future that completes when the value arrives.
///
/// Completes with `Err(`[`Closed`]`)` if the [`Sender`] is dropped first.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Receiver<T> {
    shared: Arc<Mutex<Shared<T>>>,
}

impl<T> Future for Receiver<T> {
    type Output = Result<T, Closed>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut shared = self.shared.lock().expect("shared state poisoned");

        if let Some(value) = shared.value.take() {
            return Poll::Ready(Ok(value));
        }

        if shared.closed {
            return Poll::Ready(Err(Closed));
        }

        // Rule 3: keep the newest waker, since the future may be polled by a
        // different task than last time. `will_wake` avoids a clone when the task
        // has not changed, which is the common case.
        match &shared.waker {
            Some(existing) if existing.will_wake(cx.waker()) => {}
            _ => shared.waker = Some(cx.waker().clone()),
        }

        Poll::Pending
    }
}

// A leaf future like this holds no self-referential state -- just a handle to shared
// state -- so it is `Unpin`, and can be polled through `Pin::new` without boxing.
// That is typical: pinning becomes awkward for the combinators built *on top* of
// leaves, not for the leaves themselves.

#[cfg(test)]
mod tests {
    use super::{Closed, channel};
    use crate::testing::{CountingWaker, poll_once};
    use std::pin::Pin;
    use std::task::Poll;

    #[test]
    fn a_value_sent_before_the_first_poll_is_ready_immediately() {
        let (tx, mut rx) = channel::<u32>();
        tx.send(7);

        let waker = CountingWaker::new();
        assert_eq!(
            poll_once(Pin::new(&mut rx), &waker.waker()),
            Poll::Ready(Ok(7))
        );
        // Nothing was waiting, so nothing needed waking.
        assert_eq!(waker.count(), 0);
    }

    #[test]
    fn parks_until_a_value_arrives_then_wakes_once() {
        let (tx, mut rx) = channel::<u32>();
        let waker = CountingWaker::new();

        assert_eq!(poll_once(Pin::new(&mut rx), &waker.waker()), Poll::Pending);
        assert_eq!(waker.count(), 0, "no wake until there is something to report");

        tx.send(7);
        assert_eq!(waker.count(), 1, "sending must wake the parked task");

        assert_eq!(
            poll_once(Pin::new(&mut rx), &waker.waker()),
            Poll::Ready(Ok(7))
        );
    }

    #[test]
    fn only_the_most_recently_registered_waker_is_woken() {
        // Rule 3. An executor may poll a future from a different task than before, so
        // a stale waker would notify a task that is no longer waiting.
        let (tx, mut rx) = channel::<u32>();
        let first = CountingWaker::new();
        let second = CountingWaker::new();

        assert_eq!(poll_once(Pin::new(&mut rx), &first.waker()), Poll::Pending);
        assert_eq!(poll_once(Pin::new(&mut rx), &second.waker()), Poll::Pending);

        tx.send(7);

        assert_eq!(first.count(), 0, "the superseded waker must not be woken");
        assert_eq!(second.count(), 1);
    }

    #[test]
    fn a_wake_from_another_thread_reaches_the_waiting_task() {
        // Deterministic: `join` guarantees the send completed before the assertion,
        // so this genuinely exercises the cross-thread path rather than racing it.
        let (tx, mut rx) = channel::<u32>();
        let waker = CountingWaker::new();

        assert_eq!(poll_once(Pin::new(&mut rx), &waker.waker()), Poll::Pending);

        std::thread::spawn(move || tx.send(7)).join().expect("sender thread panicked");

        assert_eq!(waker.count(), 1);
        assert_eq!(
            poll_once(Pin::new(&mut rx), &waker.waker()),
            Poll::Ready(Ok(7))
        );
    }

    #[test]
    fn dropping_the_sender_closes_the_channel() {
        let (tx, mut rx) = channel::<u32>();
        let waker = CountingWaker::new();

        assert_eq!(poll_once(Pin::new(&mut rx), &waker.waker()), Poll::Pending);

        drop(tx);
        assert_eq!(waker.count(), 1, "closing must wake the parked task too");

        assert_eq!(
            poll_once(Pin::new(&mut rx), &waker.waker()),
            Poll::Ready(Err(Closed))
        );
    }

    #[test]
    fn a_value_sent_before_the_sender_drops_still_arrives() {
        // `send` consumes the sender, so the Drop that marks the channel closed runs
        // immediately afterwards. The value must win over the close.
        let (tx, mut rx) = channel::<u32>();
        tx.send(7);

        let waker = CountingWaker::new();
        assert_eq!(
            poll_once(Pin::new(&mut rx), &waker.waker()),
            Poll::Ready(Ok(7))
        );
    }

    #[test]
    fn closed_reports_a_readable_error() {
        assert_eq!(
            Closed.to_string(),
            "sender dropped without sending a value"
        );
    }

    #[tokio::test]
    async fn completes_when_awaited_normally() {
        // The same thing end to end, driven by a real executor rather than by hand.
        let (tx, rx) = channel::<u32>();

        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            tx.send(7);
        });

        assert_eq!(rx.await, Ok(7));
    }

    #[test]
    fn the_receiver_is_unpin() {
        fn assert_unpin<T: Unpin>() {}
        // Holds only an Arc, so no boxing is needed to poll it.
        assert_unpin::<super::Receiver<std::marker::PhantomPinned>>();
    }
}

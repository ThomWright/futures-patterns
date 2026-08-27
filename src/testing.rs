//! Helpers for testing futures by polling them directly.
//!
//! Most async tests drive a future with `.await` and only observe its final
//! output. That hides everything interesting about an implementation: how many
//! times it was polled, whether it registered a waker, whether it woke the task
//! when it should have. A future can be badly wrong and still produce the right
//! value under `.await`.
//!
//! These helpers poll futures by hand so tests can assert on the poll sequence
//! itself, without a runtime.
//!
//! # Choosing a waker
//!
//! - [`Waker::noop`] (from `std`) discards wakes. Use it when the test only
//!   cares about the `Poll` values.
//! - [`CountingWaker`] records how many times it was woken. Use it to pin down
//!   waker behaviour, which is where the subtle bugs live: a future that returns
//!   `Pending` without arranging a wake hangs forever, and one that wakes when it
//!   shouldn't burns CPU.
//!
//! # Example
//!
//! ```
//! use futures_patterns::testing::{poll_once, CountingWaker};
//! use futures_patterns::state_machine::two_state::CountDown;
//! use std::task::Poll;
//!
//! let waker = CountingWaker::new();
//! let mut future = Box::pin(CountDown::new(2));
//!
//! // The first poll is not ready, but it must arrange to be polled again.
//! assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Pending);
//! assert_eq!(waker.count(), 1);
//! ```

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};

/// A [`Waker`] that counts how many times it has been woken.
///
/// Built on [`Wake`], so it needs no `unsafe` code — implementing that trait for
/// an `Arc<W>` is the supported way to make a waker by hand.
#[derive(Debug, Default)]
pub struct CountingWaker {
    wakes: AtomicUsize,
}

impl CountingWaker {
    /// Creates a new waker with a wake count of zero.
    ///
    /// Returns an `Arc` because [`Waker`] is built from `Arc<W: Wake>`, and the
    /// test needs to keep a handle to read the count back.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Returns a [`Waker`] backed by this counter.
    ///
    /// Can be called repeatedly; every returned waker increments the same count.
    pub fn waker(self: &Arc<Self>) -> Waker {
        Waker::from(Arc::clone(self))
    }

    /// Returns the number of times this waker has been woken.
    pub fn count(&self) -> usize {
        self.wakes.load(Ordering::Relaxed)
    }
}

impl Wake for CountingWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wakes.fetch_add(1, Ordering::Relaxed);
    }
}

/// Polls a future exactly once.
///
/// Returns the raw [`Poll`] so a test can assert on `Pending` as well as
/// `Ready` — which `.await` gives no way to observe.
///
/// # Example
///
/// ```
/// use futures_patterns::testing::poll_once;
/// use std::task::{Poll, Waker};
///
/// let mut future = Box::pin(async { 42 });
/// assert_eq!(poll_once(future.as_mut(), Waker::noop()), Poll::Ready(42));
/// ```
pub fn poll_once<F: Future>(future: Pin<&mut F>, waker: &Waker) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(waker))
}

/// Polls a future until it is ready, returning its output and the number of polls taken.
///
/// The poll count is the point of this helper: it is how a test pins down a
/// state machine's shape rather than just its final value.
///
/// Wakes are discarded, so this drives futures that make progress on every poll.
/// It is not a runtime and cannot make a future that waits on real I/O or a timer
/// complete.
///
/// # Panics
///
/// Panics if the future is still pending after `max_polls`. A bounded loop keeps
/// a buggy future from hanging the test suite forever.
///
/// # Example
///
/// ```
/// use futures_patterns::testing::poll_until_ready;
///
/// let mut future = Box::pin(async { 42 });
/// let (output, polls) = poll_until_ready(future.as_mut(), 10);
/// assert_eq!(output, 42);
/// assert_eq!(polls, 1);
/// ```
pub fn poll_until_ready<F: Future>(mut future: Pin<&mut F>, max_polls: usize) -> (F::Output, usize) {
    let waker = Waker::noop();
    for polls in 1..=max_polls {
        if let Poll::Ready(output) = poll_once(future.as_mut(), waker) {
            return (output, polls);
        }
    }
    panic!("future still pending after {max_polls} polls");
}

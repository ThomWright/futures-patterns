//! Transform a future's output with a function.
//!
//! `Map` wraps a future and applies a function to its output when it completes. It is
//! the smallest combinator here: one inner future, one transformation, no coordination.
//!
//! # Why the function lives in an `Option`
//!
//! The function is `FnOnce`, so calling it consumes it, and calling it requires moving
//! it out of the struct. `Option::take` is what allows that from behind a `&mut`.
//!
//! The empty `Option` doubles as a record that the work is done, which is how a second
//! poll is caught and turned into a panic rather than a silent wrong answer.
//!
//! # Pinning
//!
//! `pin-project-lite` projects the pin from `Map` to the inner future, which the
//! `#[pin]` attribute on that field asks for. The function needs no such treatment: it
//! is called, never polled.
//!
//! # When to use
//!
//! When you have a future whose output is the wrong shape.
//!
//! # Example
//!
//! ```
//! use futures_patterns::composition::map::Map;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let future = async { 21 };
//! let doubled = Map::new(future, |x| x * 2);
//! let result = doubled.await;
//! assert_eq!(result, 42);
//! # }
//! ```

use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// Future for transforming the output of another future.
    ///
    /// This future wraps an inner future and applies a function to its output.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Map<Fut, F> {
        // The #[pin] attribute means that when Map is pinned, this field
        // is also pinned. This is necessary because we need to poll the
        // inner future, which requires it to be pinned.
        #[pin]
        future: Fut,

        // The mapping function doesn't need to be pinned because we only
        // need to call it (which requires &mut F or F), not poll it.
        f: Option<F>,
    }
}

impl<Fut, F> Map<Fut, F> {
    /// Creates a new `Map` future.
    ///
    /// The provided function will be called with the output of the inner
    /// future when it completes.
    ///
    /// # Example
    ///
    /// ```
    /// use futures_patterns::composition::map::Map;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let future = async { "hello" };
    /// let mapped = Map::new(future, |s: &str| s.to_uppercase());
    /// assert_eq!(mapped.await, "HELLO");
    /// # }
    /// ```
    pub fn new(future: Fut, f: F) -> Self {
        Map {
            future,
            f: Some(f),
        }
    }
}

impl<Fut, F, T> Future for Map<Fut, F>
where
    Fut: Future,
    F: FnOnce(Fut::Output) -> T,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Project the pin to get access to the pinned fields.
        // This gives us:
        // - `future: Pin<&mut Fut>` (because it's marked with #[pin])
        // - `f: &mut Option<F>` (unpinned mutable reference)
        let this = self.project();

        // Poll the inner future
        match this.future.poll(cx) {
            Poll::Ready(output) => {
                // The future completed - apply the mapping function.
                // We use take() to move the function out of the Option,
                // as we can only call FnOnce once.
                let map_fn = this
                    .f
                    .take()
                    .expect("Map polled after completion");

                Poll::Ready(map_fn(output))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Creates a future that applies a function to the output of another future.
///
/// This is a convenience function for creating a `Map` combinator.
///
/// # Example
///
/// ```
/// use futures_patterns::composition::map::map;
///
/// # #[tokio::main]
/// # async fn main() {
/// let future = async { 10 };
/// let result = map(future, |x| x + 32).await;
/// assert_eq!(result, 42);
/// # }
/// ```
pub fn map<Fut, F, T>(future: Fut, f: F) -> Map<Fut, F>
where
    Fut: Future,
    F: FnOnce(Fut::Output) -> T,
{
    Map::new(future, f)
}

#[cfg(test)]
mod tests {
    use super::{Map, map};
    use crate::basic::ready::ready;
    use crate::state_machine::two_state::CountDown;
    use crate::testing::{poll_once, poll_until_ready};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Poll, Waker};

    #[test]
    fn transforms_the_output() {
        let mut fut = Box::pin(map(ready(21), |x| x * 2));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(42));
    }

    #[test]
    fn can_change_the_output_type() {
        let mut fut = Box::pin(map(ready(42), |x| format!("got {x}")));
        assert_eq!(
            poll_once(fut.as_mut(), Waker::noop()),
            Poll::Ready("got 42".to_string())
        );
    }

    #[test]
    fn does_not_call_the_function_until_the_inner_future_is_ready() {
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&calls);

        // CountDown(2) is pending for two polls, so the first two polls of Map must
        // leave the mapping function untouched.
        let mut fut = Box::pin(map(CountDown::new(2), move |x| {
            seen.fetch_add(1, Ordering::Relaxed);
            x
        }));

        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
        assert_eq!(calls.load(Ordering::Relaxed), 0);

        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(2));
        assert_eq!(calls.load(Ordering::Relaxed), 1, "called exactly once");
    }

    #[test]
    fn calls_the_function_exactly_once() {
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&calls);
        let (_, polls) = poll_until_ready(
            Box::pin(map(CountDown::new(3), move |x| {
                seen.fetch_add(1, Ordering::Relaxed);
                x
            }))
            .as_mut(),
            10,
        );
        assert_eq!(polls, 4, "three pending polls, then ready");
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    #[should_panic(expected = "Map polled after completion")]
    fn panics_when_polled_after_completion() {
        // The mapping function is `FnOnce` and has been consumed, so there is nothing
        // left to call. Map reports this rather than failing silently.
        let mut fut = Box::pin(map(CountDown::new(0), |x| x));
        let _ = poll_once(fut.as_mut(), Waker::noop());
        let _ = poll_once(fut.as_mut(), Waker::noop());
    }

    #[test]
    fn accepts_a_non_copy_fnonce() {
        // Moving a String into the closure proves `FnOnce` is enough; the closure does
        // not have to be callable twice.
        let suffix = String::from("!");
        let mut fut = Box::pin(Map::new(ready("hi"), move |s: &str| s.to_string() + &suffix));
        assert_eq!(
            poll_once(fut.as_mut(), Waker::noop()),
            Poll::Ready("hi!".to_string())
        );
    }
}

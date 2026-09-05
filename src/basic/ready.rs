//! A future that immediately resolves to a value.
//!
//! Ready on the first poll, handing over a value supplied when it was created.
//!
//! The value is kept in an `Option` and moved out on that poll, so `Ready` does carry
//! state, and polling it again panics: there is nothing left to hand over.
//!
//! # When to use
//!
//! When an API requires a future but the value is already to hand.
//!
//! # Example
//!
//! ```
//! use futures_patterns::basic::ready::ready;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let value = ready(42).await;
//! assert_eq!(value, 42);
//! # }
//! ```
//!
//! Follows `core::future::Ready`; see NOTICE.md.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for the [`ready`] function.
///
/// This future completes immediately with the provided value.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Ready<T> {
    value: Option<T>,
}

/// Creates a future that immediately resolves to the provided value.
///
/// This is useful when you need to return a Future but the value is already
/// available synchronously.
///
/// # Example
///
/// ```
/// use futures_patterns::basic::ready::ready;
///
/// # #[tokio::main]
/// # async fn main() {
/// let result = ready("Hello, world!").await;
/// assert_eq!(result, "Hello, world!");
/// # }
/// ```
pub fn ready<T>(value: T) -> Ready<T> {
    Ready { value: Some(value) }
}

impl<T> Future for Ready<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        // We store the value in an Option so we can take it out without requiring Clone.
        // This allows Ready to work with any type T, not just Clone types.
        //
        // We use take() to move the value out, leaving None behind.
        // If polled again after completion, this will panic - which is acceptable
        // because polling a completed future is a logic error.
        Poll::Ready(
            self.value
                .take()
                .expect("Ready polled after completion"),
        )
    }
}

// `Ready<T>` is `Unpin` for every `T`, including a `T` that is not.
//
// It is not redundant: the compiler grants `Unpin` only when every field has it, so
// `Ready<T>` would otherwise follow `T`. Claiming it for all `T` is sound because we
// never construct a `Pin<&mut T>` anywhere -- the value is never pinned.
//
// `poll` above relies on it: remove this impl and `self.value.take()` stops compiling,
// because reaching a field through a `Pin` is automatic only for `Unpin` types.
//
// See `advanced::pinning` for more about pinning.
impl<T> Unpin for Ready<T> {}

#[cfg(test)]
mod tests {
    use super::{Ready, ready};
    use crate::testing::poll_once;
    use std::marker::PhantomPinned;
    use std::pin::Pin;
    use std::task::{Poll, Waker};

    fn assert_unpin<T: Unpin>() {}

    #[test]
    fn completes_on_first_poll() {
        let mut fut = Box::pin(ready(42));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(42));
    }

    #[test]
    #[should_panic(expected = "Ready polled after completion")]
    fn panics_when_polled_after_completion() {
        // The value was moved out on the first poll, so there is nothing to return.
        let mut fut = Box::pin(ready(42));
        let _ = poll_once(fut.as_mut(), Waker::noop());
        let _ = poll_once(fut.as_mut(), Waker::noop());
    }

    #[test]
    fn is_unpin_even_when_the_value_is_not() {
        assert_unpin::<Ready<PhantomPinned>>();

        // What that buys the caller: `Pin::new`, so no `Box::pin` and no `pin!`.
        let mut fut = ready(PhantomPinned);
        assert_eq!(
            poll_once(Pin::new(&mut fut), Waker::noop()),
            Poll::Ready(PhantomPinned)
        );
    }
}

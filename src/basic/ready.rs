//! A future that immediately resolves to a value.
//!
//! This is the simplest possible Future implementation - it always returns
//! `Poll::Ready` on the first poll. It requires no state tracking and demonstrates
//! the most basic structure of the Future trait.
//!
//! # When to use
//!
//! Use this pattern when you need to wrap a value in a Future but the value
//! is already available. Common in APIs that require Future return types but
//! sometimes have the result immediately available.
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

// Not redundant. `Unpin` is an auto trait, so the derived impl would only hold when
// `T: Unpin`. `Ready` never creates a `Pin<&mut T>` -- the value is moved straight out
// with `take()` -- so it is sound to be `Unpin` for every `T`, and useful: it means a
// `Ready<T>` can be polled without boxing even when `T` cannot be moved.
impl<T> Unpin for Ready<T> {}

#[cfg(test)]
mod tests {
    use super::ready;
    use crate::testing::poll_once;
    use std::task::{Poll, Waker};

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
}

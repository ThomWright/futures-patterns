//! Transform a future's output with a function.
//!
//! The `Map` combinator wraps a future and applies a function to its output
//! when it completes. This is one of the fundamental composition patterns that
//! allows building complex async operations from simpler ones.
//!
//! # Pattern overview
//!
//! Map demonstrates:
//! - How to wrap and poll an inner future
//! - How to transform the output type
//! - Basic use of `pin-project-lite` for safe pin projection
//! - How combinators compose futures
//!
//! # Pinning strategy
//!
//! This uses `pin-project-lite` to safely project the pin from `Map` to the
//! inner future. The `#[pin]` attribute ensures that when `Map` is pinned,
//! the inner future is also properly pinned.
//!
//! # When to use
//!
//! Use this pattern when you need to:
//! - Transform a future's output type
//! - Apply post-processing to async results
//! - Build combinator libraries
//! - Chain operations on futures
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

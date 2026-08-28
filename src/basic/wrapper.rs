//! Wrapping an existing future in a newtype.
//!
//! This pattern shows how to wrap an existing future to:
//! - Hide implementation details.
//! - Provide a cleaner, more semantic API.
//! - Avoid exposing complex nested future types.
//! - Create domain-specific future types.
//!
//! # The challenge
//!
//! When you have a future (like `Notified` from tokio::sync::Notify or
//! `Receiver` from a channel) and want to wrap it in a custom type, you need to:
//!
//! 1. Store the inner future
//! 2. Implement Future for your wrapper
//! 3. Handle pinning correctly
//! 4. Forward the poll to the inner future
//!
//! The tricky part is pinning - when your wrapper is pinned, you need to project
//! that pin to the inner future safely.
//!
//! # Key methods for working with pinned types
//!
//! When working with `Option<Future>` or similar wrapper types, these methods are crucial:
//!
//! - `Option::as_pin_mut()` - Converts `Pin<&mut Option<T>>` to `Option<Pin<&mut T>>`.
//! - `Pin::as_mut()` - Converts `&mut Pin<Pointer<T>>` to `Pin<&mut T>` (reborrowing).
//! - `Pin::new()` - Creates `Pin<&mut T>` when `T: Unpin`.
//! - `self.project()` (from pin-project) - Projects pinned struct to pinned fields.
//!
//! # Two approaches
//!
//! ## 1. Simple wrapper (when inner future is Unpin)
//!
//! If the inner future is `Unpin`, you can just use `Pin::new()`:
//!
//! ```rust
//! use std::future::Future;
//! use std::pin::Pin;
//! use std::task::{Context, Poll};
//!
//! pub struct Finished<F> {
//!     inner: F,
//! }
//!
//! impl<F: Future + Unpin> Future for Finished<F> {
//!     type Output = F::Output;
//!
//!     fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//!         // Safe because F is Unpin
//!         Pin::new(&mut self.inner).poll(cx)
//!     }
//! }
//! ```
//!
//! ## 2. Generic wrapper (works with any future)
//!
//! For futures that might not be `Unpin`, use `pin-project-lite`:
//!
//! ```rust
//! use pin_project_lite::pin_project;
//! use std::future::Future;
//! use std::pin::Pin;
//! use std::task::{Context, Poll};
//!
//! pin_project! {
//!     pub struct Finished<F> {
//!         #[pin]
//!         inner: F,
//!     }
//! }
//!
//! impl<F: Future> Future for Finished<F> {
//!     type Output = F::Output;
//!
//!     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//!         // Project the pin to the inner future
//!         self.project().inner.poll(cx)
//!     }
//! }
//! ```
//!
//! # When to use
//!
//! Use this pattern when:
//! - You want to hide complex future types from your API.
//! - You need domain-specific future types (e.g., `ShutdownSignal`, `TaskComplete`).
//! - You're building a library and want to avoid exposing implementation details.
//! - You need to add semantic meaning to a generic future.
//!
//! # Examples
//!
//! ```
//! use futures_patterns::basic::wrapper::Finished;
//! use tokio::sync::Notify;
//! use std::sync::Arc;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let notify = Arc::new(Notify::new());
//! let notified = notify.notified();
//!
//! // Wrap the Notified future in our custom type
//! let finished = Finished::new(notified);
//!
//! // Spawn a task to notify
//! let notify_clone = notify.clone();
//! tokio::spawn(async move {
//!     tokio::time::sleep(std::time::Duration::from_millis(10)).await;
//!     notify_clone.notify_one();
//! });
//!
//! // Wait using our wrapped future
//! finished.await;
//! println!("Finished!");
//! # }
//! ```

use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// A wrapper around a future that signals completion.
    ///
    /// This demonstrates the pattern of wrapping an arbitrary future in a
    /// newtype to provide a cleaner API or hide implementation details.
    ///
    /// The inner future type is hidden from users - they just see `Finished`.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Finished<F> {
        // The #[pin] attribute ensures that when Finished is pinned,
        // the inner future is also properly pinned.
        #[pin]
        inner: F,
    }
}

impl<F> Finished<F> {
    /// Creates a new `Finished` future wrapping the given future.
    ///
    /// # Example
    ///
    /// ```
    /// use futures_patterns::basic::wrapper::Finished;
    /// use futures_patterns::basic::ready::ready;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let finished = Finished::new(ready(42));
    /// let value = finished.await;
    /// assert_eq!(value, 42);
    /// # }
    /// ```
    pub fn new(inner: F) -> Self {
        Finished { inner }
    }

    /// Gets a reference to the inner future.
    ///
    /// This is useful if you need to inspect the inner future without
    /// consuming the wrapper.
    pub fn get_ref(&self) -> &F {
        &self.inner
    }

    /// Gets a mutable reference to the inner future.
    pub fn get_mut(&mut self) -> &mut F {
        &mut self.inner
    }

    /// Consumes the wrapper and returns the inner future.
    ///
    /// This allows extracting the wrapped future if needed.
    pub fn into_inner(self) -> F {
        self.inner
    }
}

impl<F: Future> Future for Finished<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Use pin-project to safely project the pin from Finished to the inner future.
        // This gives us Pin<&mut F>, which we can then poll.
        self.project().inner.poll(cx)
    }
}

// Example: A domain-specific wrapper for shutdown signals
pin_project! {
    /// A future that completes when a shutdown signal is received.
    ///
    /// This demonstrates wrapping a channel receiver in a more semantic type.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct ShutdownSignal<F> {
        #[pin]
        receiver: F,
    }
}

impl<F> ShutdownSignal<F> {
    /// Creates a new shutdown signal from a receiver future.
    ///
    /// # Example
    ///
    /// ```
    /// use futures_patterns::basic::wrapper::ShutdownSignal;
    /// use tokio::sync::oneshot;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let (tx, rx) = oneshot::channel();
    ///
    /// let shutdown = ShutdownSignal::new(rx);
    ///
    /// // Send shutdown signal
    /// tx.send(()).unwrap();
    ///
    /// // Wait for shutdown
    /// let _ = shutdown.await;
    /// # }
    /// ```
    pub fn new(receiver: F) -> Self {
        ShutdownSignal { receiver }
    }
}

impl<F, T, E> Future for ShutdownSignal<F>
where
    F: Future<Output = Result<T, E>>,
{
    // For shutdown signals, we typically don't care about the value,
    // just that the signal was received.
    type Output = Result<(), E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Project to get the pinned receiver
        match self.project().receiver.poll(cx) {
            Poll::Ready(Ok(_)) => Poll::Ready(Ok(())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

// Example: Wrapping an Unpin future (simpler approach)
/// A simple wrapper when you know the inner future is Unpin.
///
/// This demonstrates that if your inner future is `Unpin`, you can skip
/// pin-project and use a simpler approach.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct SimpleWrapper<F> {
    inner: F,
}

impl<F> SimpleWrapper<F> {
    /// Creates a new wrapper.
    pub fn new(inner: F) -> Self {
        SimpleWrapper { inner }
    }
}

// Only implement Future when F is Unpin
impl<F: Future + Unpin> Future for SimpleWrapper<F> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Safe because F is Unpin - we can get a Pin<&mut F> from &mut F
        Pin::new(&mut self.inner).poll(cx)
    }
}

// No manual `Unpin` impl: the derived one is already `Unpin` exactly when `F` is.

// Example: Wrapping Option<Future> - a future that might not exist
pin_project! {
    /// A future wrapper that might contain a future, or might be empty.
    ///
    /// This is useful when you have optional async operations. If `Some`, it polls
    /// the inner future. If `None`, it immediately returns a default value.
    ///
    /// # Key challenge with Option<Future>
    ///
    /// The tricky part is that `Option<F>` itself needs to be pinned when `F` is
    /// not `Unpin`. We use `pin-project` with `#[pin]` on the Option field to handle
    /// this correctly.
    ///
    /// # Example
    ///
    /// ```
    /// use futures_patterns::basic::wrapper::OptionFuture;
    /// use futures_patterns::basic::ready::{ready, Ready};
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// // With a future
    /// let with_future = OptionFuture::new(Some(ready(42)));
    /// assert_eq!(with_future.await, Some(42));
    ///
    /// // Without a future. The type parameter has to be named: there is no
    /// // argument to infer it from.
    /// let without_future = OptionFuture::<Ready<i32>>::new(None);
    /// assert_eq!(without_future.await, None);
    /// # }
    /// ```
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct OptionFuture<F> {
        // The #[pin] applies to the Option itself, which means when OptionFuture
        // is pinned, the Option is pinned, and if it contains a future, that
        // future is also pinned.
        #[pin]
        inner: Option<F>,
    }
}

impl<F> OptionFuture<F> {
    /// Creates a new `OptionFuture`.
    ///
    /// If `None`, the future will immediately return `None` when polled.
    /// If `Some(future)`, it will poll the inner future.
    pub fn new(inner: Option<F>) -> Self {
        OptionFuture { inner }
    }

    /// Creates an `OptionFuture` with no inner future.
    ///
    /// This will immediately complete with `None`.
    pub fn none() -> Self {
        OptionFuture { inner: None }
    }

    /// Creates an `OptionFuture` with an inner future.
    pub fn some(future: F) -> Self {
        OptionFuture { inner: Some(future) }
    }

    /// Returns `true` if the inner future exists.
    pub fn is_some(&self) -> bool {
        self.inner.is_some()
    }

    /// Returns `true` if there is no inner future.
    pub fn is_none(&self) -> bool {
        self.inner.is_none()
    }

    /// Consumes the wrapper and returns the inner future.
    ///
    /// This is the sound way to recover the inner future: taking `self` by value
    /// proves nothing has pinned it yet, so moving it out is fine. It is useful for
    /// discarding the wrapper while keeping the operation — for example to rewrap
    /// it in a different combinator.
    ///
    /// There is deliberately no equivalent that takes `Pin<&mut Self>`. Once the
    /// wrapper has been pinned, `inner` is pinned too (it is a `#[pin]` field), and
    /// an already-polled future may hold references into its own storage. Returning
    /// it by value would move it and leave those references dangling. `std` draws
    /// the same line: [`Pin::get_mut`] is safe only for `T: Unpin`, and the
    /// unconditional version, [`Pin::get_unchecked_mut`], is `unsafe`.
    ///
    /// To abandon a future that is already pinned, use [`OptionFuture::clear`],
    /// which drops it in place instead of moving it out.
    ///
    /// [`Pin::get_mut`]: std::pin::Pin::get_mut
    /// [`Pin::get_unchecked_mut`]: std::pin::Pin::get_unchecked_mut
    ///
    /// # Example
    ///
    /// ```
    /// use futures_patterns::basic::wrapper::OptionFuture;
    /// use futures_patterns::basic::ready::ready;
    ///
    /// let wrapped = OptionFuture::new(Some(ready(42)));
    /// let inner = wrapped.into_inner();
    /// assert!(inner.is_some());
    /// ```
    pub fn into_inner(self) -> Option<F> {
        self.inner
    }

    /// Drops the inner future in place, leaving `None`.
    ///
    /// Polling afterwards completes immediately with `None`. Use this to abandon an
    /// optional operation early and release whatever the future was holding, without
    /// waiting for it to finish.
    ///
    /// This works for any `F`, including futures that are not `Unpin`, because
    /// [`Pin::set`] drops the old value where it sits rather than moving it out.
    /// That is the same approach `tokio_util::io::ReaderStream` takes to empty its
    /// pinned `Option` field.
    ///
    /// [`Pin::set`]: std::pin::Pin::set
    ///
    /// # Example
    ///
    /// ```
    /// use futures_patterns::basic::wrapper::OptionFuture;
    /// use futures_patterns::testing::poll_once;
    /// use std::task::{Poll, Waker};
    ///
    /// let mut wrapped = Box::pin(OptionFuture::new(Some(std::future::pending::<i32>())));
    /// wrapped.as_mut().clear();
    /// assert_eq!(poll_once(wrapped.as_mut(), Waker::noop()), Poll::Ready(None));
    /// ```
    pub fn clear(self: Pin<&mut Self>) {
        self.project().inner.set(None);
    }
}

impl<F: Future> Future for OptionFuture<F> {
    type Output = Option<F::Output>;

    // Note the asymmetry with `SimpleOptionFuture` below, which clears its slot on
    // completion and so reports `None` if polled again. This one leaves the slot
    // populated, so a second poll would re-poll a finished future. That is the
    // caller's contract violation rather than something to defend against here, but
    // it does mean the two types behave differently when the rule is broken.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Project to get Pin<&mut Option<F>>
        let inner = self.project().inner;

        // Use Option::as_pin_mut to go from Pin<&mut Option<F>> to Option<Pin<&mut F>>
        match inner.as_pin_mut() {
            Some(future) => {
                // Poll the inner future
                match future.poll(cx) {
                    Poll::Ready(value) => Poll::Ready(Some(value)),
                    Poll::Pending => Poll::Pending,
                }
            }
            None => {
                // No future to poll - immediately ready with None
                Poll::Ready(None)
            }
        }
    }
}

// Alternative implementation without pin-project, when F is Unpin
/// A simpler `OptionFuture` implementation when the inner future is `Unpin`.
///
/// This shows that if you know your future is `Unpin`, you can avoid pin-project.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct SimpleOptionFuture<F> {
    inner: Option<F>,
}

impl<F> SimpleOptionFuture<F> {
    /// Creates a new `SimpleOptionFuture`.
    pub fn new(inner: Option<F>) -> Self {
        SimpleOptionFuture { inner }
    }
}

impl<F: Future + Unpin> Future for SimpleOptionFuture<F> {
    type Output = Option<F::Output>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &mut self.inner {
            Some(future) => {
                // Safe because F is Unpin
                match Pin::new(future).poll(cx) {
                    Poll::Ready(value) => {
                        // Clear the option after completion
                        self.inner = None;
                        Poll::Ready(Some(value))
                    }
                    Poll::Pending => Poll::Pending,
                }
            }
            None => Poll::Ready(None),
        }
    }
}

// As with `SimpleWrapper`, the derived `Unpin` impl is already what we want.

// Focused on the pinning rules these wrappers demonstrate.
#[cfg(test)]
mod tests {
    use super::{Finished, OptionFuture, SimpleOptionFuture, SimpleWrapper};
    use crate::basic::ready::{Ready, ready};
    use crate::testing::{poll_once, poll_until_ready};
    use std::task::{Poll, Waker};

    #[test]
    fn option_future_polls_the_inner_future() {
        let mut fut = Box::pin(OptionFuture::new(Some(ready(42))));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(Some(42)));
    }

    #[test]
    fn option_future_is_ready_with_none_when_empty() {
        let mut fut = Box::pin(OptionFuture::<Ready<i32>>::new(None));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(None));
    }

    #[test]
    fn into_inner_recovers_the_future_before_it_is_pinned() {
        let wrapped = OptionFuture::new(Some(ready(42)));
        let inner = wrapped.into_inner().expect("future should still be there");
        // Recovered intact, and still usable.
        let (output, _) = poll_until_ready(Box::pin(inner).as_mut(), 2);
        assert_eq!(output, 42);
    }

    #[test]
    fn clear_drops_a_pinned_future_in_place() {
        // `std::future::pending` never completes, so a successful Ready(None) proves
        // the slot was emptied rather than polled.
        let mut fut = Box::pin(OptionFuture::new(Some(std::future::pending::<i32>())));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);

        fut.as_mut().clear();
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(None));
    }

    #[test]
    fn clear_runs_the_inner_futures_destructor() {
        use std::sync::Arc;

        // Dropping the Arc clone held by the future is observable via the strong
        // count, which is how we know `clear` destroys in place rather than leaking.
        let tracker = Arc::new(());
        let clone = Arc::clone(&tracker);
        let inner = async move {
            let _held = clone;
            std::future::pending::<i32>().await
        };

        let mut fut = Box::pin(OptionFuture::new(Some(inner)));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
        assert_eq!(Arc::strong_count(&tracker), 2);

        fut.as_mut().clear();
        assert_eq!(Arc::strong_count(&tracker), 1, "inner future should be dropped");
    }

    #[test]
    fn simple_option_future_clears_itself_after_completion() {
        let mut fut = Box::pin(SimpleOptionFuture::new(Some(ready(7))));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(Some(7)));
        // Unlike OptionFuture, this one empties its slot on completion, so a second
        // poll reports None instead of re-polling a finished future.
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(None));
    }

    #[test]
    fn finished_forwards_to_the_inner_future() {
        let mut fut = Box::pin(Finished::new(ready("done")));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready("done"));
    }

    #[test]
    fn the_simple_wrappers_are_unpin_exactly_when_the_inner_future_is() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<SimpleWrapper<Ready<i32>>>();
        assert_unpin::<SimpleOptionFuture<Ready<i32>>>();
    }
}

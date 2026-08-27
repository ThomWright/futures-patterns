//! A future that wraps a polling function.
//!
//! This pattern allows creating a future from a closure that implements the polling
//! logic directly. It's the most flexible basic pattern and demonstrates important
//! concepts around pinning and the `Unpin` trait.
//!
//! # Why this struct is intentionally !Unpin
//!
//! This is a crucial learning point from tokio's implementation. When a closure `F`
//! is `!Unpin`, we need `PollFn<F>` to also be `!Unpin`. Here's why:
//!
//! If `PollFn` were unconditionally `Unpin`, Rust's compiler would add `noalias`
//! annotations to mutable references to `PollFn`. This is a problem because if the
//! closure owns a future, that "leaks" the noalias annotation to the owned future,
//! which can cause soundness issues.
//!
//! By making `PollFn` conditionally `Unpin` (only when `F` is `Unpin`), we avoid
//! this problem.
//!
//! See: <https://internals.rust-lang.org/t/surprising-soundness-trouble-around-pollfn/17484>
//!
//! # When to use
//!
//! Use this pattern when:
//! - You need custom polling logic without defining a new type
//! - You're prototyping async behavior
//! - You need to integrate with non-standard async sources
//! - You want to manually control when a future becomes ready
//!
//! # Example
//!
//! ```
//! use futures_patterns::basic::poll_fn::poll_fn;
//! use std::task::Poll;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let mut count = 0;
//! let result = poll_fn(|cx| {
//!     count += 1;
//!     if count >= 3 {
//!         Poll::Ready(count)
//!     } else {
//!         // Returning Pending without arranging a wake would hang the task
//!         // forever, because nothing would ever ask for another poll.
//!         cx.waker().wake_by_ref();
//!         Poll::Pending
//!     }
//! }).await;
//! assert_eq!(result, 3);
//! # }
//! ```

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for the [`poll_fn`] function.
///
/// This struct intentionally does NOT implement `Unpin` unconditionally.
/// It is `!Unpin` when `F` is `!Unpin`, which is important for soundness.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct PollFn<F> {
    f: F,
}

/// Creates a future from a function returning [`Poll`].
///
/// The provided function will be called each time the future is polled.
/// It receives a `Context` which provides access to the waker for scheduling
/// future polls.
///
/// # Example
///
/// ```
/// use futures_patterns::basic::poll_fn::poll_fn;
/// use std::task::Poll;
///
/// # #[tokio::main]
/// # async fn main() {
/// let value = poll_fn(|_cx| Poll::Ready(42)).await;
/// assert_eq!(value, 42);
/// # }
/// ```
pub fn poll_fn<T, F>(f: F) -> PollFn<F>
where
    F: FnMut(&mut Context<'_>) -> Poll<T>,
{
    PollFn { f }
}

impl<F> fmt::Debug for PollFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PollFn").finish()
    }
}

impl<T, F> Future for PollFn<F>
where
    F: FnMut(&mut Context<'_>) -> Poll<T>,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        // SAFETY: we never construct a `Pin<&mut F>` anywhere, so reaching `f` through
        // an unpinned `&mut` is sound. `f` is called, never polled.
        //
        // `pin_project!` cannot replace this unsafe block directly:
        //  * with `#[pin]` on the field, projection yields `Pin<&mut F>`, which cannot
        //    be used to call the closure;
        //  * without `#[pin]`, the generated struct becomes unconditionally `Unpin`,
        //    which is the very thing that must be avoided.
        //
        // The `alt` module below gets there anyway, using `#[project(!Unpin)]` to opt
        // out of the generated `Unpin` impl.
        let me = unsafe { Pin::into_inner_unchecked(self) };
        (me.f)(cx)
    }
}

// There is deliberately no `Unpin` impl here at all, not even a conditional one.
//
// The derived impl already makes `PollFn<F>` `Unpin` exactly when `F` is, which is what
// soundness requires; writing it out by hand would only restate that. What matters is
// that we never write the *unconditional* `impl<F> Unpin for PollFn<F> {}`. Tokio does
// the same, and covers the property with a test rather than an impl.

/// A safe alternative to the manual `unsafe` implementation above.
///
/// The parent module reaches for `unsafe` twice: once for
/// `Pin::into_inner_unchecked` in `poll`, and once for the hand-written
/// `impl<F: Unpin> Unpin for PollFn<F>`. `pin_project_lite` expresses the same
/// thing declaratively, with no `unsafe` at all.
///
/// # Why `#[project(!Unpin)]` is required here
///
/// Without the attribute, `pin_project!` generates an `Unpin` impl that holds whenever
/// the unpinned fields are `Unpin` -- and since `f` is not a `#[pin]` field, that makes
/// `PollFn<F>` `Unpin` far too eagerly. Tokio's own comment cites exactly this as the
/// reason pin-project cannot be used for `poll_fn`.
///
/// `#[project(!Unpin)]` is what closes that gap: it makes the struct *unconditionally*
/// `!Unpin`, which is strictly more conservative than the manual impl, holding even
/// when `F` is `Unpin`.
///
/// That is the safe choice for `poll_fn` specifically. As the parent module explains,
/// the hazard is that an `Unpin` `PollFn` lets the compiler apply `noalias` to
/// `&mut PollFn<F>`, and if the closure owns a future, that annotation leaks to the
/// owned future. Opting out of `Unpin` entirely rules that out.
///
/// Note that `f` is deliberately *not* marked `#[pin]`. The closure is called, never
/// polled, so nothing here needs a `Pin<&mut F>`.
///
/// The `!Unpin` guarantee holds even for a closure that is itself `Unpin`, so this
/// does not compile:
///
/// ```compile_fail
/// use futures_patterns::basic::poll_fn::alt::poll_fn;
/// use std::task::Poll;
///
/// fn assert_unpin<T: Unpin>(_: &T) {}
///
/// let future = poll_fn(|_cx| Poll::Ready(42));
/// assert_unpin(&future); // error: PhantomPinned cannot be unpinned
/// ```
pub mod alt {
    use std::{
        pin::Pin,
        task::{Context, Poll},
    };

    use pin_project_lite::pin_project;

    pin_project! {
        /// Future for the [`poll_fn`] function.
        ///
        /// Unconditionally `!Unpin`, even when `F: Unpin`.
        #[must_use = "futures do nothing unless you `.await` or poll them"]
        #[project(!Unpin)]
        pub struct PollFn<F> {
            f: F,
        }
    }

    /// Creates a future from a function returning [`Poll`].
    ///
    /// # Example
    ///
    /// ```
    /// use futures_patterns::basic::poll_fn::alt::poll_fn;
    /// use std::task::Poll;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let value = poll_fn(|_cx| Poll::Ready(42)).await;
    /// assert_eq!(value, 42);
    /// # }
    /// ```
    pub fn poll_fn<T, F>(f: F) -> PollFn<F>
    where
        F: FnMut(&mut Context<'_>) -> Poll<T>,
    {
        PollFn { f }
    }

    impl<T, F> Future for PollFn<F>
    where
        F: FnMut(&mut Context<'_>) -> Poll<T>,
    {
        type Output = T;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
            // `f` is not a `#[pin]` field, so projection hands back a plain
            // `&mut F` and no unsafe is needed to call it.
            (self.project().f)(cx)
        }
    }
}

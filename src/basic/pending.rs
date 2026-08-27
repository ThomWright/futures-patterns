//! A future that never completes.
//!
//! This future always returns `Poll::Pending` and never resolves to a value.
//! It demonstrates the minimal implementation of a perpetually-pending future.
//!
//! # When to use
//!
//! This pattern is useful for:
//! - Testing timeout behavior
//! - Creating placeholder futures during development
//! - Demonstrating async control flow
//! - Race conditions where one branch should never complete
//!
//! # Important note on Wakers
//!
//! This implementation never calls `cx.waker().wake()`, so the runtime will not poll
//! it again, and the task parks forever. That is the *correct* behaviour here, not a
//! shortcut: a future must arrange a wake only when progress becomes possible, and
//! for `pending` it never does. Waking would just spin the scheduler to no purpose.
//!
//! Contrast `state_machine::two_state`, which does wake, because it genuinely can
//! make progress on the next poll.
//!
//! # Example
//!
//! ```no_run
//! use futures_patterns::basic::pending::pending;
//! use std::time::Duration;
//!
//! # #[tokio::main]
//! # async fn main() {
//! // This will never complete
//! let never: i32 = pending().await;
//! # }
//! ```

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for the [`pending`] function.
///
/// This future never completes - it always returns `Poll::Pending`.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Pending<T> {
    // Carries the type parameter without storing a value, so `Pending<T>` is
    // zero-sized whatever `T` is.
    //
    // `fn() -> T` rather than plain `T`: auto traits propagate through
    // `PhantomData<T>`, which would make `Pending<Rc<_>>` neither `Send` nor `Sync`
    // even though no `T` is ever stored. Wrapping it in a function pointer keeps
    // those impls unconditional. `std::future::Pending` does the same.
    _marker: PhantomData<fn() -> T>,
}

/// Creates a future that never resolves.
///
/// This future will always return `Poll::Pending` when polled. It's useful for
/// testing timeout behavior or representing futures that should never complete.
///
/// # Example
///
/// ```
/// use futures_patterns::basic::pending::pending;
///
/// use futures_patterns::composition::race::{race, Either};
/// use futures_patterns::basic::ready::ready;
///
/// # #[tokio::main]
/// # async fn main() {
/// // A branch that must never win a race.
/// let never_completes = pending::<i32>();
///
/// match race(ready(1), never_completes).await {
///     Either::Left(value) => assert_eq!(value, 1),
///     Either::Right(_) => unreachable!("pending() never completes"),
/// }
/// # }
/// ```
pub fn pending<T>() -> Pending<T> {
    Pending {
        _marker: PhantomData,
    }
}

impl<T> Future for Pending<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Always return Pending, never Ready.
        // Note: We don't call cx.waker().wake() because there's never
        // any progress to be made - this future will never complete.
        Poll::Pending
    }
}

// `Pending<T>` holds no state that needs pinning, so this is sound for every `T`.
// It is not redundant: `Unpin` is an auto trait and would otherwise propagate through
// the marker, leaving `Pending<T>` `!Unpin` whenever `T` is.
impl<T> Unpin for Pending<T> {}

// Hand-written rather than derived: `derive` would add a `T: Clone` bound, but the
// struct is zero-sized and always copyable.
impl<T> Clone for Pending<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Pending<T> {}

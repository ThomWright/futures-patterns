//! `Pin`, `Unpin`, and structural pinning.
//!
//! An `async fn` compiles to a state machine that can hold references into itself, so
//! moving one after polling has begun would leave those references dangling.
//! `Pin<&mut F>` is the promise that it will not move. `Unpin` marks the types that need
//! no such promise -- most of them -- and for those a `Pin` is inert: `Pin::new` wraps a
//! `&mut` without ceremony and hands it straight back.
//!
//! # Structural pinning
//!
//! `Pin<&mut Wrapper>` promises that the wrapper will not move. Whether that promise
//! extends to a field is not automatic: the wrapper decides, and the decision is called
//! structural pinning. Marking a field `#[pin]` is that decision written down.
//!
//! The two answers are not variations on a theme. Each settles three things at once, in
//! opposite directions:
//!
//! |                          | Not structural | Structural (`#[pin]`)  |
//! |--------------------------|----------------|------------------------|
//! | `poll` can reach the field as | `&mut F`  | `Pin<&mut F>`          |
//! | The field can be moved out    | yes       | no                     |
//! | The wrapper is `Unpin`        | always    | only when `F` is       |
//!
//! [`NotStructural`] and [`Structural`] are the smallest pair that shows it: one field
//! each, opposite answers. They are stripped-down versions of [`crate::basic::ready`]
//! and [`crate::basic::wrapper`], written out together because the difference is easier
//! to see side by side than across two modules.
//!
//! # Getting at a field
//!
//! A *projection* is a method that borrows a field through a `Pin<&mut Self>`. What it
//! hands back is the wrapper's choice: `Pin<&mut F>` for a structurally pinned field,
//! `&mut F` for one that is not. Std reserves a narrower phrase for the first case,
//! "projecting a pin", and calls the decision itself structural pinning.
//!
//! `Pin<&mut Self>` is not `&mut Self`, so reaching a field is a choice rather than a
//! dereference, and several routes look plausible while only one compiles. Two questions
//! decide it: is the field structurally pinned, and is its type `Unpin`.
//!
//! | You have | You want | Use |
//! |---|---|---|
//! | `Pin<&mut Self>`, field is `#[pin]` | `Pin<&mut F>`, to poll it | `self.project().field` |
//! | `Pin<&mut Self>`, field is not `#[pin]` | `&mut F`, to call or read it | `self.project().field` |
//! | `&mut F` where `F: Unpin` | `Pin<&mut F>` | `Pin::new(&mut f)` |
//! | `Pin<&mut Option<F>>` | `Option<Pin<&mut F>>` | `.as_pin_mut()` |
//! | `Pin<&mut Self>`, needed again afterwards | a second `Pin<&mut Self>` | `self.as_mut()` |
//! | `Pin<&mut F>` | to replace the value in place | `.set(new)` |
//! | none of the above | `&mut Self` | `unsafe { get_unchecked_mut() }` |
//!
//! The last row is the escape hatch, and reaching for it usually means one of the first
//! two answers was wrong. Two modules genuinely need it: [`crate::state_machine::maybe_done`]
//! uses `get_unchecked_mut`, and [`crate::advanced::poll_fn`] the equivalent
//! `Pin::into_inner_unchecked`.
//!
//! # Why `pin_project!` is safer than a hand-written `Unpin` impl
//!
//! `Unpin` is a safe trait, so the `impl<T> Unpin for NotStructural<T> {}` below carries
//! no `unsafe` to draw the eye. It is correct only while nothing projects into the value,
//! and nothing in the type system records that. Add an `unsafe { map_unchecked_mut(..) }`
//! later and both halves break at once and in silence: a caller can `Pin::new` the
//! wrapper, hand the value a pin it will trust, and then move it -- and taking the value
//! out is by then moving a pinned value too.
//!
//! Going through `pin_project!` is what makes the pairing checkable. The macro writes the
//! `Unpin` impl from the `#[pin]` markers, and refuses to share the job:
//!
//! ```compile_fail,E0119
//! use pin_project_lite::pin_project;
//!
//! pin_project! {
//!     pub struct Structural<F> {
//!         #[pin]
//!         inner: F,
//!     }
//! }
//!
//! // error[E0119]: conflicting implementations of trait `Unpin`
//! impl<F> Unpin for Structural<F> {}
//! ```
//!
//! By hand there is no such check, which is the argument for going through the macro
//! wherever the choice is live.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use pin_project_lite::pin_project;

/// A wrapper whose pin does not reach the value it holds.
///
/// The value is ordinary data. It is never pinned, and it is moved out on the first
/// poll, so this is `Unpin` whatever it holds:
///
/// ```
/// use futures_patterns::advanced::pinning::NotStructural;
/// use std::marker::PhantomPinned;
/// use std::pin::Pin;
///
/// fn assert_unpin<T: Unpin>(_: &T) {}
///
/// let held = NotStructural::new(PhantomPinned);
/// assert_unpin(&held);
/// ```
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct NotStructural<T> {
    value: Option<T>,
}

impl<T> NotStructural<T> {
    /// Wraps a value, to be handed back on the first poll.
    pub fn new(value: T) -> Self {
        Self { value: Some(value) }
    }
}

impl<T> Future for NotStructural<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<T> {
        Poll::Ready(self.value.take().expect("polled after completion"))
    }
}

// Sound because no `Pin<&mut T>` is ever created, so nothing inside ever became
// address-sensitive. The module docs say what breaks if that stops being true.
impl<T> Unpin for NotStructural<T> {}

pin_project! {
    /// A wrapper whose pin reaches the future it holds.
    ///
    /// The future is pinned where it lies and polled there, so it can never be moved
    /// out -- and this wrapper is `Unpin` only when the future is. An `async` block is
    /// not, so this does not compile:
    ///
    /// ```compile_fail
    /// use futures_patterns::advanced::pinning::Structural;
    ///
    /// fn assert_unpin<T: Unpin>(_: &T) {}
    ///
    /// let wrapped = Structural::new(async {});
    /// assert_unpin(&wrapped); // error: `Structural<...>` cannot be unpinned
    /// ```
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Structural<F> {
        #[pin]
        inner: F,
    }
}

impl<F> Structural<F> {
    /// Wraps a future, to be polled where it lies.
    pub fn new(inner: F) -> Self {
        Self { inner }
    }
}

impl<F: Future> Future for Structural<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        self.project().inner.poll(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::{NotStructural, Structural};
    use crate::basic::ready::{Ready, ready};
    use crate::testing::poll_once;
    use std::marker::PhantomPinned;
    use std::pin::Pin;
    use std::task::{Poll, Waker};

    fn assert_unpin<T: Unpin>() {}

    #[test]
    fn the_non_structural_wrapper_is_unpin_whatever_it_holds() {
        assert_unpin::<NotStructural<PhantomPinned>>();

        // Which is what lets it be polled through `Pin::new`, with no boxing.
        let mut fut = NotStructural::new(PhantomPinned);
        assert_eq!(
            poll_once(Pin::new(&mut fut), Waker::noop()),
            Poll::Ready(PhantomPinned)
        );
    }

    #[test]
    fn the_structural_wrapper_is_unpin_when_the_future_is() {
        // The negative case cannot be written as an assertion, so it is a
        // `compile_fail` doctest on `Structural` instead.
        assert_unpin::<Structural<Ready<i32>>>();
    }

    #[test]
    fn the_structural_wrapper_polls_the_future_where_it_lies() {
        let mut fut = Box::pin(Structural::new(ready(42)));
        assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(42));
    }
}

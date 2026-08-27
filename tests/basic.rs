//! Tests for the basic futures, including the auto-trait behaviour their
//! marker-type choices are meant to guarantee.

use futures_patterns::basic::pending::{Pending, pending};
use futures_patterns::basic::poll_fn::poll_fn;
use futures_patterns::basic::ready::ready;
use futures_patterns::testing::{CountingWaker, poll_once};
use std::marker::PhantomPinned;
use std::rc::Rc;
use std::task::{Poll, Waker};

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}
fn assert_unpin<T: Unpin>() {}

#[test]
fn ready_completes_on_first_poll() {
    let mut fut = Box::pin(ready(42));
    assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(42));
}

#[test]
#[should_panic(expected = "Ready polled after completion")]
fn ready_panics_when_polled_after_completion() {
    let mut fut = Box::pin(ready(42));
    let _ = poll_once(fut.as_mut(), Waker::noop());
    let _ = poll_once(fut.as_mut(), Waker::noop());
}

#[test]
fn pending_never_completes_and_never_wakes() {
    let waker = CountingWaker::new();
    let mut fut = Box::pin(pending::<i32>());

    assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Pending);
    assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Pending);
    // Waking would be wrong: no progress is ever possible, so a wake would only
    // spin the scheduler.
    assert_eq!(waker.count(), 0);
}

#[test]
fn pending_auto_traits_do_not_depend_on_t() {
    // `Pending<T>` stores no `T`, so its auto traits should not follow `T`. This
    // holds only because the marker is `PhantomData<fn() -> T>` rather than
    // `PhantomData<T>`, matching `std::future::Pending`.
    assert_send::<Pending<Rc<()>>>();
    assert_sync::<Pending<Rc<()>>>();
    assert_unpin::<Pending<PhantomPinned>>();
}

#[test]
fn pending_is_zero_sized_and_copyable() {
    assert_eq!(std::mem::size_of::<Pending<[u8; 1024]>>(), 0);
    // Copy, not just Clone, and without requiring `T: Clone`.
    let a = pending::<Rc<()>>();
    let _b = a;
    let _c = a;
}

#[test]
fn poll_fn_calls_the_closure_on_every_poll() {
    let waker = CountingWaker::new();
    let mut polls = 0;
    let mut fut = Box::pin(poll_fn(|cx| {
        polls += 1;
        if polls >= 3 {
            Poll::Ready(polls)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }));

    assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Pending);
    assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Pending);
    assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Ready(3));
    assert_eq!(waker.count(), 2, "one wake per pending poll");
}

#[test]
fn alt_poll_fn_polls_like_the_manual_version() {
    // That `alt::PollFn` is unconditionally `!Unpin` cannot be asserted positively
    // at runtime; a `compile_fail` doctest on the module covers it instead.
    let mut fut = Box::pin(futures_patterns::basic::poll_fn::alt::poll_fn(|_cx| {
        Poll::Ready(7)
    }));
    assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(7));
}

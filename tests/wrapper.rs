//! Tests for the newtype wrappers, focused on the pinning rules they demonstrate.

use futures_patterns::basic::ready::{Ready, ready};
use futures_patterns::basic::wrapper::{Finished, OptionFuture, SimpleOptionFuture};
use futures_patterns::testing::poll_once;
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
    let (output, _) = futures_patterns::testing::poll_until_ready(Box::pin(inner).as_mut(), 2);
    assert_eq!(output, 42);
}

#[test]
fn clear_drops_a_pinned_future_in_place() {
    // std::future::pending is !Unpin-safe to hold pinned and never completes, so a
    // successful Ready(None) proves the slot was emptied rather than polled.
    let mut fut = Box::pin(OptionFuture::new(Some(std::future::pending::<i32>())));
    assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);

    fut.as_mut().clear();
    assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(None));
}

#[test]
fn clear_runs_the_inner_futures_destructor() {
    use std::sync::Arc;

    // Dropping the Arc clone held by the future is observable via the strong count,
    // which is how we know `clear` destroys in place rather than leaking.
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

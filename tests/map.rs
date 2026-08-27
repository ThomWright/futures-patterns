//! Tests for the `Map` combinator.

use futures_patterns::basic::ready::ready;
use futures_patterns::composition::map::{Map, map};
use futures_patterns::state_machine::two_state::CountDown;
use futures_patterns::testing::{poll_once, poll_until_ready};
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

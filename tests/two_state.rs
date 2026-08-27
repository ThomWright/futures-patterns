//! Poll-level tests for the `CountDown` state machine.
//!
//! `CountDown`'s contract is entirely about *how many times* it is polled and
//! what it wakes, so these tests drive it by hand rather than with `.await`.

use futures_patterns::state_machine::two_state::CountDown;
use futures_patterns::testing::{CountingWaker, poll_once, poll_until_ready};
use std::task::Poll;

#[test]
fn yields_the_original_count_not_the_remaining_one() {
    let (output, _) = poll_until_ready(Box::pin(CountDown::new(3)).as_mut(), 10);
    assert_eq!(output, 3);
}

#[test]
fn is_pending_for_exactly_count_polls() {
    let (output, polls) = poll_until_ready(Box::pin(CountDown::new(3)).as_mut(), 10);
    // Documented contract: Pending for `count` polls, then Ready on the next.
    assert_eq!(polls, 4, "expected 3 pending polls then 1 ready poll");
    assert_eq!(output, 3);
}

#[test]
fn wakes_the_task_on_every_pending_poll() {
    let waker = CountingWaker::new();
    let mut future = Box::pin(CountDown::new(2));

    assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Pending);
    assert_eq!(waker.count(), 1, "a Pending poll must arrange to be re-polled");

    assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Pending);
    assert_eq!(waker.count(), 2);

    assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Ready(2));
    assert_eq!(waker.count(), 2, "a Ready poll must not wake the task");
}

#[test]
fn zero_is_ready_immediately_without_waking() {
    let waker = CountingWaker::new();
    let mut future = Box::pin(CountDown::new(0));

    assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Ready(0));
    assert_eq!(waker.count(), 0);
}

#[test]
fn stays_ready_when_polled_after_completion() {
    let mut future = Box::pin(CountDown::new(1));
    let waker = CountingWaker::new();

    assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Pending);
    assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Ready(1));
    // CountDown documents itself as idempotent, unlike Ready and Map which panic.
    assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Ready(1));
    assert_eq!(poll_once(future.as_mut(), &waker.waker()), Poll::Ready(1));
}

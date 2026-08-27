//! Tests for the `Race` combinator, focused on its documented left-bias.

use futures_patterns::basic::pending::pending;
use futures_patterns::basic::poll_fn::poll_fn;
use futures_patterns::basic::ready::ready;
use futures_patterns::composition::race::{Either, race};
use futures_patterns::state_machine::two_state::CountDown;
use futures_patterns::testing::{CountingWaker, poll_once};
use std::task::{Poll, Waker};

#[test]
fn left_wins_when_both_are_ready() {
    let mut fut = Box::pin(race(ready(1), ready(2)));
    assert_eq!(
        poll_once(fut.as_mut(), Waker::noop()),
        Poll::Ready(Either::Left(1))
    );
}

#[test]
fn does_not_poll_the_right_future_when_the_left_is_ready() {
    // The documented consequence of left-bias: `right` is never touched at all.
    // A closure that panics on poll makes that observable.
    let right = poll_fn(|_cx| -> Poll<i32> { panic!("right must not be polled") });
    let mut fut = Box::pin(race(ready(1), right));
    assert_eq!(
        poll_once(fut.as_mut(), Waker::noop()),
        Poll::Ready(Either::Left(1))
    );
}

#[test]
fn right_wins_when_the_left_is_pending() {
    let mut fut = Box::pin(race(pending::<i32>(), ready(2)));
    assert_eq!(
        poll_once(fut.as_mut(), Waker::noop()),
        Poll::Ready(Either::Right(2))
    );
}

#[test]
fn is_pending_while_both_are_pending() {
    let mut fut = Box::pin(race(pending::<i32>(), pending::<i32>()));
    assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Pending);
}

#[test]
fn polls_both_futures_while_both_are_pending() {
    // Each CountDown wakes once per pending poll, so two wakes from a single poll of
    // Race proves both branches were polled and both registered the waker.
    let waker = CountingWaker::new();
    let mut fut = Box::pin(race(CountDown::new(5), CountDown::new(5)));

    assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Pending);
    assert_eq!(waker.count(), 2);
}

#[test]
fn right_wins_when_it_completes_first() {
    // Left needs three polls, right needs one, so right wins despite being second.
    let mut fut = Box::pin(race(CountDown::new(3), CountDown::new(1)));
    let waker = CountingWaker::new();

    assert_eq!(poll_once(fut.as_mut(), &waker.waker()), Poll::Pending);
    assert_eq!(
        poll_once(fut.as_mut(), &waker.waker()),
        Poll::Ready(Either::Right(1))
    );
}

#[test]
fn branches_may_have_different_output_types() {
    let mut fut = Box::pin(race(ready("left"), ready(2u8)));
    assert_eq!(
        poll_once(fut.as_mut(), Waker::noop()),
        Poll::Ready(Either::Left("left"))
    );
}

//! Tests for `MaybeDone`.
//!
//! `MaybeDone` splits "has it finished?" from "give me the value", because those
//! happen at different times when coordinating several futures. The final test
//! builds a two-branch join to show why that split is needed.

use futures_patterns::basic::ready::{Ready, ready};
use futures_patterns::state_machine::maybe_done::{MaybeDone, maybe_done};
use futures_patterns::state_machine::two_state::CountDown;
use futures_patterns::testing::poll_once;
use std::pin::Pin;
use std::task::{Poll, Waker};

#[test]
fn polling_to_completion_yields_unit_not_the_output() {
    // The output is deliberately not the inner future's; it is retrieved separately.
    let mut fut = Box::pin(maybe_done(ready(42)));
    assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(()));
}

#[test]
fn take_output_returns_none_before_completion() {
    let mut fut = Box::pin(maybe_done(CountDown::new(3)));
    assert_eq!(fut.as_mut().take_output(), None);
}

#[test]
fn take_output_yields_the_value_once_then_none() {
    let mut fut = Box::pin(maybe_done(ready(42)));
    assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(()));

    assert_eq!(fut.as_mut().take_output(), Some(42));
    // Now in the Gone state: the value has been harvested and cannot be taken twice.
    assert_eq!(fut.as_mut().take_output(), None);
}

#[test]
fn done_absorbs_further_polls_without_touching_the_inner_future() {
    // This is what makes a uniform "poll every branch" loop legal in `join!`.
    // `ready` panics if polled twice, so completing without a panic proves the
    // inner future was not re-polled.
    let mut fut = Box::pin(maybe_done(ready(42)));
    assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(()));
    assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(()));
    assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(()));
    assert_eq!(fut.as_mut().take_output(), Some(42));
}

#[test]
#[should_panic(expected = "MaybeDone polled after value taken")]
fn polling_after_the_value_is_taken_panics() {
    let mut fut = Box::pin(maybe_done(ready(42)));
    let _ = poll_once(fut.as_mut(), Waker::noop());
    let _ = fut.as_mut().take_output();
    let _ = poll_once(fut.as_mut(), Waker::noop());
}

#[test]
fn output_mut_allows_editing_the_stored_value() {
    let mut fut = Box::pin(maybe_done(ready(42)));
    assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(()));

    *fut.as_mut().output_mut().expect("should be Done") += 1;
    assert_eq!(fut.as_mut().take_output(), Some(43));
}

#[test]
fn output_mut_is_none_unless_done() {
    let mut fut = Box::pin(maybe_done(CountDown::new(3)));
    assert!(fut.as_mut().output_mut().is_none());
}

/// A two-branch join, built the way `tokio::join!` builds an N-branch one.
///
/// Both branches are polled on every round, completed branches park their output,
/// and the outputs are harvested together at the end. Awaiting the branches in
/// sequence instead would run them one after the other.
fn join2<A: Future, B: Future>(a: A, b: B) -> (A::Output, B::Output, usize) {
    let mut a = Box::pin(maybe_done(a));
    let mut b = Box::pin(maybe_done(b));

    let mut rounds = 0;
    loop {
        rounds += 1;
        let a_done = poll_once(a.as_mut(), Waker::noop()).is_ready();
        let b_done = poll_once(b.as_mut(), Waker::noop()).is_ready();
        if a_done && b_done {
            break;
        }
        assert!(rounds < 100, "join did not converge");
    }

    (
        a.as_mut().take_output().expect("a completed"),
        b.as_mut().take_output().expect("b completed"),
        rounds,
    )
}

#[test]
fn joins_two_futures_concurrently() {
    // Three polls and five polls respectively. Run concurrently the join takes
    // max(3, 5) + 1 rounds, not the 9 that awaiting them in sequence would need.
    let (a, b, rounds) = join2(CountDown::new(3), CountDown::new(5));
    assert_eq!((a, b), (3, 5));
    assert_eq!(rounds, 6);
}

#[test]
fn join_handles_branches_with_different_output_types() {
    let (a, b, _) = join2(ready("left"), CountDown::new(2));
    assert_eq!(a, "left");
    assert_eq!(b, 2);
}

#[test]
fn unpin_is_broader_than_the_derived_impl_would_be() {
    // The manual impl requires only `Fut: Unpin`. A derived one would additionally
    // demand `Fut::Output: Unpin`, because `Done` holds an output.
    fn assert_unpin<T: Unpin>() {}
    assert_unpin::<MaybeDone<Ready<std::marker::PhantomPinned>>>();
}


#[test]
fn can_be_polled_through_a_plain_pin_when_unpin() {
    // A consequence of that Unpin impl: no boxing needed.
    let mut fut = maybe_done(ready(1));
    let pinned = Pin::new(&mut fut);
    assert_eq!(poll_once(pinned, Waker::noop()), Poll::Ready(()));
}

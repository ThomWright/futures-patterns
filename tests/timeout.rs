//! Tests for `Timeout`.
//!
//! These run on a paused clock (`start_paused = true`), so they exercise real
//! deadline behaviour without sleeping in real time. Tokio auto-advances the clock
//! when every task is idle, which is what lets an expiring timer resolve instantly.

use futures_patterns::testing::poll_once;
use futures_patterns::time::timeout::{Elapsed, timeout, timeout_at};
use std::error::Error;
use std::future::pending;
use std::task::{Poll, Waker};
use std::time::Duration;
use tokio::time::Instant;

#[tokio::test(start_paused = true)]
async fn completes_with_ok_when_the_future_finishes_in_time() {
    assert_eq!(timeout(Duration::from_secs(30), async { 42 }).await, Ok(42));
}

#[tokio::test(start_paused = true)]
async fn completes_with_elapsed_when_the_deadline_passes_first() {
    let result = timeout(Duration::from_secs(30), pending::<i32>()).await;
    assert_eq!(result, Err(Elapsed));
}

#[tokio::test(start_paused = true)]
async fn a_slow_future_still_wins_if_it_finishes_before_the_deadline() {
    let result = timeout(Duration::from_secs(30), async {
        tokio::time::sleep(Duration::from_secs(10)).await;
        42
    })
    .await;
    assert_eq!(result, Ok(42));
}

#[tokio::test(start_paused = true)]
async fn the_value_is_polled_before_the_timer() {
    // The deadline has already passed, but the value is ready on its first poll and
    // is polled first, so it wins. This is the documented polling order.
    let mut fut = Box::pin(timeout(Duration::ZERO, async { 42 }));
    tokio::time::sleep(Duration::from_secs(1)).await;

    assert_eq!(poll_once(fut.as_mut(), Waker::noop()), Poll::Ready(Ok(42)));
}

#[tokio::test(start_paused = true)]
async fn is_pending_while_neither_the_value_nor_the_timer_is_ready() {
    let mut fut = Box::pin(timeout(Duration::from_secs(30), pending::<i32>()));
    assert!(poll_once(fut.as_mut(), Waker::noop()).is_pending());
}

#[tokio::test(start_paused = true)]
async fn timeout_at_uses_an_absolute_deadline() {
    let deadline = Instant::now() + Duration::from_secs(30);
    assert_eq!(timeout_at(deadline, async { 42 }).await, Ok(42));
}

#[tokio::test(start_paused = true)]
async fn timeout_at_expires_for_a_deadline_already_in_the_past() {
    let deadline = Instant::now() - Duration::from_secs(1);
    assert_eq!(timeout_at(deadline, pending::<i32>()).await, Err(Elapsed));
}

#[tokio::test(start_paused = true)]
async fn into_inner_recovers_the_future_so_it_can_be_rewrapped() {
    // Cancelling the timeout without discarding the operation: drop the wrapper,
    // keep the future, give it a new deadline.
    let first = timeout(Duration::from_secs(1), async { 42 });
    let operation = first.into_inner();

    assert_eq!(timeout(Duration::from_secs(30), operation).await, Ok(42));
}

#[tokio::test(start_paused = true)]
async fn exposes_the_inner_future_by_reference() {
    let mut fut = timeout(Duration::from_secs(1), async { 42 });
    // Sanity: the accessors compile and refer to the wrapped future.
    let _: &_ = fut.get_ref();
    let _: &mut _ = fut.get_mut();
}

#[test]
fn elapsed_is_a_std_error_with_a_readable_message() {
    let err = Elapsed::new();
    assert_eq!(err.to_string(), "deadline has elapsed");
    // Usable with `?` and error-reporting machinery.
    let boxed: Box<dyn Error> = Box::new(err);
    assert_eq!(boxed.to_string(), "deadline has elapsed");
}

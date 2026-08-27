//! Require a future to complete within a time limit.
//!
//! The `Timeout` pattern races a future against a timer. If the future completes
//! first, its value is returned as `Ok`. If the timer expires first, an `Err`
//! is returned. This is a practical application of the race pattern with tokio's
//! timer infrastructure.
//!
//! # Pattern overview
//!
//! Timeout demonstrates:
//! - Racing futures with different output types
//! - Integrating with tokio's timer system (Sleep)
//! - Result transformation (wrapping success/timeout cases)
//! - Cooperative scheduling considerations
//! - How polling order affects behavior
//!
//! # Polling strategy
//!
//! This implementation polls the value future first, then the delay. This means:
//! - If the value is ready, we return immediately without checking the timer
//! - This is more efficient when the operation completes quickly
//! - It matches tokio's implementation strategy
//!
//! # Cooperative scheduling: deliberately omitted
//!
//! Tokio's real `timeout` does something this implementation does not, and the gap
//! is worth understanding.
//!
//! Tokio gives each task a budget of roughly 128 operations per poll. Once it is
//! exhausted, resource futures return `Pending` to force the task to yield, even if
//! their work could have completed. `Sleep` takes part in that budget. So if the
//! inner future burns the whole budget before returning `Pending`, polling the delay
//! can return `Pending` *because the budget ran out* rather than because the deadline
//! is still in the future -- and an expired timeout goes unnoticed until some later
//! poll. Tokio's own comment calls this the pathological case "where the underlying
//! future always exhausts the budget and we never get a chance to evaluate whether
//! the timeout was hit or not".
//!
//! Tokio handles it by noticing that the budget was drained by the inner future and
//! then polling the delay with an unconstrained budget:
//!
//! ```text
//! let had_budget_before = coop::has_budget_remaining();
//! if let Poll::Ready(v) = me.value.poll(cx) { return Poll::Ready(Ok(v)); }
//! let has_budget_now = coop::has_budget_remaining();
//!
//! if let (true, false) = (had_budget_before, has_budget_now) {
//!     coop::with_unconstrained(poll_delay)
//! } else {
//!     poll_delay()
//! }
//! ```
//!
//! This crate cannot reproduce that: `coop` is `pub(crate)` inside tokio, so
//! `has_budget_remaining` and `with_unconstrained` are unreachable from outside. The
//! budget is also never a factor for the futures used here, none of which consume it.
//!
//! The consequence is that this `timeout` can under-report expiry when wrapped around
//! a future that drains the budget. Prefer [`tokio::time::timeout`] in production.
//!
//! See `tokio/src/time/timeout.rs` for the original.
//!
//! # When to use
//!
//! Use this pattern for:
//! - Network operations that might hang
//! - User interactions with time limits
//! - Preventing indefinite blocking
//! - Implementing retry logic with deadlines
//!
//! # Example
//!
//! ```
//! use futures_patterns::time::timeout::{timeout, Elapsed};
//! use std::time::Duration;
//!
//! # #[tokio::main]
//! # async fn main() {
//! // Fast operation - completes before timeout
//! let result = timeout(Duration::from_secs(1), async {
//!     42
//! }).await;
//! assert_eq!(result, Ok(42));
//!
//! // Slow operation - times out
//! let result = timeout(Duration::from_millis(10), async {
//!     tokio::time::sleep(Duration::from_secs(1)).await;
//!     100
//! }).await;
//! assert!(result.is_err());
//! # }
//! ```

use pin_project_lite::pin_project;
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{sleep, Sleep};

/// Error returned when a timeout expires.
///
/// This indicates that the operation didn't complete within the specified time limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elapsed;

impl fmt::Display for Elapsed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "deadline has elapsed")
    }
}

impl Error for Elapsed {}

impl Elapsed {
    /// Creates a new `Elapsed` error.
    pub fn new() -> Self {
        Elapsed
    }
}

impl Default for Elapsed {
    fn default() -> Self {
        Elapsed
    }
}

pin_project! {
    /// Future returned by [`timeout`].
    ///
    /// This future completes with `Ok(T)` if the inner future completes in time,
    /// or `Err(Elapsed)` if the timeout expires first.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Timeout<T> {
        // The future we're racing against time
        #[pin]
        value: T,

        // The tokio sleep timer
        #[pin]
        delay: Sleep,
    }
}

impl<T> Timeout<T> {
    /// Creates a new `Timeout` with the given future and delay.
    ///
    /// This is an internal constructor. Use the [`timeout`] function instead.
    fn new(value: T, delay: Sleep) -> Self {
        Timeout { value, delay }
    }

    /// Gets a reference to the underlying value in this timeout.
    pub fn get_ref(&self) -> &T {
        &self.value
    }

    /// Gets a mutable reference to the underlying value in this timeout.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.value
    }

    /// Consumes this timeout, returning the underlying value.
    ///
    /// This allows extracting the inner future without waiting for completion.
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> Future for Timeout<T>
where
    T: Future,
{
    type Output = Result<T::Output, Elapsed>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        // First, try polling the value future.
        // If it completes, return immediately without checking the timeout.
        if let Poll::Ready(value) = this.value.poll(cx) {
            return Poll::Ready(Ok(value));
        }

        // The value isn't ready yet - check whether we have timed out.
        //
        // Tokio guards this poll with cooperative-budget handling, which is omitted
        // here; see the module docs for what that costs.
        match this.delay.poll(cx) {
            Poll::Ready(()) => {
                // Timer expired - timeout!
                Poll::Ready(Err(Elapsed::new()))
            }
            Poll::Pending => {
                // Both value and timer are pending
                // Wakers have been registered by both poll calls
                Poll::Pending
            }
        }
    }
}

/// Requires a future to complete before the specified duration has elapsed.
///
/// If the future completes before the duration has elapsed, then the completed
/// value is returned as `Ok`. Otherwise, an `Err` containing [`Elapsed`] is returned.
///
/// # Cancellation
///
/// Canceling a timeout is done by dropping the returned future. No additional
/// cleanup work is required. The inner future can be extracted with
/// [`Timeout::into_inner`].
///
/// # Example
///
/// ```
/// use futures_patterns::time::timeout::timeout;
/// use std::time::Duration;
///
/// # #[tokio::main]
/// # async fn main() {
/// let operation = async {
///     // Some async work
///     42
/// };
///
/// match timeout(Duration::from_millis(100), operation).await {
///     Ok(result) => println!("Got result: {}", result),
///     Err(_) => println!("Operation timed out"),
/// }
/// # }
/// ```
///
/// # Note
///
/// This function requires a tokio runtime to be active, as it uses tokio's
/// timer infrastructure via [`tokio::time::sleep`].
pub fn timeout<F>(duration: Duration, future: F) -> Timeout<F>
where
    F: Future,
{
    let delay = sleep(duration);
    Timeout::new(future, delay)
}

/// Requires a future to complete before the specified deadline.
///
/// Similar to [`timeout`], but uses an absolute deadline instead of a duration.
///
/// # Example
///
/// ```
/// use futures_patterns::time::timeout::timeout_at;
/// use std::time::Duration;
/// use tokio::time::Instant;
///
/// # #[tokio::main]
/// # async fn main() {
/// let deadline = Instant::now() + Duration::from_secs(1);
/// let operation = async { 42 };
///
/// match timeout_at(deadline, operation).await {
///     Ok(result) => println!("Completed: {}", result),
///     Err(_) => println!("Timed out"),
/// }
/// # }
/// ```
pub fn timeout_at<F>(deadline: tokio::time::Instant, future: F) -> Timeout<F>
where
    F: Future,
{
    let delay = tokio::time::sleep_until(deadline);
    Timeout::new(future, delay)
}

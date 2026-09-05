#![doc = include_str!("../README.md")]
//!
//! ## Key concepts
//!
//! ### Pinning
//!
//! An `async fn` compiles to a state machine that can hold references into itself, so
//! moving one after polling has begun would leave those references dangling.
//! `Pin<&mut F>` is the promise that it will not move. `Unpin` marks the types that need
//! no such promise -- most of them -- and for those a `Pin` is inert.
//!
//! A wrapper holding a future then decides whether its own pin reaches through to the
//! field. [`advanced::pinning`] works through that choice and what each answer commits
//! the wrapper to. [`composition::map`], [`composition::race`] and [`time::timeout`]
//! take one answer with `pin-project-lite`; [`basic::ready`] and [`basic::pending`] take
//! the other; [`advanced::poll_fn`] reaches for `unsafe` because neither fits.
//!
//! ### State management
//!
//! Futures are state machines. The state machine patterns show how to:
//!
//! - Use enums to represent different states
//! - Transition between states during polling
//! - Store and extract values at different stages
//!
//! ### Polling after completion
//!
//! Once a future returns `Poll::Ready`, the caller must not poll it again. `.await`
//! and the runtime both honour this, so the rule only comes up when writing a
//! combinator that drives futures itself.
//!
//! `Future::poll`'s own documentation is blunt about what happens if you break it:
//! calling `poll` again "may panic, block forever, or cause other kinds of problems;
//! the `Future` trait places no requirements on the effects of such a call". The one
//! hard limit is that it must not be undefined behaviour, since `poll` is not
//! `unsafe`.
//!
//! This is worth being clear about, because it is easy to read the behaviours below
//! as a *lifecycle* the trait sanctions. It is not one. There is no blessed
//! `Pending -> Ready -> Pending` sequence; a future returning `Pending` after it has
//! completed is simply an implementation deciding what to do about a call that should
//! never have happened. Every option below is a choice about unspecified behaviour:
//!
//! - [`basic::ready`] and [`composition::map`] panic. Both consume something on.
//!   completion -- a value, an `FnOnce` -- so there is nothing left to return, and a
//!   panic beats a silent wrong answer.
//! - [`state_machine::two_state`] keeps returning the same value. It is idempotent.
//!   because it stores its output rather than moving it out.
//! - [`state_machine::maybe_done`] absorbs the extra poll without touching the inner.
//!   future. This is the load-bearing one: it is what lets `join!` poll every branch
//!   on every wakeup without re-polling the branches that already finished.
//! - `Fuse`, from the `futures` crate, returns `Pending` from then on, so a finished
//!   branch quietly drops out of a `select!` loop instead of blowing it up.
//!
//! Two of these are worth reaching for deliberately, and which you want depends on
//! *when the output should be delivered relative to completion*.
//! [`state_machine::maybe_done`] decouples "it has finished" from "give me the value";
//! [`composition::fuse`] fuses the two into a single event.
//!
//! |             | the completing poll                        | every poll after that          |
//! |-------------|--------------------------------------------|--------------------------------|
//! | `MaybeDone` | `Ready(())`; output withheld and kept     | `Ready(())`; inner untouched   |
//! | `Fuse`      | `Ready(output)`; output out, inner dropped | `Pending`; no waker registered |
//!
//! Neither generalises the other, and each module explains why.
//!
//! Finally, a type can advertise its choice generically through
//! [`fused::FusedFuture`], which is what lets a caller that does not know the concrete
//! type decide whether polling is worthwhile.
//!
//! ### Wakers
//!
//! When a future returns `Poll::Pending`, it must arrange for the task to be
//! woken when progress can be made. The patterns show:
//!
//! - When to call `wake()` (in `two_state`)
//! - When NOT to call `wake()` (in `pending`)
//! - How wakers are handled by composed futures
//! - How to store one so another thread can wake you (in `waking::shared_state`),
//!   which is the case the other three avoid
//!
//! ## Examples
//!
//! ### Basic usage
//!
//! ```
//! use futures_patterns::basic::ready::ready;
//! use futures_patterns::advanced::poll_fn::poll_fn;
//! use std::task::Poll;
//!
//! # #[tokio::main]
//! # async fn main() {
//! // Create a future that's immediately ready
//! let value = ready(42).await;
//! assert_eq!(value, 42);
//!
//! // Create a custom future with a closure
//! let custom = poll_fn(|_cx| Poll::Ready("done"));
//! assert_eq!(custom.await, "done");
//! # }
//! ```
//!
//! ### Composition
//!
//! ```
//! use futures_patterns::composition::map::map;
//! use futures_patterns::composition::race::{race, Either};
//! use futures_patterns::basic::ready::ready;
//!
//! # #[tokio::main]
//! # async fn main() {
//! // Transform a future's output
//! let doubled = map(ready(21), |x| x * 2);
//! assert_eq!(doubled.await, 42);
//!
//! // Race two futures
//! let result = race(ready(1), ready(2)).await;
//! match result {
//!     Either::Left(v) => assert_eq!(v, 1),
//!     Either::Right(v) => println!("Right: {}", v),
//! }
//! # }
//! ```
//!
//! ### Timeouts
//!
//! ```
//! use futures_patterns::time::timeout::timeout;
//! use std::time::Duration;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let operation = async {
//!     // Some async work
//!     42
//! };
//!
//! match timeout(Duration::from_secs(1), operation).await {
//!     Ok(result) => println!("Completed: {}", result),
//!     Err(_) => println!("Timed out"),
//! }
//! # }
//! ```
//!
pub mod advanced;
pub mod basic;
pub mod composition;
pub mod fused;
pub mod state_machine;
pub mod testing;
pub mod time;
pub mod waking;

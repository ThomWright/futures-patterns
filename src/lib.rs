//! Async patterns built on `poll`, `Pin` and `Waker`, explained.
//!
//! An `async fn` hides the state machine the compiler generates for it. Writing
//! `poll` directly puts `Pin`, `Waker` and the poll contract back in view, which is
//! the point: every pattern here exists to explain one of them.
//!
//! Some follow a production implementation closely and say which. Others are invented
//! to isolate a single idea. Either way each is documented with the concepts it
//! depends on, the trade-offs it makes, and what it simplifies.
//!
//! # Organisation
//!
//! The patterns are organised by complexity:
//!
//! ## Basic patterns
//!
//! Start here to understand the fundamentals of the Future trait:
//!
//! - [`basic::ready`] - A future that immediately returns a value.
//! - [`basic::pending`] - A future that never completes.
//! - [`basic::poll_fn`] - Wrap a closure into a future.
//! - [`basic::wrapper`] - Wrap an existing future in a newtype.
//!
//! These demonstrate the basic structure of futures and introduce concepts like
//! polling, wakers, and pinning.
//!
//! ## State machine patterns
//!
//! Learn how to build futures with internal state transitions:
//!
//! - [`state_machine::maybe_done`] - Track whether a future has completed.
//! - [`state_machine::two_state`] - Simple countdown state machine.
//!
//! State machines are fundamental to implementing complex async behaviour. These
//! examples show how to use enums to represent different states and manage
//! transitions during polling.
//!
//! ## Waking
//!
//! Where readiness comes from in the first place:
//!
//! - [`waking::shared_state`] - A future woken by another thread.
//!
//! Every other pattern here completes immediately, wakes itself, or forwards a poll
//! to a future underneath it. This one parks and is woken by something external,
//! which is what leaf futures do and what the rest are ultimately built on.
//!
//! ## Composition patterns
//!
//! Build futures that drive other futures:
//!
//! - [`composition::map`] - Transform a future's output.
//! - [`composition::race`] - Return the first of two futures to complete.
//! - [`composition::join`] - Wait for two futures and collect both outputs.
//! - [`composition::try_join`] - The same, but stop at the first error.
//! - [`composition::fuse`] - Make polling after completion harmless.
//!
//! Composition is key to building complex async operations from simple pieces.
//! These patterns introduce pin projection and coordinating multiple futures.
//!
//! ## Time-based patterns
//!
//! Work with time and deadlines using tokio's timer infrastructure:
//!
//! - [`time::timeout`] - Require a future to complete within a time limit.
//!
//! This demonstrates integration with runtime services and practical patterns
//! for real-world async code.
//!
//! ## Testing
//!
//! - [`testing`] - Poll futures by hand, and count wakes.
//!
//! `.await` only reveals a future's final output. These helpers drive a future one
//! poll at a time so tests can assert on the whole poll sequence -- how many polls
//! it took, and whether the task was woken when it should have been. That is where
//! the subtle bugs in a `Future` impl actually live.
//!
//! # Key concepts
//!
//! ## Pinning
//!
//! Futures often need to be pinned in memory because they can contain self-referential
//! data. Different patterns demonstrate different pinning strategies:
//!
//! - Manual unsafe pinning (in `poll_fn`)
//! - Conditional `Unpin` implementation (in `maybe_done`)
//! - `pin-project-lite` for safe projection (in `map`, `race`, `timeout`)
//!
//! ## State management
//!
//! Futures are state machines. The state machine patterns show how to:
//!
//! - Use enums to represent different states
//! - Transition between states during polling
//! - Store and extract values at different stages
//!
//! ## Polling after completion
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
//! ## Wakers
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
//! # Examples
//!
//! ## Basic usage
//!
//! ```
//! use futures_patterns::basic::ready::ready;
//! use futures_patterns::basic::poll_fn::poll_fn;
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
//! ## Composition
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
//! ## Timeouts
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
//! # Learning path
//!
//! Recommended order for learning:
//!
//! 1. [`basic::ready`] and [`basic::pending`] -- always ready, and never ready.
//! 2. [`basic::poll_fn`] -- a future from a closure. The first place pinning forces
//!    `unsafe`.
//! 3. [`basic::wrapper`] -- wrapping another future, and pin projection.
//! 4. [`state_machine::two_state`] -- states written out by hand, and asking to be
//!    polled again.
//! 5. [`waking::shared_state`] -- a future that waits until another thread wakes it.
//!    Where readiness comes from.
//! 6. [`state_machine::maybe_done`] -- keeping a finished future's output while the
//!    others catch up.
//! 7. [`composition::map`] -- transforming another future's output.
//! 8. [`composition::race`] -- whichever of two finishes first, and why the polling
//!    order matters.
//! 9. [`composition::join`] -- waiting for both. Then [`composition::try_join`],
//!    where failing early means abandoning a branch.
//! 10. [`composition::fuse`] -- promising more than the `Future` contract requires,
//!     and [`fused`] for saying so.
//! 11. [`time::timeout`] -- racing against the runtime's timer. The first pattern that
//!     needs a runtime.
//!
//! # References
//!
//! Worth reading alongside this:
//!
//! - [tokio](https://github.com/tokio-rs/tokio)
//! - [futures-rs](https://github.com/rust-lang/futures-rs)
//! - The Rust async book
//!
//! A module derived from one of them says so in its own docs, and `NOTICE.md` records
//! which file each follows.

pub mod basic;
pub mod composition;
pub mod fused;
pub mod state_machine;
pub mod testing;
pub mod time;
pub mod waking;

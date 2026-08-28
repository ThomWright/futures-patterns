//! A collection of patterns for implementing Futures in Rust.
//!
//! This crate provides educational implementations of common Future patterns,
//! based on real-world examples from tokio and other production async libraries.
//! Each pattern is implemented with comprehensive documentation explaining the
//! concepts, trade-offs, and implementation details.
//!
//! # Organization
//!
//! The patterns are organized by complexity:
//!
//! ## Basic Patterns
//!
//! Start here to understand the fundamentals of the Future trait:
//!
//! - [`basic::ready`] - A future that immediately returns a value
//! - [`basic::pending`] - A future that never completes
//! - [`basic::poll_fn`] - Wrap a closure into a future
//! - [`basic::wrapper`] - Wrap an existing future in a newtype
//!
//! These demonstrate the basic structure of futures and introduce concepts like
//! polling, wakers, and pinning.
//!
//! ## State Machine Patterns
//!
//! Learn how to build futures with internal state transitions:
//!
//! - [`state_machine::maybe_done`] - Track whether a future has completed
//! - [`state_machine::two_state`] - Simple countdown state machine
//!
//! State machines are fundamental to implementing complex async behavior. These
//! examples show how to use enums to represent different states and manage
//! transitions during polling.
//!
//! ## Waking
//!
//! Where readiness comes from in the first place:
//!
//! - [`waking::shared_state`] - A future woken by another thread
//!
//! Every other pattern here completes immediately, wakes itself, or forwards a poll
//! to a future underneath it. This one parks and is woken by something external,
//! which is what leaf futures do and what the rest are ultimately built on.
//!
//! ## Composition Patterns
//!
//! See how to combine futures to create more powerful abstractions:
//!
//! - [`composition::map`] - Transform a future's output
//! - [`composition::race`] - Return the first of two futures to complete
//! - [`composition::join`] - Wait for two futures and collect both outputs
//! - [`composition::try_join`] - The same, but stop at the first error
//!
//! Composition is key to building complex async operations from simple pieces.
//! These patterns introduce pin projection and coordinating multiple futures.
//!
//! ## Time-Based Patterns
//!
//! Work with time and deadlines using tokio's timer infrastructure:
//!
//! - [`time::timeout`] - Require a future to complete within a time limit
//!
//! This demonstrates integration with runtime services and practical patterns
//! for real-world async code.
//!
//! ## Testing
//!
//! - [`testing`] - Poll futures by hand, and count wakes
//!
//! `.await` only reveals a future's final output. These helpers drive a future one
//! poll at a time so tests can assert on the whole poll sequence -- how many polls
//! it took, and whether the task was woken when it should have been. That is where
//! the subtle bugs in a `Future` impl actually live.
//!
//! # Key Concepts
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
//! ## State Management
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
//! - [`basic::ready`] and [`composition::map`] panic. Both consume something on
//!   completion -- a value, an `FnOnce` -- so there is nothing left to return, and a
//!   panic beats a silent wrong answer.
//! - [`state_machine::two_state`] keeps returning the same value. It is idempotent
//!   because it stores its output rather than moving it out.
//! - [`state_machine::maybe_done`] absorbs the extra poll without touching the inner
//!   future. This is the load-bearing one: it is what lets `join!` poll every branch
//!   on every wakeup without re-polling the branches that already finished.
//! - `Fuse`, from the `futures` crate, returns `Pending` from then on, so a finished
//!   branch quietly drops out of a `select!` loop instead of blowing it up.
//!
//! To get the guarantee from an arbitrary future rather than choosing it per type,
//! there are two tools, and which one you want depends on what a finished future
//! should report and whether its output needs keeping:
//!
//! - [`state_machine::maybe_done`] answers `Ready(())`, meaning "I am finished", and
//!   parks the output for collection later. That is what a join needs: it must know
//!   every branch is done before it can build the result.
//! - `Fuse`, from the `futures` crate, hands the output straight out on the poll where
//!   the inner future completes, keeping nothing, and answers `Pending` on every poll
//!   after that. So the output is delivered once, at completion, and never again. That
//!   is what a select loop needs: a finished branch stops being chosen.
//!
//! Note that `Fuse`'s later `Pending` registers no waker, so awaiting a finished
//! `Fuse` on its own hangs exactly like [`basic::pending`]. It is safe inside a
//! `select!` only because the other branches get the task woken, which makes `Fuse` a
//! select-loop component rather than a general "safe to poll again" wrapper.
//!
//! The difference is really about *when the output is delivered relative to
//! completion*: `MaybeDone` decouples "it has finished" from "give me the value",
//! while `Fuse` fuses the two into a single event.
//!
//! |                 | the completing poll                          | every poll after that            |
//! |-----------------|----------------------------------------------|----------------------------------|
//! | `MaybeDone`     | `Ready(())`; output withheld and parked      | `Ready(())`; inner untouched     |
//! | `Fuse`          | `Ready(output)`; output out, inner dropped   | `Pending`; no waker registered   |
//!
//! Neither generalises the other. A join cannot be built on `Fuse`, because there is
//! nowhere to park an output while a slower branch runs; a select loop built on
//! `MaybeDone` would spin on branches answering `Ready(())`.
//!
//! `futures` also pairs `Fuse` with a `FusedFuture::is_terminated` method, so a
//! well-behaved `select!` can skip finished branches rather than poll them and rely on
//! the `Pending`. The graceful `Pending` is the safety net, not the mechanism.
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
//! # Learning Path
//!
//! Recommended order for learning:
//!
//! 1. Start with [`basic::ready`] and [`basic::pending`] to understand the Future trait
//! 2. Study [`basic::poll_fn`] to learn about pinning and unsafe usage
//! 3. Read [`basic::wrapper`] for the pinning rules around wrapping a future
//! 4. Move to [`state_machine::two_state`] for a simple state machine
//! 5. Build a leaf future in [`waking::shared_state`] to see where readiness
//!    comes from, and how a waker gets stored for another thread to find
//! 6. Examine [`state_machine::maybe_done`] for a production-like pattern
//! 7. Learn composition with [`composition::map`]
//! 8. Study coordination with [`composition::race`]
//! 9. See [`composition::join`] for the other way to coordinate two futures, and
//!    the payoff that justifies [`state_machine::maybe_done`]; then
//!    [`composition::try_join`], where failing early means abandoning a branch
//! 10. Finish with [`time::timeout`] to see everything combined
//!
//! # References
//!
//! These implementations are based on patterns from:
//!
//! - [tokio](https://github.com/tokio-rs/tokio) - Production async runtime
//! - The Rust async book
//! - Real-world async codebases
//!
//! Each module includes references to the original implementations where applicable.

pub mod basic;
pub mod composition;
pub mod state_machine;
pub mod testing;
pub mod time;
pub mod waking;

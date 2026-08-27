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
//! ## Composition Patterns
//!
//! See how to combine futures to create more powerful abstractions:
//!
//! - [`composition::map`] - Transform a future's output
//! - [`composition::race`] - Return the first of two futures to complete
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
//! ## Wakers
//!
//! When a future returns `Poll::Pending`, it must arrange for the task to be
//! woken when progress can be made. The patterns show:
//!
//! - When to call `wake()` (in `two_state`)
//! - When NOT to call `wake()` (in `pending`)
//! - How wakers are handled by composed futures
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
//! 3. Move to [`state_machine::two_state`] for a simple state machine
//! 4. Examine [`state_machine::maybe_done`] for a production-like pattern
//! 5. Learn composition with [`composition::map`]
//! 6. Study coordination with [`composition::race`]
//! 7. Finish with [`time::timeout`] to see everything combined
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

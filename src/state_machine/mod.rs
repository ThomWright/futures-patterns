//! Futures as explicit state machines.
//!
//! An `async` block compiles into a state machine. Writing one by hand means naming
//! the states yourself, usually as an enum, and moving between them inside `poll`.
//!
//! [`two_state`] is the smaller example: a counter that reports `Pending` until it
//! is exhausted. [`maybe_done`] is the production shape, and the one worth
//! understanding -- it is what `tokio::join!` is built from.

pub mod maybe_done;
pub mod two_state;

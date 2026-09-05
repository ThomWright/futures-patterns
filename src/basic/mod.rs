//! The fundamentals of the `Future` trait.
//!
//! These patterns introduce polling, wakers and pinning with as little machinery as
//! possible. [`ready`] and [`pending`] are the two degenerate cases -- always
//! complete, never complete -- and between them they show what a `poll` method has
//! to decide. [`yield_now`] takes the case in between, not ready yet but able to
//! continue at once, which is where a future has to arrange its own wake.
//! [`poll_fn`] then builds a future from a closure, which is where
//! pinning and `unsafe` first become unavoidable. [`wrapper`] covers wrapping an
//! existing future in a type of your own.

pub mod ready;
pub mod pending;
pub mod yield_now;
pub mod poll_fn;
pub mod wrapper;

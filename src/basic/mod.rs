//! The fundamentals of the `Future` trait.
//!
//! These patterns introduce polling, wakers and pinning with as little machinery as
//! possible. [`ready`] and [`pending`] are the two degenerate cases -- always
//! complete, never complete -- and between them they show what a `poll` method has
//! to decide. [`poll_fn`] then builds a future from a closure, which is where
//! pinning and `unsafe` first become unavoidable. [`wrapper`] covers wrapping an
//! existing future in a type of your own.

pub mod ready;
pub mod pending;
pub mod poll_fn;
pub mod wrapper;

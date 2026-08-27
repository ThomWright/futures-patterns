//! Building bigger futures out of smaller ones.
//!
//! Both patterns here own inner futures and have to project their own pinnedness
//! down to them, which is what `pin-project-lite` is for. [`map`] wraps a single
//! future and transforms its output; [`race`] drives two at once and returns
//! whichever finishes first, which means deciding a polling order and living with
//! the bias that creates.

pub mod map;
pub mod race;

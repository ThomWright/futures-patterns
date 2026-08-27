//! Deadlines, using tokio's timer.
//!
//! [`timeout`] is the [`composition::race`](crate::composition::race) pattern
//! applied to a real runtime service: race the operation against a `Sleep` and
//! report whichever wins. It is also where the gap between a teaching
//! implementation and a production one is widest, so its docs spell out what tokio
//! does that this does not.

pub mod timeout;

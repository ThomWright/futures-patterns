//! Where readiness comes from.
//!
//! Every other pattern in this crate either completes immediately, wakes itself, or
//! forwards a poll to a future underneath it. None of them show what makes async work
//! at all: a future that parks, and is woken later by *something else* -- another
//! thread, an I/O event, a timer firing.
//!
//! That is what a leaf future does, and [`shared_state`] builds one.
//!
//! # Why this matters beyond `Future`
//!
//! The waker discipline learned here is the same one `Stream::poll_next` and tower's
//! `Service::poll_ready` require. All three park a task and must arrange to be
//! revisited, and all three hang or lose wakeups when it is done wrong. It is worth
//! learning once, in the simplest setting available.

pub mod shared_state;

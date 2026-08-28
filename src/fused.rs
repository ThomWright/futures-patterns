//! Advertising a stronger contract than [`Future`] gives.
//!
//! [`Future::poll`]'s documentation says that calling `poll` after it has returned
//! `Ready` "may panic, block forever, or cause other kinds of problems; the `Future`
//! trait places no requirements on the effects of such a call".
//!
//! That is a floor, not a ceiling. A concrete type is free to document more than the
//! trait demands, and several in this crate do:
//! [`MaybeDone`](crate::state_machine::maybe_done) promises that a completed branch
//! absorbs further polls, and [`Fuse`](crate::composition::fuse) promises they return
//! `Pending`. Callers who know the concrete type can rely on those promises.
//!
//! [`FusedFuture`] is how a type advertises the same thing *generically*, to a caller
//! that only knows it has some future. It is the mechanism the `futures` crate uses,
//! reproduced here.
//!
//! # Who needs it
//!
//! Only code that is generic over the futures it drives. A `select!` loop polls
//! whatever branches it is handed, cannot know their post-completion behaviour, and so
//! must ask before polling. A [`join`](crate::composition::join) does not need it: it
//! polls `MaybeDone` specifically, and relies on that type's own promise instead.
//! Both `futures` and `tokio` implement join exactly that way.
//!
//! Follows the trait in `futures-core/src/future.rs`; see NOTICE.md.

use std::future::Future;

/// A future that knows when it should no longer be polled.
///
/// Implement this when the type can answer honestly, and prefer to be conservative:
/// `true` means "do not poll me", so it is safe to report it early and unhelpful to
/// report it late.
pub trait FusedFuture: Future {
    /// Returns `true` if the future should no longer be polled.
    ///
    /// This says nothing about *what* happens if it is polled anyway. That remains up
    /// to the implementation, and differs between them:
    /// [`Fuse`](crate::composition::fuse::Fuse) returns `Pending` while
    /// [`MaybeDone`](crate::state_machine::maybe_done::MaybeDone) panics once its
    /// output has been taken.
    fn is_terminated(&self) -> bool;
}

//! The trait a future implements to report that it has finished.
//!
//! One method, for callers that must decide whether polling is still allowed without
//! knowing the concrete type in front of them.
//!
//! Follows the trait in `futures-core/src/future.rs`; see NOTICE.md.

use std::future::Future;

/// A future that can say whether it has finished.
///
/// [`is_terminated`](Self::is_terminated) reports whether the future should still be
/// polled, so a caller that does not know the concrete type can avoid polling one that
/// has already completed.
///
/// # Why it is needed
///
/// [`Future::poll`]'s documentation says that calling `poll` after it has returned
/// `Ready` "may panic, block forever, or cause other kinds of problems; the `Future`
/// trait places no requirements on the effects of such a call".
///
/// That is a floor, not a ceiling. A concrete type may document more than the trait
/// demands, and several here do: [`MaybeDone`](crate::state_machine::maybe_done)
/// promises that a completed branch absorbs further polls, and
/// [`Fuse`](crate::composition::fuse) promises they return `Pending`. A caller that
/// knows the concrete type can rely on those directly. `FusedFuture` is how the same
/// thing is advertised to one that does not.
///
/// # Contract
///
/// `is_terminated` must return `true` once the future has completed. Callers rely on
/// that to avoid polling it again, which `Future` does not permit: `futures`' `select!`
/// requires this trait precisely to "prevent it from being polled after completion"
/// when selecting in a loop.
///
/// It may also return `true` earlier, for a future that can no longer make progress and
/// should be dropped rather than polled.
///
/// Nothing is promised about what happens if a terminated future is polled anyway. The
/// mechanism is avoidance, not tolerance, which is why implementors here differ: `Fuse`
/// returns `Pending` and `MaybeDone` panics once its output has been taken.
///
/// # Who needs it
///
/// Only code that is generic over the futures it drives. A `select!` loop polls
/// whatever branches it is handed, cannot know their post-completion behaviour, and so
/// must ask before polling. A [`join`](crate::composition::join) does not need it: it
/// polls `MaybeDone` specifically, and relies on that type's own promise instead. Both
/// `futures` and `tokio` implement join exactly that way.
///
/// # The name
///
/// It comes from `Iterator::fuse`, and promises more than this trait asks for. A fused
/// iterator keeps returning `None` once it has finished, and `Fuse` does latch like
/// that. `MaybeDone` latches in only one of its two terminal states, and still
/// satisfies the trait, which asks only whether the future has finished. `Done` and
/// `Gone` are both reachable only after it has.
pub trait FusedFuture: Future {
    /// Returns `true` if the future should no longer be polled.
    fn is_terminated(&self) -> bool;
}

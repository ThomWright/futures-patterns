# futures-patterns

> [!NOTE]
> This is very WIP and also very vibe-coded.

Async patterns built on `poll`, `Pin` and `Waker`, explained.

`async fn` is a higher level interface for writing async code. But some things cannot be written that way: a future you need to name, so it can sit in a struct field or have traits implemented on it; a leaf future woken by something outside the async world; a combinator driving several futures at once; or a type whose behaviour after completion is part of its contract.

Doing it by hand sometimes means satisfying compiler-enforced invariants which are difficult to meet, or upholding contracts the compiler does *not* check.

This library contains a set of examples to illustrate lower level async concepts, and show how to write code which satisfies both the compiler and those other invariants.

## Organisation

The examples are organised by complexity:

### Basic patterns

Start here to understand the fundamentals of the Future trait:

- [`basic::ready`] - A future that immediately returns a value.
- [`basic::pending`] - A future that never completes.
- [`basic::yield_now`] - A future that gives up the thread once.
- [`basic::wrapper`] - Wrap an existing future in a newtype.

These demonstrate the basic structure of futures and introduce concepts like polling, wakers, and pinning.

### State machine patterns

Learn how to build futures with internal state transitions:

- [`state_machine::maybe_done`] - Track whether a future has completed.
- [`state_machine::two_state`] - Simple countdown state machine.

State machines are fundamental to implementing complex async behaviour. These examples show how to use enums to represent different states and manage transitions during polling.

### Waking

Where readiness comes from in the first place:

- [`waking::shared_state`] - A future woken by another thread.

Every other example here completes immediately, wakes itself, or forwards a poll to a future underneath it. This one parks and is woken by something external, which is what leaf futures do and what the rest are ultimately built on.

### Composition patterns

Build futures that drive other futures:

- [`composition::map`] - Transform a future's output.
- [`composition::race`] - Return the first of two futures to complete.
- [`composition::join`] - Wait for two futures and collect both outputs.
- [`composition::try_join`] - The same, but stop at the first error.
- [`composition::fuse`] - Make polling after completion harmless.

Composition is key to building complex async operations from simple pieces. These examples introduce pin projection and coordinating multiple futures.

### Time-based patterns

Work with time and deadlines using tokio's timer infrastructure:

- [`time::timeout`] - Require a future to complete within a time limit.

This demonstrates integration with runtime services and practical patterns for real-world async code.

### Advanced

Deeper explorations into more advanced topics:

- [`advanced::pinning`] - Whether a wrapper's pin reaches the value inside it.
- [`advanced::poll_fn`] - Wrap a closure into a future.

### Testing

- [`testing`] - Poll futures by hand, and count wakes.

`.await` only reveals a future's final output. These helpers drive a future one poll at a time so tests can assert on the whole poll sequence -- how many polls it took, and whether the task was woken when it should have been. That is where the subtle bugs in a `Future` impl actually live.

## Learning path

Recommended order for learning:

1. [`basic::ready`] and [`basic::pending`] - always ready, and never ready.
2. [`basic::yield_now`] - pending once, and arranging its own wake.
3. [`basic::wrapper`] - wrapping another future, and pin projection.
4. [`state_machine::two_state`] - states written out by hand, and asking to be polled again.
5. [`waking::shared_state`] - a future that waits until another thread wakes it. Where readiness comes from.
6. [`state_machine::maybe_done`] - keeping a finished future's output while the others catch up.
7. [`composition::map`] - transforming another future's output.
8. [`composition::race`] - whichever of two finishes first, and why the polling order matters.
9. [`composition::join`] - waiting for both. Then [`composition::try_join`], where failing early means abandoning a branch.
10. [`composition::fuse`] - promising more than the `Future` contract requires, and [`fused`] for saying so.
11. [`time::timeout`] - racing against the runtime's timer. The first example that needs a runtime.
12. [`advanced::pinning`] - the choice each wrapper makes about the field it holds, and what it commits to.
13. [`advanced::poll_fn`] - a future from a closure. The first place pinning forces `unsafe`.

[`testing`] is useful throughout; reach for it as soon as you want to assert on something `.await` cannot show you.

## Documentation

Run `cargo doc --open` to view the full documentation, including the key concepts the examples share.

Run `cargo test` to check them. Most of the explanation lives in doc comments, so the doctests are a substantial part of the suite; the poll-level tests that `.await` cannot express sit beside the code they cover, in each module's `mod tests`.

## References

Worth reading alongside this. Where a module is derived from one of them rather than merely informed by it, [NOTICE.md](NOTICE.md) says so.

- [tokio](https://github.com/tokio-rs/tokio)
- [futures-rs](https://github.com/rust-lang/futures-rs)
- [tower](https://github.com/tower-rs/tower)
- [linkerd2-proxy](https://github.com/linkerd/linkerd2-proxy)
- The Rust async book

## License

MIT — see [LICENSE](./LICENSE).

Some modules are derived from tokio, futures-rs and the Rust standard library, all used here under MIT. Their notices, and which file each follows, are in [NOTICE.md](NOTICE.md).

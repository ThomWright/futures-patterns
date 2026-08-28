# futures-patterns

A collection of patterns for implementing `Futures` in Rust.

This crate provides educational implementations of common Future patterns, based on real-world examples from tokio and other production async libraries. Each pattern is documented with the concepts it depends on, the trade-offs it makes, and where it diverges from the implementation it is based on.

## Patterns implemented

### Basic patterns

- **Ready** - A future that immediately returns a value.
- **Pending** - A future that never completes.
- **PollFn** - Wrap a closure into a future.
- **Wrapper** - Wrap an existing future in a newtype to hide or rename it.

### State machine patterns

- **MaybeDone** - Track whether a future has completed.
- **TwoState** - Simple countdown state machine.

### Waking

- **shared_state** - A future woken by another thread.

Every other pattern here completes immediately, wakes itself, or forwards a poll to a future underneath it. This one parks and is woken by something external, which is what leaf futures do and what everything else is built on.

### Composition patterns

- **Map** - Transform a future's output.
- **Race** - Return the first of two futures to complete.
- **Join** - Wait for two futures and collect both outputs.
- **TryJoin** - The same, but stop at the first error.
- **Fuse** - Make polling after completion harmless.

### Time-based patterns

- **Timeout** - Require a future to complete within a time limit.

### Testing

- **testing** - Poll futures by hand, and count wakes.

`.await` only shows a future's final output. To test a `Future` implementation you need to see the poll sequence itself: how many polls it took, and whether the task was woken when it should have been. That is where the subtle bugs live.

## Learning path

Recommended order for understanding the patterns:

1. `basic::ready` and `basic::pending` - always ready, and never ready.
2. `basic::poll_fn` - a future from a closure. The first place pinning forces `unsafe`.
3. `basic::wrapper` - wrapping another future, and projecting pinnedness onto it.
4. `state_machine::two_state` - states written out by hand, and waking yourself.
5. `waking::shared_state` - where readiness comes from: parked until another thread wakes you.
6. `state_machine::maybe_done` - parking a finished future's output.
7. `composition::map` - transforming an output. Why the function lives in an `Option`.
8. `composition::race` - first of two wins, and the bias that creates.
9. `composition::join` and `composition::try_join` - waiting for both. Failing early means abandoning a branch.
10. `composition::fuse` and `fused` - promising more than the `Future` contract requires.
11. `time::timeout` - racing a runtime timer, and where this stops matching tokio.

`testing` is useful throughout; reach for it as soon as you want to assert on something `.await` cannot show you.

## Documentation

Run `cargo doc --open` to view the full documentation with detailed explanations of each pattern.

Run `cargo test` to check them. Most of the explanation lives in doc comments, so the doctests are a substantial part of the suite; `tests/` holds the poll-level tests that `.await` cannot express.

## References

Based on implementations from:

- [tokio](https://github.com/tokio-rs/tokio)
- [tower](https://github.com/tower-rs/tower)
- [linkerd-proxy](https://github.com/linkerd/linkerd2-proxy)

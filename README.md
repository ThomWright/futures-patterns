# futures-patterns

A collection of patterns for implementing `Futures` in Rust.

This crate provides educational implementations of common Future patterns, based on real-world examples from tokio and other production async libraries. Each pattern is documented with the concepts it depends on, the trade-offs it makes, and where it diverges from the implementation it is based on.

## Patterns Implemented

### Basic Patterns

- **Ready** - A future that immediately returns a value
- **Pending** - A future that never completes
- **PollFn** - Wrap a closure into a future
- **Wrapper** - Wrap an existing future in a newtype to hide or rename it

### State Machine Patterns

- **MaybeDone** - Track whether a future has completed
- **TwoState** - Simple countdown state machine

### Waking

- **shared_state** - A future woken by another thread

Every other pattern here completes immediately, wakes itself, or forwards a poll to a future underneath it. This one parks and is woken by something external, which is what leaf futures do and what everything else is built on.

### Composition Patterns

- **Map** - Transform a future's output
- **Race** - Return the first of two futures to complete
- **Join** - Wait for two futures and collect both outputs
- **TryJoin** - The same, but stop at the first error
- **Fuse** - Make polling after completion harmless

### Time-Based Patterns

- **Timeout** - Require a future to complete within a time limit

### Testing

- **testing** - Poll futures by hand, and count wakes

`.await` only shows a future's final output. To test a `Future` implementation you need to see the poll sequence itself: how many polls it took, and whether the task was woken when it should have been. That is where the subtle bugs live.

## Learning Path

Recommended order for understanding the patterns:

1. `basic::ready` and `basic::pending` - the two degenerate futures, always ready and never ready, which between them show what `poll` has to decide
2. `basic::poll_fn` - building a future from a closure, and the first point where pinning forces `unsafe`
3. `basic::wrapper` - wrapping someone else's future, and projecting your own pinnedness onto it
4. `state_machine::two_state` - writing the states out by hand, and waking yourself
5. `waking::shared_state` - where readiness comes from: a future parked until another thread wakes it
6. `state_machine::maybe_done` - parking a finished future's output, so that several futures can be driven at once
7. `composition::map` - transforming an output, and why the mapping function has to live in an `Option`
8. `composition::race` - driving two futures and taking the first, and the bias any polling order creates
9. `composition::join` and `composition::try_join` - driving two and waiting for both; failing early means abandoning a branch
10. `composition::fuse` and `fused` - how a type can promise more than the `Future` contract requires
11. `time::timeout` - racing against a runtime timer, and where a teaching implementation stops matching tokio

`testing` is useful throughout; reach for it as soon as you want to assert on something `.await` cannot show you.

## Documentation

Run `cargo doc --open` to view the full documentation with detailed explanations of each pattern.

Run `cargo test` to check them. Most of the explanation lives in doc comments, so the doctests are a substantial part of the suite; `tests/` holds the poll-level tests that `.await` cannot express.

## References

Based on implementations from:

- [tokio](https://github.com/tokio-rs/tokio)
- [tower](https://github.com/tower-rs/tower)
- [linkerd-proxy](https://github.com/linkerd/linkerd2-proxy)

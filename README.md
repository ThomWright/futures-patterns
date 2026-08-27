# futures-patterns

A collection of patterns for implementing `Futures` in Rust.

This crate provides educational implementations of common Future patterns, based on real-world examples from tokio and other production async libraries. Each pattern includes comprehensive documentation explaining the concepts, trade-offs, and implementation details.

## Patterns Implemented

### Basic Patterns

- **Ready** - A future that immediately returns a value
- **Pending** - A future that never completes
- **PollFn** - Wrap a closure into a future
- **Wrapper** - Wrap an existing future in a newtype to hide or rename it

### State Machine Patterns

- **MaybeDone** - Track whether a future has completed
- **TwoState** - Simple countdown state machine

### Composition Patterns

- **Map** - Transform a future's output
- **Race** - Return the first of two futures to complete

### Time-Based Patterns

- **Timeout** - Require a future to complete within a time limit

### Testing

- **testing** - Poll futures by hand, and count wakes

`.await` only shows a future's final output. To test a `Future` implementation you need to see the poll sequence itself: how many polls it took, and whether the task was woken when it should have been. That is where the subtle bugs live.

## Learning Path

Recommended order for understanding the patterns:

1. Start with `basic::ready` and `basic::pending` to understand the Future trait
2. Study `basic::poll_fn` to learn about pinning and unsafe usage
3. Read `basic::wrapper` for the pinning rules around wrapping a future
4. Move to `state_machine::two_state` for a simple state machine
5. Examine `state_machine::maybe_done` for a production-like pattern
6. Learn composition with `composition::map`
7. Study coordination with `composition::race`
8. Finish with `time::timeout` to see everything combined

`testing` is useful throughout; reach for it as soon as you want to assert on something `.await` cannot show you.

## Documentation

Run `cargo doc --open` to view the full documentation with detailed explanations of each pattern.

Run `cargo test` to check them. Most of the explanation lives in doc comments, so the doctests are a substantial part of the suite; `tests/` holds the poll-level tests that `.await` cannot express.

## References

Based on implementations from:

- [tokio](https://github.com/tokio-rs/tokio)
- [tower](https://github.com/tower-rs/tower)
- [linkerd-proxy](https://github.com/linkerd/linkerd2-proxy)

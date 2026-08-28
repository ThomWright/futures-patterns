# futures-patterns

Async patterns built on `poll`, `Pin` and `Waker`, explained.

An `async fn` hides the state machine the compiler generates for it. Writing `poll` directly puts `Pin`, `Waker` and the poll contract back in view, which is the point: every pattern here exists to explain one of them.

Some follow a production implementation closely and say which. Others are invented to isolate a single idea. Either way each is documented with the concepts it depends on, the trade-offs it makes, and what it simplifies.

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
3. `basic::wrapper` - wrapping another future, and pin projection.
4. `state_machine::two_state` - states written out by hand, and asking to be polled again.
5. `waking::shared_state` - a future that waits until another thread wakes it. Where readiness comes from.
6. `state_machine::maybe_done` - keeping a finished future's output while the others catch up.
7. `composition::map` - transforming another future's output.
8. `composition::race` - whichever of two finishes first, and why the polling order matters.
9. `composition::join` - waiting for both. Then `composition::try_join`, where failing early means abandoning a branch.
10. `composition::fuse` - promising more than the `Future` contract requires, and `fused` for saying so.
11. `time::timeout` - racing against the runtime's timer. The first pattern that needs a runtime.

`testing` is useful throughout; reach for it as soon as you want to assert on something `.await` cannot show you.

## Documentation

Run `cargo doc --open` to view the full documentation with detailed explanations of each pattern.

Run `cargo test` to check them. Most of the explanation lives in doc comments, so the doctests are a substantial part of the suite; `tests/` holds the poll-level tests that `.await` cannot express.

## References

Worth reading alongside this. Where a module is derived from one of them rather than merely informed by it, [NOTICE.md](NOTICE.md) says so.

- [tokio](https://github.com/tokio-rs/tokio)
- [futures-rs](https://github.com/rust-lang/futures-rs)
- [tower](https://github.com/tower-rs/tower)
- [linkerd2-proxy](https://github.com/linkerd/linkerd2-proxy)

## License

MIT — see [LICENSE](LICENSE).

Some modules are derived from tokio, futures-rs and the Rust standard library, all used here under MIT. Their notices, and which file each follows, are in [NOTICE.md](NOTICE.md).

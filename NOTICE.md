# Third-party notices

This crate reimplements patterns from other projects in order to explain them. Some modules follow their originals closely enough to be derived work rather than merely inspired by them, and are listed below with the file they follow.

Everything listed is available under the MIT licence, which is reproduced once at the end because the text is identical for all of them. Where a project is dual-licensed, it is used here under MIT.

## tokio

Copyright (c) 2023 Tokio Contributors. <https://github.com/tokio-rs/tokio>

- `advanced::poll_fn` follows `tokio/src/future/poll_fn.rs`, including its use of `Pin::into_inner_unchecked` and the reasoning for leaving `PollFn` conditionally `Unpin`.
- `time::timeout` follows `tokio/src/time/timeout.rs`, and `Elapsed`'s message is taken from `tokio/src/time/error.rs`.

## futures-rs

Copyright (c) 2016 Alex Crichton. Copyright (c) 2017 The Tokio Authors. <https://github.com/rust-lang/futures-rs>

- `state_machine::maybe_done` follows `futures-util/src/future/maybe_done.rs`, including its three states and its `FusedFuture` implementation.
- `composition::fuse` follows `futures-util/src/future/future/fuse.rs`.
- `composition::join` follows the polling and collection shape of `futures-util/src/future/join.rs`.
- `fused::FusedFuture` follows the trait in `futures-core/src/future.rs`.

## The Rust standard library

Copyright (c) The Rust Project Developers. <https://github.com/rust-lang/rust>

- `basic::ready` follows `core::future::Ready`, which uses the same `Option<T>` and panics with the same message when polled after completion.
- `basic::pending` follows `core::future::Pending`, including `PhantomData<fn() -> T>` so that the auto traits do not follow `T`.

## MIT licence

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

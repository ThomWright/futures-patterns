# futures-patterns

## What this crate is for

Learning the parts of Rust async that are hard, by implementing them: writing `poll` rather than an `async fn`, so that `Pin`, `Waker` and the poll contract stay visible. It is not a library anyone depends on, so **educational clarity beats every other consideration** — including neat testability. If a clearer pattern is harder to test, take the clearer pattern and say what the tests do not cover.

Scope is async traits generally, not only `Future`: `Stream::poll_next` and tower's `Service::poll_ready` belong here too.

A gitignored `reference/` directory may exist locally, holding source worth reading. Look at what is actually in it rather than assuming. It is not part of this repo, so never cite it, link into it, or assume a reader has it.

## Prose

Applies to doc comments, README and commit messages alike.

### Say the specific thing

- **Be terse in proportion to the content.** If a thing is simple, describe it in a phrase. "Always ready, and never ready" beat a sentence and a half explaining what that shows.
- **Never use a word that stands in for the specifics.** Ones that have needed removing from this repo: fundamental, comprehensive, powerful, production-like, crucial, robust, properly. Say what is actually true or cut the sentence.
- **Describe the subject, not the document.** No "this is a crucial learning point", "here's why", "it is worth being explicit about", "as we will see". If a point needs making, make it.
- **Do not explain the joke.** State the thing and trust the reader.

### Introduce before using

- No term before the thing that teaches it. Naming a concept the reader is about to learn is fine; relying on one they have not met is not.
- No definite article for something unintroduced. "Why the function lives in an `Option`" assumes a field the reader has never seen.
- When a sentence names two things, attribute what follows to whichever it belongs to. Only `try_join` can fail early; only `fused` is the trait.

### Lists

- Every item must earn its place. Four bullets saying "hide the concrete type" four ways is one bullet.
- Punctuate consistently *within* a list, and let the items decide. The test is whether an item continues the lead-in: "the patterns show how to: / Use enums…" continues it and takes no full stop, while "[`basic::ready`] - A future that immediately returns a value." is a labelled entry and does.

### Mechanics

- British spelling, unless quoting something that uses US spelling.
- Sentence case for headings.
- Never hard-wrap Markdown. `markdownlint` is configured with MD013 off so it agrees.

## Documenting a module

Every module, not only the pattern ones, should answer these in roughly this order. Lead with what the thing *is*; the teaching comes after it, not instead of it.

- What it does, in a sentence.
- Why it is shaped the way it is — the failure that each design decision prevents. This is the part worth the words.
- What it simplifies compared with the real implementation, and what that costs. Never quietly simplify.
- Where the original lives, named so that anyone can find it: a crate plus a path inside *that* crate, such as `tokio/src/time/timeout.rs`, or a type path such as `futures_util::future::Fuse`.

Ground claims about production code in the source rather than from memory. Quote it when the wording matters; several confident paraphrases have turned out to be wrong.

## Attribution

Everything here is a reimplementation, and some modules follow their original closely enough to be derived work. Record those in `NOTICE.md` with the upstream copyright, the licence text, and the file each one follows, and put a one-line note at the end of the module's own docs.

Decide which by comparing against the source rather than from memory. `composition::map` reads like it must be derived from futures-rs and is not — theirs is an enum using `project_replace`, ours a struct holding an `Option<F>` — while `basic::ready` looked incidental and turned out to share std's design and its panic message.

## Tests

- Unit tests live in-module, in `#[cfg(test)] mod tests`. There is deliberately no `tests/` directory.
- Test at the poll level using `crate::testing`, not through `.await` alone. `.await` shows a future's final output and hides everything interesting: how many polls it took, and whether the task was woken when it should have been.
- Prefer a test that makes a contract observable over one that restates the output. `ready` panicking when polled twice is what proves `MaybeDone` absorbs polls rather than forwarding them; a counting waker is what proves a finished `Fuse` registers nobody.
- Timer tests use `#[tokio::test(start_paused = true)]`, so they exercise real deadlines without sleeping.

## Before committing

All four must be clean:

```sh
cargo test
cargo clippy --all-targets
cargo doc --no-deps
markdownlint *.md
```

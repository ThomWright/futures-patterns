# Pedagogy

How this crate teaches, and what a module or comment should be checked against. Complements the "Documenting a module" and "Prose" sections in `CLAUDE.md`.

## Sequencing

- Introduce-before-use applies to explanations, not just terms. A comment or doc that answers "why" using a concept the reader has not met yet is not an explanation — it is an assertion borrowing an explanation's shape. Check a comment against what the learning path has covered *by that point*, not against what the writer knows.
- Prove the local claim; teach the general mechanism elsewhere. A specific instance ("this type is `Unpin`") and the rule behind it ("why blanket `Unpin` impls are sound") are different jobs. Give the general one its own module, positioned where the reader is ready for it; let the local site point at it rather than absorb it.
- Sequencing defers danger, not only difficulty. Where a hard concept could appear early or late, prefer the ordering that keeps `unsafe` — or anything else costly to reach for — as late as the concept graph allows, even if that means teaching a concept in two passes: an early pass showing its effect, a later module teaching its mechanism.
- Motivate with a demonstration, not a claim. Introduce a concept by first showing the failure it prevents — a test that would panic or misbehave without it — before the device that prevents it. A design decision's rationale (already required by "Documenting a module" in `CLAUDE.md`) is stronger shown than asserted.
- Patterns suit early material; mechanism studies suit later. Early modules should mostly be reusable shapes that motivate why the reader is here at all; save the deepest internals-only exhibits for once the path has earned the reader's patience.

## Explaining what can't be explained yet

When the mechanism behind something is out of reach at that point in the path, there is no safe middle ground between showing and pointing — gesturing at a benefit without the contrast that makes it real teaches nothing. Pick one:

- Show a concrete before/after or with/without contrast, using only vocabulary already introduced.
- Point past it — "why this is safe: [`pinning`]" — and stop there.

Never write the sentence that sounds like an explanation but is not one.

## Pointers

- Forward pointers are legitimate, not a defect, wherever the knowledge is graph-shaped rather than linear. Do not force a false linear order to avoid one.
- Distinguish a pointer the reader must follow to trust the current claim from one that is a deeper detour for later. Conflating them makes the reader either chase everything or skip what they shouldn't.
- Keep every forward pointer enumerated in the crate-level learning path, so none go stale or dangle.

## Consistency

- Fix terminology at first use. Once a term is defined, later mentions link back to it rather than restate it in different words — restatement is where drift creeps in.
- Teach contrasting patterns side by side, not in isolated modules far apart. Two things that look similar and diverge in one instructive way (`MaybeDone` vs `Fuse`, say) sharpen each other; taught alone, neither lands as clearly.

## Density

Fade the scaffolding as the path progresses. Early modules can be fully worked, nothing left unexplained. Later ones can assume more of what has already been built up and explain less. Make that gradient deliberate rather than accidental.

## Reviewing against this file

For a module at position N in the learning path:

1. Build the set of concepts taught by modules 1..N-1.
2. Check every comment and doc comment in module N: does it lean on anything outside that set? Does it restate the code instead of arguing for a choice? Does it assert something the reader cannot yet verify, where an example would let them?
3. Check the crate-level learning path: is every forward pointer this module makes still enumerated there, marked required or optional?
4. Can this module's role — pattern, mechanism study, or a mix — be answered, even where it's never stated?

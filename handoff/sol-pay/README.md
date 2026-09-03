# Handoff to sol-pay

Findings that surfaced while specifying the demonstrator but that belong to the
library, not to the demo. They are written to stand alone — a sol-pay reader
needs no part of the demonstrator's spec to follow them.

This directory is a staging area, not a home. Each file is meant to be moved
into `wbreeze/sol-pay` — as a note in the repository, or opened as an issue and
then deleted from here. Nothing in this directory should be referenced from the
demonstrator's own `SPEC.md`.

| file | one line |
| --- | --- |
| `01-server-side-reach.md` | A Rust-only server half reaches ~1% of candidate integrators. What to do about it. |
| `02-multi-site-and-the-single-delegate.md` | "Can a reader only be metered by one site at a time?" — answerable, and the answer is not in the docs. |
| `03-the-revenue-claim.md` | Does a penny a view beat advertising? Not at a penny. What does hold, and what the price would have to be. |

Surfacing exactly this kind of thing is what building a demonstrator was for.
Both are consequences of the design meeting a real integration rather than
defects in it.

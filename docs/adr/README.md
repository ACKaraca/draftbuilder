# DraftBuilder architecture decision records

This folder records decisions specific to DraftBuilder. Suite-wide decisions live in
[`arc-suite`](https://github.com/ACKaraca/arc-suite/tree/main/docs/adr) and are not repeated here.

## Index

| ADR | Title | Status |
|---|---|---|
| [0001](0001-drawing-database-is-the-model.md) | The DWG/DXF drawing database is the in-memory model | accepted |
| [0002](0002-stable-ids-from-fingerprint-and-handle.md) | Stable object identifiers derived from the drawing fingerprint and handle | accepted |
| [0003](0003-lossless-preservation.md) | Lossless preservation of unknown objects and data | accepted |
| [0004](0004-pure-rust-dwg-from-public-specification.md) | DWG reading and writing in pure Rust from the public specification | accepted |
| [0005](0005-lisp-interpreter-out-of-process.md) | The Lisp-dialect interpreter runs out of process as an automation client | accepted |
| [0006](0006-toolset-objects-degrade-to-entities.md) | Toolset objects are typed objects that degrade to plain entities | accepted |

## Template

Copy this into `NNNN-<slug>.md` with the next free number. Numbers are never reused; a superseded
record keeps its file and links to its successor.

```markdown
# ADR-NNNN: <decision as a short statement>

<One or two sentences stating the decision.>

- **Status:** proposed | accepted | superseded by ADR-NNNN
- **Suite context:** links to related suite decisions

## Context

<The forces and constraints that make a decision necessary.>

## Decision

<What is decided, stated precisely.>

## Consequences

<What becomes easier, harder or required.>

## Alternatives considered

<Each alternative and why it was not chosen.>

## Reopen when

<The observable trigger that would justify revisiting the decision.>
```

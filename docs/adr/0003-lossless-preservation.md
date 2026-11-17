# ADR-0003: Lossless preservation of unknown objects and data

DraftBuilder keeps every object, class, field and extended datum it does not model and writes it back
unchanged, with explicit rules for when edits invalidate it.

- **Status:** accepted
- **Suite context:** [suite decision records](https://github.com/ACKaraca/arc-suite/tree/main/docs/adr)

## Context

Drawings carry custom objects from discipline applications and add-ins, extended data from
corporate tools, and fields from format versions newer than a given mapping. Dropping or rewriting
any of it damages a partner's data without their knowledge. Keeping raw data is easy for untouched
objects and hard when the user edits the owner or related objects.

## Decision

- Unknown classes, objects and entities are preserved with class record, raw data, owner handle,
  references and proxy graphics. Unmapped fields of known objects are preserved beside the object.
- Extended data, xrecords and extension dictionaries are always preserved verbatim; handle-typed
  values are remapped on copy.
- Unknown entities can be moved, copied, rotated, scaled and mirrored: the transform is applied to
  the proxy graphics and to the transform fields the class record exposes, and the edit is recorded
  in the loss report as a transform of preserved data.
- Any other edit to an unknown entity is refused for that entity with an explanation. Erasing it is
  allowed and undoable.
- When a known object is edited in an aspect covered by preserved unmapped fields, those fields are
  dropped for that object and reported; untouched aspects keep them.
- Every open and save produces a preservation report; the CLI returns it as JSON.

## Consequences

- Round trips through DraftBuilder do not degrade partners' data.
- The model carries raw data for many objects; memory is bounded by sharing raw blocks with the file
  buffer where possible.
- Some edits are refused; users see why and which application owns the object.

## Alternatives considered

- **Explode unknown objects to their proxy graphics.** Simple, but destroys data.
- **Keep unknown objects read-only and immovable.** Safe, but blocks routine edits such as moving a
  whole plan.

## Reopen when

Corpus validation shows a class of custom objects whose transform cannot be applied safely through
proxy graphics and exposed fields, or users need richer editing of a widely used foreign class that
a WebAssembly plug-in could provide.

# ADR-0001: The DWG/DXF drawing database is the in-memory model

DraftBuilder keeps drawings in a model aligned object for object with the DWG/DXF data model and
saves `.dwg` and `.dxf` files directly.

- **Status:** accepted
- **Suite context:** [suite decision records](https://github.com/ACKaraca/arc-suite/tree/main/docs/adr)

## Context

Other Arc products store a native document as one SQLite package. DraftBuilder's users exchange
DWG and DXF files every day, keep them in their own file systems and document management, and expect
the file they open to be the file they save. A separate native format would put a translation step
between users and their deliverables and would make every save a potential loss.

## Decision

- The in-memory model of `draftbuilder-model` is the DWG/DXF drawing database: header, classes,
  symbol tables, block records, entities, dictionaries, handles, extended data and preserved foreign
  objects, built on the shared drafting types of `arc-draft` and stored in the `arc-doc` object store
  with transactions and the operation journal.
- `.dwg` and `.dxf` are the native files. Data the format cannot carry (journal history across
  sessions, recovery journal, per-drawing DraftBuilder settings, Hub metadata, toolset project data)
  is kept in a `.draftb` sidecar package beside the drawing. The drawing opens fully without it.
- Drawing-database semantics (handles, owner tree, dictionaries, extended data) live in
  DraftBuilder, not in `arc-draft`, so other products use the drafting model without DWG specifics.

## Consequences

- Reading and writing are mappings with no approximation on the native path.
- Features that need persistent data the format does not define use either extension dictionaries
  and xrecords in a documented layout or the sidecar package; each such layout is documented.
- Losing the sidecar loses history, never drawing content.
- `arc-draft` must stay expressive enough to represent every drafting entity of the format; gaps are
  filled in `arc-draft` when a second product needs them, otherwise in `draftbuilder-model`.

## Alternatives considered

- **A native SQLite document with DWG import and export.** Consistent with other products, but turns
  every save into an export and every open into an import with loss.
- **DXF as the only native format.** Simpler to implement, but DWG is what users exchange.
- **Putting DWG semantics into `arc-draft`.** Would burden every product with handles, dictionaries
  and proxies.

## Reopen when

A capability requires persistent data that can be stored neither in documented extension dictionary
layouts nor in the sidecar package without degrading the drawing for other applications.

# DWG and DXF compatibility

This document describes how the in-memory drawing database aligns with the DWG/DXF data model, how
unknown content is preserved, which versions are targeted, how DWG reading and writing are
implemented and validated, and how fidelity is stated. Every statement of fidelity here is a target;
achieved fidelity is published per release from the validation results described below.

## Alignment of the model with the format

The drawing database is the DWG/DXF data model expressed as typed Rust objects
([ADR-0001](../adr/0001-drawing-database-is-the-model.md)):

| Format concept | Database concept |
|---|---|
| Header variables | Header settings, stored under their format names and exposed through DraftBuilder settings |
| Class section | Class records, including classes DraftBuilder does not model |
| Symbol tables and records | Symbol tables and records with every field of the format |
| Blocks section, block records, model and paper space | Block records owning entities; model space and paper space records |
| Entities | Entity types with every field, including those DraftBuilder does not use itself |
| Objects section and named object dictionary | Dictionaries, xrecords, layouts, groups, styles and other named objects |
| Handles and handle references | Handles with typed references (hard owner, soft owner, hard pointer, soft pointer) |
| Extended data and extension dictionaries | Extended data and extension dictionaries unchanged |
| Proxy entities and objects | Preserved objects with their raw data and proxy graphics |
| Object coordinate system | Stored exactly as written; world coordinates are computed, never stored back |

Because the model and the format are aligned, reading and writing are mappings: no object is
reconstructed from approximations, and saving an unmodified drawing reproduces the same objects,
handles and values.

## Preservation of unknown content

[ADR-0003](../adr/0003-lossless-preservation.md) defines the rules; in summary:

- **Unknown classes and objects** are kept with their class record, raw data, owner and references,
  and written back unchanged. Unknown entities display, plot and snap through their proxy graphics
  ([DB-DBASE-017](../product/capabilities/01-drawing-database.md),
  [DB-DBASE-018](../product/capabilities/01-drawing-database.md)).
- **Unmapped fields of known objects** (for example fields added in a format version newer than the
  mapping) are kept beside the object and written back while the object is unchanged in those
  aspects.
- **Extended data, xrecords and extension dictionaries** are kept verbatim; handle-typed values are
  remapped when objects are copied ([DB-DBASE-020](../product/capabilities/01-drawing-database.md)).
- **Transforms** of unknown entities (move, copy, rotate, scale, mirror) transform their proxy
  graphics and are recorded; edits that would make raw data inconsistent are refused for that object
  and reported.
- **Deletion** of an owner removes the preserved content it owns, recorded in the journal so undo
  restores it.
- Every open and save produces a preservation report
  ([DB-DBASE-019](../product/capabilities/01-drawing-database.md)).

## Version targets

DWG versions are grouped into format families that share one file structure.

| Family | Read target | Write target | Phase |
|---|---|---|---|
| 2018 family (current) | edit | edit, default for new drawings | 1 read, 2 write |
| 2013 family | edit | edit (save-down) | 1 read, 2 write |
| 2010 family | edit | edit (save-down) | 1 read, 2 write |
| 2007 family | edit | edit (save-down) | 2 |
| 2004 family | edit | edit (save-down) | 2 |
| 2000 family | edit | edit (save-down) | 2 |
| R14, R13 | edit | — | 2 |
| R12 and earlier | view through DXF only | — | open question ([OQ-01](../product/open-questions.md)) |
| A newer family than supported | refused without touching the file, with a clear message | — | 1 |

| DXF | Read | Write |
|---|---|---|
| ASCII and binary, R12 through 2018 family | edit | edit |

A save-down to an older family converts objects that the target cannot represent and lists every
conversion in the loss report before writing; the open drawing keeps its full content
([DB-DBASE-029](../product/capabilities/01-drawing-database.md)).

## Implementation

### Pure-Rust reader and writer

- DWG reading and writing live in `arc-io-dwg`, a pure-Rust crate implemented from the publicly
  available DWG format specification; DXF lives in `arc-io-dxf`, implemented from the published DXF
  reference ([ADR-0004](../adr/0004-pure-rust-dwg-from-public-specification.md)).
- The crates produce and consume a neutral model of each format: sections, class records, objects
  with typed fields and raw data for unrecognized content. `draftbuilder-io` maps that model to the
  drawing database.
- The bit-level layer (bit-coded values, compressed sections, checksums, page maps, encryption of
  section headers as the specification describes) is isolated in one module with fuzz targets.
- Readers bound every count, length, recursion depth and allocation, and fail with a typed error
  rather than a panic on malformed input.
- Whether an existing permissively licensed pure-Rust DWG library is adopted as the starting point
  is decided under [OQ-02](../product/open-questions.md), subject to a provenance review that
  confirms it was written from the public specification.

### Third-party readers behind a byte-level bridge

A third-party DWG library whose license is outside the suite allowlist, or whose provenance cannot be
confirmed, is never linked. If one is used at all, it runs as a separate process that receives file
bytes and returns file bytes or a neutral document, and it is never required for a core workflow
([DB-INTEROP-006](../product/capabilities/19-interoperability-standards.md)). Its primary use is as a
validation oracle in CI.

### Fonts, proxy graphics and derived blocks

- Shape fonts are rendered by DraftBuilder; an openly licensed replacement set covers common font
  names ([DB-INTEROP-021](../product/capabilities/19-interoperability-standards.md)).
- On save, DraftBuilder writes the derived data other applications expect: dimension blocks,
  anonymous dynamic-block representations, proxy graphics for its own custom objects, and plain
  entity fallbacks for toolset objects.
- 3D solid and surface data carried inside DWG objects is preserved byte for byte; editing it depends
  on the outcome of [OQ-07](../product/open-questions.md).

## Validation with independent readers

| Check | Method |
|---|---|
| Round trip | Each corpus file is opened and saved unmodified; the output must contain the same objects, handles, field values and preserved content |
| Edit round trip | Scripted edits are applied, saved and reread; changed objects match expected state, unchanged objects are byte-equivalent at the object level |
| Independent read-back | Every written file is opened by at least one independent reader that shares no code with DraftBuilder, run out of process in CI; object counts, symbol tables and geometry are compared |
| Save-down | Each supported older family is written and read back by DraftBuilder and an independent reader; the loss report must list every conversion observed |
| Hostile input | The reader is fuzzed continuously; corrupted corpus files must open through recover or fail with a typed error |
| Cross-platform | All checks run on Windows, macOS and Linux |

Corpus files come from the suite corpus repositories, each with an owner, license and checksum. No
vendor sample files are used.

## Fidelity statements

Release notes state fidelity per format family and object class as one of: **preserved** (written
back unchanged), **edited** (modelled and written correctly after edits), **converted** (written in
a simpler form, listed in the loss report) or **not yet validated**. A class moves to a higher level
only when its validation checks pass. DraftBuilder does not describe its DWG support as complete or
as matching any other application; it publishes what its validation suite has shown.

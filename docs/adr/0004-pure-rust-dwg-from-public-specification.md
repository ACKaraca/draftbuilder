# ADR-0004: DWG reading and writing in pure Rust from the public specification

DWG support is implemented in the pure-Rust crate `arc-io-dwg` from the publicly available format
specification; third-party readers are used only out of process, mainly as validation oracles.

- **Status:** accepted
- **Suite context:** [suite decision records](https://github.com/ACKaraca/arc-suite/tree/main/docs/adr)

## Context

DWG is the product's native format, so its reader and writer are on every critical path. The suite
interoperability policy allows only published specifications and allowlisted licenses, and forbids
linking libraries under restrictive or uncertain licenses. Available DWG libraries are either under
licenses outside the allowlist, offered under membership terms, or young projects whose provenance
needs review.

## Decision

- `arc-io-dwg` implements DWG reading and writing in safe Rust from the publicly available DWG
  specification; `arc-io-dxf` implements DXF from the published DXF reference.
- An existing permissively licensed pure-Rust DWG library may be adopted or contributed to only after
  a provenance review confirms clean-room origin and its license is on the allowlist
  ([OQ-02](../product/open-questions.md)).
- Libraries outside the allowlist or with unconfirmed provenance may run only as a separate process
  behind a byte-level bridge, never linked and never required for a core workflow; their main use is
  as independent readers in CI.
- DWG fidelity is stated as validated targets per version family; see
  [dwg-compatibility.md](../architecture/dwg-compatibility.md).

## Consequences

- The project owns the hardest code path and must staff it, fuzz it and keep a large corpus.
- No license or membership constraint is imposed on users or redistributors.
- Format families newer than the public specification are supported only after the specification
  covers them or they are described by original analysis within the clean-room policy.

## Alternatives considered

- **Linking a membership-licensed SDK.** Mature, but its terms conflict with the suite license policy
  and redistribution freedom.
- **Linking a copyleft DWG library.** Incompatible with the Apache-2.0 distribution of the product.
- **DXF only, with DWG through an external converter.** Leaves the native format to a component the
  project cannot ship.

## Reopen when

The public specification stops covering a format family that users need and no clean-room path is
available, or a permissively licensed implementation with confirmed provenance clearly exceeds
`arc-io-dwg` on the validation corpus.

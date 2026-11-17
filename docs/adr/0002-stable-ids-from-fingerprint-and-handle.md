# ADR-0002: Stable object identifiers derived from the drawing fingerprint and handle

Every object's 128-bit stable identifier is computed from the drawing's fingerprint GUID and the
object's handle, so identity needs no stored mapping.

- **Status:** accepted
- **Suite context:** [suite decision records](https://github.com/ACKaraca/arc-suite/tree/main/docs/adr)

## Context

The suite addresses objects by stable 128-bit identifiers for the journal, history, change sets,
links and Hub issues. DWG objects carry handles that are unique within a drawing and persistent
across saves, and each drawing carries a fingerprint GUID that is kept when the file is saved again
and a version GUID that changes on every save. Storing a separate identifier per object would need a
place in the file for every object, or a mapping in the sidecar that can be lost or desynchronized.

## Decision

- Stable ID = name-based UUID (version 5) over a DraftBuilder namespace, the drawing fingerprint GUID
  and the handle value.
- Handles are allocated from the handle seed and never reused, including after undo, purge and
  recovery.
- Copying objects into a drawing allocates new handles, and therefore new stable IDs, and remaps
  internal references.
- "Save as independent copy" generates a new fingerprint; a plain save as keeps it.
- Drawings with a missing or null fingerprint receive one on first save; the change is listed in the
  save report.

## Consequences

- Identity survives editing in other applications that keep handles and the fingerprint, as the
  format expects of them.
- Two copies of one drawing edited in parallel share identities, which branch-and-merge and compare
  need.
- An application that renumbers handles or regenerates the fingerprint breaks identity; compare then
  falls back to geometric matching and says so.

## Alternatives considered

- **Random IDs stored in extended data.** Adds data to every object and depends on other applications
  preserving it.
- **Random IDs mapped in the sidecar.** The mapping is lost when the sidecar is not transferred.
- **Handles alone.** Handles are not unique across drawings, which Hub, links and references require.

## Reopen when

A significant share of corpus drawings arrive with renumbered handles or regenerated fingerprints
after round trips through other applications.

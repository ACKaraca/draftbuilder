# ADR-0008: Windows object linking and the automation bridge live in one Windows-only FFI crate

Embedded-object support and the COM automation bridge use the operating system's component
interfaces through a single FFI crate, `crates/draftbuilder-windows`, compiled only on Windows.

- **Status:** accepted
- **Suite context:** [suite decision records](https://github.com/ACKaraca/arc-suite/tree/main/docs/adr)

## Context

Windows offers object linking and embedding, which lets a drawing hold a document of another
application and edit it in place, and a component automation interface through which existing
scripts drive an application. Both need unsafe calls into system libraries, which the suite rules
allow only in FFI crates named in a decision record. Neither mechanism exists on macOS or Linux, and
neither may weaken the command model: the bridge must not become a second write path.

## Decision

- `crates/draftbuilder-windows` is the only crate that contains unsafe code for these features. It
  holds the object-linking container (site, in-place activation, cached image capture) and the
  automation server (class registration, type information, dispatch).
- The container is used by the desktop application on Windows only. A foreign document is activated
  only after the user's confirmation and never in headless runs ([DB-XREF-031 to 033](../product/capabilities/12-references-underlays.md)).
- The automation server runs in `apps/draftbuilder-com`, a separate process that is an
  automation-protocol client like the Lisp host. It maps object-model calls onto registered commands
  and queries over the protocol, never links `draftbuilder-model`, and every call runs as a
  transaction ([DB-AUTO-038 to 043](../product/capabilities/18-customization-automation.md)).
- On other platforms the crate is absent; drawings with embedded objects keep showing the cached
  image and the objects are preserved ([DB-XREF-022](../product/capabilities/12-references-underlays.md)).

## Consequences

- Unsafe code is confined to one reviewed crate, fuzzed and tested against the protocol.
- Existing automation scripts reach DraftBuilder without an interpreter inside the application
  process, and a faulty script cannot corrupt the drawing.
- The bridge covers the object model through a mapping table; members without a mapping fail with a
  named error and appear in a coverage report.

## Alternatives considered

- **Hosting a scripting runtime inside the application process.** Couples script faults to the
  drawing and adds a second automation surface.
- **Embedding the container logic in the UI crate.** Spreads unsafe code through a crate that must
  stay safe and cross-platform.
- **No in-place editing, preservation only.** Leaves a conventional Windows workflow unsupported.

## Reopen when

The mapping coverage report shows that common scripts cannot run through the protocol with
acceptable latency, or the platform deprecates the interfaces the crate uses.

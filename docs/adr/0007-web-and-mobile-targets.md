# ADR-0007: Browser and mobile builds are targets of the same crates

The browser, tablet and phone builds compile the existing headless crates and the same UI crate for
additional targets; they add only thin entry-point applications and a per-command availability flag.

- **Status:** accepted
- **Suite context:** [ADR-0018 Web and mobile targets](https://github.com/ACKaraca/arc-suite/blob/main/docs/adr/0018-web-and-mobile-targets.md)

## Context

The suite allows a browser build (the same Rust crates compiled to WebAssembly with the same UI
kit) and tablet and phone builds, and keeps the desktop rule of no web view and no JavaScript
interface. DraftBuilder must open, measure, mark up and edit drawings on these targets, keep them
available offline, and synchronize through Arc Hub. Some desktop components do not fit a device: the
out-of-process Lisp host ([ADR-0005](0005-lisp-interpreter-out-of-process.md)), third-party DWG
reader bridges ([ADR-0004](0004-pure-rust-dwg-from-public-specification.md)) and very large
regenerations.

## Decision

- `draftbuilder-model`, `-commands`, `-io` and `-toolset` build for `wasm32` and for iOS and Android
  as well as for desktop; the layering gate also checks that they contain no code that needs a
  desktop-only facility. Platform facilities (file access, storage, clipboard, camera, location) come
  through the platform services of `arc-app`, never through direct calls from these crates.
- Two entry-point applications contain wiring only: `apps/draftbuilder-web` and
  `apps/draftbuilder-mobile`. They use `draftbuilder-ui`, which gains touch and pen input events,
  adaptive panel layout and the loupe for point entry, and gains no second UI toolkit.
- Every registered command carries a target-availability flag naming the stages and targets that
  offer it. Staging is a release decision made by changing flags, not by forking commands.
- The offline replica is the drawing, its journal and pending change sets, held in the storage the
  platform offers. Synchronization sends change sets through `arc-hub-api`; conflicts keep both
  states as on the desktop.
- Components that cannot run on a device run on Arc Hub as headless jobs: Lisp routines, sheet-set
  publishing, audits and batch conversion. The device reports when a job is required.

## Consequences

- One command registry, one journal and one file mapping serve every target; a fix lands once.
- Browser storage and memory limits bound the size of an offline drawing; the budget per target is
  declared and tested ([DB-WEB-007](../product/capabilities/28-web-and-mobile.md)).
- Threads in the browser depend on platform support; regeneration falls back to fewer workers and the
  performance budgets are stated per target.
- Some commands will be unavailable at early stages; the availability flag makes that visible.

## Alternatives considered

- **A web view shell around the desktop application.** Contradicts the suite rule for desktop
  applications and doubles the rendering paths.
- **A separate JavaScript application for viewing and markup.** Two document models and two file
  readers would diverge; the browser viewer of Arc Review remains the lightweight option for viewing.
- **Streaming rendered frames from a server.** Needs a network for every interaction and breaks the
  offline principle.

## Reopen when

A target cannot meet its declared memory or input-latency budget with the shared crates, or a
platform stops offering the storage or threading the offline replica depends on.

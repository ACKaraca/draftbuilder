# AGENTS.md

DraftBuilder is the Arc suite's 2D drafting and 3D CAD application on the DWG/DXF drawing database.
It saves `.dwg` and `.dxf` natively, preserves unknown content, and runs the conventional command
vocabulary, scripts and Lisp routines. Rust only; depends on `arc-core`, never on other products.

## Status and where to start

Design phase: no code and no builds yet. Read in this order: [docs/README.md](docs/README.md),
[docs/product/vision.md](docs/product/vision.md),
[docs/product/capabilities/README.md](docs/product/capabilities/README.md),
[docs/architecture/overview.md](docs/architecture/overview.md),
[docs/architecture/domain-model.md](docs/architecture/domain-model.md),
[docs/architecture/dwg-compatibility.md](docs/architecture/dwg-compatibility.md),
[docs/architecture/command-line.md](docs/architecture/command-line.md), [docs/adr/](docs/adr/README.md).

## Repository layout (target)

```
crates/draftbuilder-model/     drawing database, preservation, associativity, settings (headless)
crates/draftbuilder-commands/  commands, prompt machines, aliases (headless)
crates/draftbuilder-io/        DWG/DXF and other format mappings, loss reports (headless)
crates/draftbuilder-lisp/      Lisp-dialect interpreter (headless, protocol client)
crates/draftbuilder-toolset/   toolset framework (headless, phase 4)
crates/draftbuilder-ui/        tools, panels, cursor input, grips (egui)
toolsets/<name>/               one crate per toolset
apps/draftbuilder/             desktop application (wiring only)
apps/draftbuilder-cli/         headless CLI and automation protocol server
apps/draftbuilder-lisp/        out-of-process Lisp host
docs/                          product, architecture, roadmap, ADRs
xtask/                         workspace tasks
```

## Commands

No build exists yet. Once code lands, these apply:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo nextest run --workspace`
- `cargo xtask layers`

## Suite rules

These rules apply in every Arc repository. The canonical text is
[`docs/engineering/agent-rules.md`](https://github.com/ACKaraca/arc-suite/blob/main/docs/engineering/agent-rules.md)
in `arc-suite`; change it there first.

1. **A person is accountable.** Whoever runs an agent answers for what it submits.
2. **Start from an issue.** Search issues and pull requests, check that nobody else is assigned,
   assign yourself and set the status to *In Progress* before changing files. When you finish,
   comment with what changed, the commit or pull request link and open items, then update the status.
   *Done* requires met and verified acceptance criteria.
3. **Verify, do not guess.** Open a definition before using it. Confirm a cause in the code.
4. **The document is the authority and everything is a command.** No change reaches a document
   except through a validated command inside a transaction. Programmatic paths never open dialogs.
5. **Respect the layers.** A crate depends only on lower layers. Nothing below the UI layer depends
   on windowing, GPU or UI crates. Products never depend on other products.
6. **Never crash, never lose work.** No `unwrap`, `expect`, `panic!`, `todo!` or `unreachable!` in
   production code. No `unsafe` outside the FFI crates named in an ADR. Treat every file and message
   as hostile: bound recursion, allocation and counts. A crash fix ships with a regression test.
7. **Clean room.** Implement from published specifications, observed behavior and original
   reasoning. Never copy or consult another product's code, decompiled output, file-format reverse
   engineering, documentation text, icons, fonts, templates or sample files. Do not name our
   features after another company's features.
8. **Licenses.** Add a dependency only if its license is on the allowlist in
   `docs/engineering/dependency-policy.md` in `arc-suite`. Record every asset's origin and license.
9. **Rust only in desktop applications.** No web view, no JavaScript or TypeScript.
10. **Small, tested changes.** One concern per pull request, with the test that fails without it.
    Update the documents that describe the behavior you change in the same pull request.
11. **Report only what ran.** Say what you tested and what you could not run.
12. **Ask a human before** adding a license outside the allowlist, publishing or releasing,
    changing a file format or public protocol, adding a new repository or remote, spending money,
    handling secrets, or any destructive git operation. Otherwise keep working.
13. **Keep secrets and personal data out** of repositories, logs and requests.

## Repository invariants

- The in-memory model is the DWG/DXF drawing database; `.dwg` and `.dxf` are the native files and the
  `.draftb` sidecar holds only history, recovery data, settings and Hub metadata (ADR-0001).
- Stable IDs are derived from the drawing fingerprint and handle; handles are never reused (ADR-0002).
- Unknown objects, classes, fields and extended data are written back unchanged; every drop or
  invalidation appears in the preservation or loss report (ADR-0003).
- DWG and DXF code comes from public specifications only; third-party DWG readers run out of process
  and never on a core path (ADR-0004). Never state DWG fidelity as achieved without validation results.
- Every command has a non-interactive `run`, a hyphen prompt form and an optional prompt machine; the
  prompt form and `run` never open dialogs.
- The Lisp interpreter is a protocol client in its own process and never links the model (ADR-0005).
- Derived data (display lists, dimension blocks, hatch lines, dynamic block representations, proxy
  graphics) is rebuildable and carries the revision it was computed from.
- Toolset objects write proxy graphics and plain-entity fallbacks (ADR-0006).
- `draftbuilder-model`, `-commands`, `-io`, `-lisp` and `-toolset` build headless.

## Documentation rules

- No trace of the planning process. Never present DraftBuilder as derived from, copied from,
  standing in for or matching another product, and never describe a feature by another product's
  feature.
- Other companies' names only as formats and standards, in factual interoperability statements,
  once in the positioning paragraph of `vision.md`, and for libraries in architecture text.
- Use DraftBuilder's own vocabulary. Conventional command names and aliases are DraftBuilder command
  names; settings use DraftBuilder names, with widely used names accepted only as compatibility.
- Each fact once; link suite rules instead of copying them. Relative links inside the repository.
- Clean room: original wording, no copied documentation structure, dialog text or default lists.
- Capability IDs (`DB-<AREA>-<NNN>`) are permanent; status lives in issues, not in tables.

## Issue workflow

Start from an issue: search issues and pull requests, confirm nobody is assigned, assign yourself,
set *In Progress*. Statuses: Backlog, Ready, In Progress, In Review, Blocked, Done (*Done* only when
acceptance criteria are met and verified). On finishing, comment with what changed, the commit or
pull request link and open items; update the status; open follow-up issues after searching for
duplicates. Labels: `type:`, `tier:`, `phase:`, `area:<token>`, `status: blocked`,
`needs: verification`. Conventional Commits with DCO sign-off.

## Verification

Run the narrowest relevant check first (one crate's tests, one script test, one corpus file), widen
only when it fails for a relevant reason or shared code changed. DWG/DXF changes run the round-trip
and independent-reader checks for the affected classes. Report only what ran and what could not run.

## Ask a human about

- Changing the DWG/DXF mapping, the sidecar schema, the toolset fallback layout or the automation
  protocol.
- Adopting or bridging any third-party DWG library, or any license outside the allowlist.
- Adding corpus files whose license or origin is unclear.
- Withdrawing or renumbering capability IDs, or changing a milestone's exit criteria.

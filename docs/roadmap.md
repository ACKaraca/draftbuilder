# DraftBuilder roadmap

Milestones of DraftBuilder mapped onto the suite phases. Each milestone has a goal, a scope given as
capability ranges, dependencies, and exit criteria stated as acceptance scenarios from
[workflows.md](product/workflows.md). There are no dates; a milestone ends when its exit criteria
pass on Windows, macOS and Linux. The suite phases are defined in
[`arc-suite`](https://github.com/ACKaraca/arc-suite).

"Phase-N rows of an area" means the rows of that capability file whose Phase column is N.

| Milestone | Suite phase | Theme |
|---|---|---|
| M0 | 0 | Foundation readiness |
| M1 | 1 | Drawing core |
| M2 | 1 | Annotation, blocks, references and sheets |
| M3 | 2 | DWG write, automation, sheet sets and collaboration |
| M4 | 3 | 3D, reality data and open 3D exchange |
| M5 | 4 | Visualization and toolsets |
| M6 | 5 | Multi-user editing and assistance |

## M0 Foundation readiness (phase 0)

**Goal.** The `arc-core` layers DraftBuilder builds on exist and a drawing database prototype runs
on the reference shell.

**Scope.** No capability rows are delivered. Prototypes of the owner tree, handles and stable IDs
([ADR-0001](adr/0001-drawing-database-is-the-model.md), [ADR-0002](adr/0002-stable-ids-from-fingerprint-and-handle.md))
and a DXF entity reader prove the mapping onto `arc-doc` and `arc-draft`.

**Dependencies.** `arc-base`, `arc-geom`, `arc-doc`, `arc-command`, `arc-gpu`, `arc-ui`, `arc-app`
minimum; layering gate and CI.

**Exit criteria.**

- A DXF file with lines, arcs, polylines and text opens in the prototype, displays in the viewport,
  accepts a move command with undo and saves back with unchanged handles.

## M1 Drawing core (phase 1)

**Goal.** Drafters can open DXF and DWG drawings, draw and modify 2D geometry precisely from the
keyboard, manage layers and properties, and save DXF.

**Scope.**

- Phase-1 rows of [DB-DBASE-001 to 044](product/capabilities/01-drawing-database.md),
  [DB-CMD-001 to 049](product/capabilities/02-command-interface.md),
  [DB-SNAP-001 to 034](product/capabilities/03-precision.md),
  [DB-SEL-001 to 030](product/capabilities/04-selection-grips.md),
  [DB-DRAW-001 to 027](product/capabilities/05-draw-2d.md),
  [DB-MOD-001 to 036](product/capabilities/06-modify-2d.md),
  [DB-LAYER-001 to 039](product/capabilities/07-layers-properties.md).
- [DB-INTEROP-001](product/capabilities/19-interoperability-standards.md) (DWG read),
  [DB-INTEROP-003 to 005](product/capabilities/19-interoperability-standards.md),
  [DB-INTEROP-007](product/capabilities/19-interoperability-standards.md),
  [DB-INTEROP-009](product/capabilities/19-interoperability-standards.md),
  [DB-INTEROP-021 to 023](product/capabilities/19-interoperability-standards.md),
  [DB-COLLAB-025](product/capabilities/17-compare-history-collaboration.md).
- [DB-AUTO-027](product/capabilities/18-customization-automation.md) and
  [DB-AUTO-030](product/capabilities/18-customization-automation.md) (protocol and CLI basics).

**Dependencies.** `arc-core` phase-1 tag with `arc-draft` entities, layers and linetypes, `arc-io-dxf`,
`arc-io-dwg` read path, `arc-geom` offsets and intersections.

**Exit criteria.**

- [W01](product/workflows.md#w01-draft-a-part-from-a-template-with-precision-tools) passes
  interactively and as a script.
- [W05](product/workflows.md#w05-open-a-partners-drawing-edit-it-and-give-it-back) passes for its
  open, preservation report and DXF save steps (DWG save follows in M3).
- The pan, zoom and modify performance targets of [overview.md](architecture/overview.md) are met on
  the corpus drawings selected for M1.

## M2 Annotation, blocks, references and sheets (phase 1)

**Goal.** Complete 2D production drawings: text, tables, dimensions, hatches, blocks with
attributes, external references and images, layouts and PDF plotting, with aliases and scripts.

**Scope.**

- Phase-1 rows of [DB-TEXT-001 to 039](product/capabilities/08-text-tables-fields.md),
  [DB-DIM-001 to 035](product/capabilities/09-dimensions-leaders.md),
  [DB-HATCH-001 to 023](product/capabilities/10-hatch-fill.md),
  [DB-BLOCK-001 to 036](product/capabilities/11-blocks-attributes.md),
  [DB-XREF-001 to 030](product/capabilities/12-references-underlays.md),
  [DB-PLOT-001 to 045](product/capabilities/14-layouts-plotting.md).
- [DB-AUTO-008](product/capabilities/18-customization-automation.md),
  [DB-AUTO-011 to 012](product/capabilities/18-customization-automation.md),
  [DB-INTEROP-010](product/capabilities/19-interoperability-standards.md),
  [DB-INTEROP-019](product/capabilities/19-interoperability-standards.md),
  [DB-COLLAB-016](product/capabilities/17-compare-history-collaboration.md).

**Dependencies.** M1; `arc-draft` text, dimensions, hatches, blocks, tables, layouts, plot styles,
annotation scaling and sheet pipeline; `arc-io-pdf` output.

**Exit criteria.**

- [W02](product/workflows.md#w02-floor-plan-with-walls-openings-furniture-and-annotation) and
  [W04](product/workflows.md#w04-layout-viewports-annotation-scaling-and-pdf) pass.
- [W03](product/workflows.md#w03-block-with-attributes-and-dynamic-behavior) steps 1, 2 and 5 pass
  (dynamic behavior follows in M3).
- [W06](product/workflows.md#w06-coordinate-with-external-references-and-underlays) steps 1, 2, 3
  (image only) and 5 pass.
- The suite phase-1 deliverable "DraftBuilder 2D core" is met: draw, modify, annotate, layers,
  blocks, layouts, plot, DXF read and write, DWG read.

## M3 DWG write, automation, sheet sets and collaboration (phase 2)

**Goal.** DraftBuilder carries daily DWG production: it saves DWG with validated fidelity, runs
existing scripts and Lisp routines, evaluates and authors dynamic blocks and constraints, publishes
sheet sets, checks standards, compares drawings and works with Arc Hub.

**Scope.**

- All phase-2 rows of files 01 to 19, including
  [DB-INTEROP-002](product/capabilities/19-interoperability-standards.md) (DWG write),
  [DB-AUTO-001 to 032](product/capabilities/18-customization-automation.md) (phase-2 rows),
  [DB-BLOCK-021 to 036](product/capabilities/11-blocks-attributes.md) (phase-2 rows),
  [DB-CNSTR-001 to 023](product/capabilities/13-parametric-constraints.md),
  [DB-PLOT-035 to 045](product/capabilities/14-layouts-plotting.md) (phase-2 rows),
  [DB-XREF-023 to 027](product/capabilities/12-references-underlays.md),
  [DB-COLLAB-001 to 023](product/capabilities/17-compare-history-collaboration.md) (phase-2 rows),
  [DB-INTEROP-024 to 032](product/capabilities/19-interoperability-standards.md).

**Dependencies.** M2; `arc-io-dwg` write path; `arc-solve` constraints and expressions;
`arc-io-pdf` input; `arc-hub-api` and the Arc Hub Documents module (suite phase 2).

**Exit criteria.**

- [W03](product/workflows.md#w03-block-with-attributes-and-dynamic-behavior),
  [W05](product/workflows.md#w05-open-a-partners-drawing-edit-it-and-give-it-back),
  [W06](product/workflows.md#w06-coordinate-with-external-references-and-underlays),
  [W07](product/workflows.md#w07-data-linked-schedule-and-data-extraction),
  [W08](product/workflows.md#w08-publish-a-sheet-set),
  [W09](product/workflows.md#w09-bring-existing-customization-aliases-scripts-and-lisp-routines),
  [W10](product/workflows.md#w10-clean-up-and-standardize-a-legacy-drawing) and
  [W11](product/workflows.md#w11-compare-revisions-and-review-through-arc-hub) pass in full.
- DWG write fidelity for the 2018, 2013 and 2010 families is validated by an independent reader on
  the corpus and published per object class ([dwg-compatibility.md](architecture/dwg-compatibility.md)).

## M4 3D, reality data and open 3D exchange (phase 3)

**Goal.** Model solids, surfaces and meshes in drawings, derive drawing views, work with point
clouds and geolocation, and exchange 3D through open formats.

**Scope.** All phase-3 rows, including
[DB-M3D-001 to 041](product/capabilities/15-3d-modeling-visualization.md) and
[DB-M3D-051](product/capabilities/15-3d-modeling-visualization.md),
[DB-REAL-001 to 020](product/capabilities/16-point-clouds-geolocation.md),
[DB-SNAP-012](product/capabilities/03-precision.md), [DB-SNAP-027](product/capabilities/03-precision.md),
[DB-SEL-016](product/capabilities/04-selection-grips.md),
[DB-XREF-028 to 030](product/capabilities/12-references-underlays.md),
[DB-INTEROP-011 to 017](product/capabilities/19-interoperability-standards.md) (phase-3 rows),
[DB-AUTO-033 to 035](product/capabilities/18-customization-automation.md),
[DB-COLLAB-024](product/capabilities/17-compare-history-collaboration.md).

**Dependencies.** M3; `arc-brep` (shared with SolidBuilder in suite phase 3), `arc-mesh`, `arc-view`,
`arc-io-step`, `arc-io-ifc`, `arc-io-mesh`, `arc-io-pointcloud`, `arc-io-geo`; resolution of
[OQ-07](product/open-questions.md) and [OQ-20](product/open-questions.md).

**Exit criteria.**

- [W12](product/workflows.md#w12-3d-solid-model-and-drawing-views) and
  [W13](product/workflows.md#w13-point-cloud-to-georeferenced-as-built-plan) pass.
- A WebAssembly plug-in that adds a command and a custom object type installs from a package, runs
  sandboxed, and its objects are preserved by DraftBuilder without the plug-in.

## M5 Visualization and toolsets (phase 4)

**Goal.** Photoreal rendering through RenderBuilder, node graphs over commands, and the discipline
toolsets on a shared framework.

**Scope.** All phase-4 rows, including
[DB-M3D-042 to 050](product/capabilities/15-3d-modeling-visualization.md),
[DB-REAL-021](product/capabilities/16-point-clouds-geolocation.md),
[DB-INTEROP-015](product/capabilities/19-interoperability-standards.md),
[DB-AUTO-036](product/capabilities/18-customization-automation.md), and the toolset files
[20](product/capabilities/20-toolset-framework.md) to [27](product/capabilities/27-toolset-raster.md)
(TSF, TARCH, TMECH, TMEP, TELEC, TPLANT, TMAP, TRAST).

**Dependencies.** M4; RenderBuilder and the Arc Scene contract (suite phase 4); `arc-scene`,
`arc-nodes`; [ADR-0006](adr/0006-toolset-objects-degrade-to-entities.md).

**Exit criteria.**

- A camera view of the W12 model with sun, lights and materials renders in RenderBuilder from
  DraftBuilder, and edits to materials update the render without re-export.
- A node graph that places and dimensions geometry through commands runs headless and interactively
  with identical results.
- For each toolset, a drawing with its objects saved for other applications opens in an independent
  reader with plain entities on the expected layers, and reopens in DraftBuilder with the typed
  objects restored; the toolset files' own scenarios pass.

## M6 Multi-user editing and assistance (phase 5)

**Goal.** Several people edit one drawing at once through Arc Hub, and an optional assistant runs
commands with preview and explicit apply.

**Scope.** [DB-COLLAB-026 to 029](product/capabilities/17-compare-history-collaboration.md),
[DB-AUTO-037](product/capabilities/18-customization-automation.md), remaining phase-5 rows of the
toolset files.

**Dependencies.** M5; Arc Hub stage 2 element-level editing (suite phase 5); resolution of
[OQ-14](product/open-questions.md) and [OQ-19](product/open-questions.md).

**Exit criteria.**

- [W14](product/workflows.md#w14-several-drafters-edit-one-drawing) passes.
- An assistant request such as "dimension all doors on this sheet" shows a preview of the commands
  and their effects, applies only on confirmation as undoable transactions, and works with a local
  model and with no network.

# DraftBuilder vision

This document states why DraftBuilder exists, whom it serves, the principles that shape it, what it
deliberately leaves out, and what it provides that the drafting field does not yet offer.

## Problem and purpose

Most construction, manufacturing and utility deliverables still travel as drawings in the DWG and
DXF formats. The people who produce them work in a keyboard-driven command vocabulary that has been
stable for decades, maintain large libraries of blocks, templates, scripts and Lisp routines, and
exchange files with partners who use many different applications. The tools available to them tie
that work to one operating system, an account or a license server, closed customization formats and
single-writer files, and they report little about what changes when a drawing passes through a
different application.

DraftBuilder is a general-purpose 2D drafting and 3D CAD application whose native data model is the
DWG/DXF drawing database. It opens and saves `.dwg` and `.dxf` files directly, keeps everything it
does not understand, accepts the command names, aliases, scripts and Lisp routines that drafters
already use, and adds what a drawing database can gain from the Arc foundation: transactions with a
full operation journal, multi-threaded regeneration, a GPU viewport, a shared sheet and plot
pipeline, headless automation and optional multi-user collaboration through Arc Hub.

## Primary users and their deliverables

| User | Typical deliverables |
|---|---|
| Architectural and interior drafters | Floor plans, sections, elevations, details, door and room schedules, issued sheet sets |
| Mechanical and fabrication drafters | Detail and assembly drawings, flat layouts for cutting, DXF files for CNC and laser equipment |
| Civil, survey and utility technicians | Site and utility plans in survey coordinates, as-built drawings from point clouds, georeferenced exchange files |
| Building-services and electrical designers | Coordinated services plans, schematics, panel layouts, schedules (with the toolsets) |
| CAD managers | Templates, standards drawings, layer conventions, aliases, tool palettes, scripts and routines for a team |
| Engineers who model in 3D | Solid and surface models, sections, drawing views, files for 3D printing and downstream tools |
| Automation developers | Batch conversion, plotting and checking pipelines, custom commands, plug-ins |

## Product principles

1. **The drawing database is the model.** The in-memory model is aligned with the DWG/DXF data
   model object for object, so reading and writing a drawing is a mapping, not a translation.
2. **Keep what you do not understand.** Unknown objects, classes and extended data survive open and
   save unchanged; anything that cannot be kept is listed in a report, never dropped silently.
3. **Keyboard first, discoverable second.** The command line, aliases, options, transparent commands
   and scripts are complete; menus, panels and search make the same commands discoverable.
4. **Every interactive command has a non-interactive form.** The same command runs from the
   command line, a script, a Lisp routine, the automation protocol, the headless CLI and Arc Hub jobs,
   and programmatic execution never opens a dialog.
5. **State fidelity, do not claim it.** DWG fidelity is a set of targets verified by independent
   readers and published per version; every import, export and save-down produces a loss report.
6. **One application, every platform.** Windows, macOS and Linux receive the same executable
   features, file behavior and customization formats; the browser, tablets and phones are further
   targets of the same code, staged from viewing to full 2D editing
   ([ADR-0007](../adr/0007-web-and-mobile-targets.md)).
7. **Offline and account free.** No feature needs a network, an account or a license server; Arc
   Hub adds collaboration when a team chooses to run one.
8. **Customization is open.** Aliases, UI customization, tool palettes, macros and standards files
   use documented, diffable formats.

## Scope

- The drawing database: model space and layouts, entities, symbol tables, blocks, dictionaries,
  extended data, units and coordinate systems ([01](capabilities/01-drawing-database.md)).
- Command line, cursor input, workspace and navigation ([02](capabilities/02-command-interface.md)),
  precision tools and user coordinate systems ([03](capabilities/03-precision.md)), selection and
  grips ([04](capabilities/04-selection-grips.md)).
- 2D drawing and modification, layers and properties, text, tables, fields, dimensions, leaders,
  tolerances, hatches, blocks with attributes and dynamic behavior, external references and underlays,
  parametric constraints ([05](capabilities/05-draw-2d.md) to
  [13](capabilities/13-parametric-constraints.md)).
- Layouts, viewports, annotation scaling, plotting, publishing and sheet sets
  ([14](capabilities/14-layouts-plotting.md)).
- 3D solids, surfaces and meshes, sections and drawing views, visualization, and rendering through
  RenderBuilder ([15](capabilities/15-3d-modeling-visualization.md)); point clouds and geolocation
  ([16](capabilities/16-point-clouds-geolocation.md)).
- Drawing comparison, history, markup and multi-user editing
  ([17](capabilities/17-compare-history-collaboration.md)); customization and automation, including
  the Lisp-dialect interpreter ([18](capabilities/18-customization-automation.md)); interoperability
  and drawing standards ([19](capabilities/19-interoperability-standards.md)).
- Optional discipline toolsets built on a shared framework: architecture, mechanical, building
  services, electrical controls, plant, mapping and raster
  ([20](capabilities/20-toolset-framework.md) to [27](capabilities/27-toolset-raster.md)).
- Browser, tablet and phone builds with touch and pen input, offline drawings, synchronization and
  shared sessions through Arc Hub ([28](capabilities/28-web-and-mobile.md)).
- Windows-specific integration: embedded objects with edit in place
  ([DB-XREF-031](capabilities/12-references-underlays.md)) and a component-automation bridge that maps
  existing automation scripts onto the command model
  ([DB-AUTO-038](capabilities/18-customization-automation.md)).
- Video output of camera animations and walkthroughs
  ([DB-M3D-052](capabilities/15-3d-modeling-visualization.md)).
- Open readers for DGN V7 and Rhino 3DM, connectors for applications whose formats are not
  published and optional user-licensed bridges
  ([DB-INTEROP-033](capabilities/19-interoperability-standards.md)).

## Non-goals

Everything that law or licensing allows is in scope. What remains excluded is excluded for a legal
reason or because another Arc product owns the task.

| Non-goal | Reason | What DraftBuilder does instead |
|---|---|---|
| Loading compiled native add-ins written for other DWG applications | They link against another application's proprietary runtime libraries, which cannot be distributed or reimplemented without reverse engineering undocumented binary interfaces | Sandboxed WebAssembly plug-ins, the automation protocol, Python, the Lisp-dialect interpreter and the Windows automation bridge ([18](capabilities/18-customization-automation.md)) |
| Running compiled or protected Lisp files | Their formats are undocumented; running them would need reverse engineering | Lisp source files run unchanged where the dialect is covered |
| Reading formats without a published specification or open reader by reverse engineering | The suite never reverse engineers formats or binaries | Open readers where they exist ([DB-INTEROP-033, 034](capabilities/19-interoperability-standards.md)), connectors running inside the other application ([DB-INTEROP-035](capabilities/19-interoperability-standards.md)), optional user-licensed bridges, and IFC, STEP, glTF, OpenUSD, E57 and LAS |
| Raster codecs whose license forbids open distribution | The license prevents shipping them in an open-source product | GeoTIFF, TIFF, PNG and JPEG imagery; a user-installed codec only through a separate process |
| Vendor cloud services, account sign-in and usage telemetry | Offline-first, account-free principle | Self-hosted Arc Hub when a team wants collaboration |
| A from-scratch exact solid kernel inside DraftBuilder | Exact modeling is shared across products | 3D solids use the suite B-rep layer `arc-brep` |

## Positioning

DraftBuilder covers the professional scope of general-purpose DWG-based drafting and CAD applications
such as AutoCAD and AutoCAD LT, BricsCAD, ZWCAD, GstarCAD, DraftSight and ARES Commander, together
with the 2D scope of open drafting tools such as LibreCAD and QCAD, and it provides discipline
toolsets for architecture, mechanical design, building services, electrical controls, plant design,
mapping and raster work.

## Differentiators

| Differentiator | Capabilities |
|---|---|
| Unknown objects, classes and extended data are preserved byte for byte, with a preservation report on every open and save | [DB-DBASE-017 to 020](capabilities/01-drawing-database.md), [DB-INTEROP-005](capabilities/19-interoperability-standards.md) |
| DWG fidelity is published as version targets validated by independent readers, with machine-readable loss reports | [DB-INTEROP-004](capabilities/19-interoperability-standards.md), [DB-INTEROP-009](capabilities/19-interoperability-standards.md) |
| One executable feature set on Windows, macOS and Linux, including identical text layout | [DB-CMD-036](capabilities/02-command-interface.md), [DB-INTEROP-023](capabilities/19-interoperability-standards.md) |
| Several people edit one drawing at the same time by borrowing objects or regions; conflicts keep both versions | [DB-COLLAB-026 to 029](capabilities/17-compare-history-collaboration.md) |
| Drawing compare, per-object history and branch-and-merge of drawing versions are first-class tools | [DB-COLLAB-001 to 010](capabilities/17-compare-history-collaboration.md), [DB-COLLAB-024](capabilities/17-compare-history-collaboration.md), [DB-DBASE-040](capabilities/01-drawing-database.md) |
| A constraint solver with degrees-of-freedom feedback and deterministic results | [DB-CNSTR-019](capabilities/13-parametric-constraints.md), [DB-CNSTR-021](capabilities/13-parametric-constraints.md) |
| The sheet and plot pipeline is shared with every Arc product, so sheets look the same whichever application produced them | [14](capabilities/14-layouts-plotting.md) |
| Headless batch plotting, publishing, conversion, standards checking and reference repair | [DB-PLOT-037](capabilities/14-layouts-plotting.md), [DB-INTEROP-019](capabilities/19-interoperability-standards.md), [DB-INTEROP-027](capabilities/19-interoperability-standards.md), [DB-XREF-016](capabilities/12-references-underlays.md) |
| Open, documented customization files and existing Lisp routines run in a sandboxed interpreter | [DB-AUTO-002](capabilities/18-customization-automation.md), [DB-AUTO-015](capabilities/18-customization-automation.md) |
| Multi-threaded regeneration and GPU display of very large drawings, with survey coordinates displayed without jitter | [DB-DBASE-044](capabilities/01-drawing-database.md), [DB-CMD-041](capabilities/02-command-interface.md), [DB-DBASE-023](capabilities/01-drawing-database.md) |
| Deterministic, auditable detection of repeated geometry and conversion to blocks; versioned block libraries | [DB-BLOCK-033 to 036](capabilities/11-blocks-attributes.md) |
| Toolset objects are typed objects with property sets that degrade to plain entities for other applications | [20](capabilities/20-toolset-framework.md) |
| The same crates run in the browser and on tablets and phones, with touch and pen input, offline drawings and shared sessions through Arc Hub | [28](capabilities/28-web-and-mobile.md) |
| Existing automation scripts on Windows run through a bridge that maps every call onto the validated, undoable command model | [DB-AUTO-038 to 043](capabilities/18-customization-automation.md) |
| Keyboard-complete operation, screen-reader names and a localized interface from the first release | [DB-CMD-047 to 049](capabilities/02-command-interface.md) |

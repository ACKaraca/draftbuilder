# DraftBuilder capabilities

The capability files specify what DraftBuilder does, one functional area per file, as testable rows
that issues, pull requests and acceptance scenarios refer to.

## Legend

| Column | Meaning |
|---|---|
| ID | `DB-<AREA>-<NNN>`: product prefix, area token, number within the area starting at 001. An identifier is permanent and used once; a withdrawn capability keeps its number and is marked withdrawn. |
| Capability | Short name; a conventional command name is given in backticks where one exists. |
| Behavior | Observable behavior precise enough to implement and test. |
| Tier | **T1 Core**: required for first production use. **T2 Professional**: required for parity with established professional drafting tools. **T3 Advanced**: specialized or differentiating. |
| Phase | Suite phase in which the capability is planned (see [roadmap](../../roadmap.md)): 0 foundation, 1 first usable products, 2 building design and collaboration, 3 engineering breadth, 4 visualization and content, 5 depth and scale. |

Status is never recorded in these tables; it is tracked in GitHub issues and the project board.

## Areas

| File | Area token | Scope | Rows |
|---|---|---|---|
| [01-drawing-database.md](01-drawing-database.md) | DBASE | Spaces, entities, symbol tables, dictionaries, handles and stable IDs, extended data, preservation, files, recovery, undo | 44 |
| [02-command-interface.md](02-command-interface.md) | CMD | Command line, prompts, options, aliases, cursor input, workspace, navigation, settings, accessibility | 49 |
| [03-precision.md](03-precision.md) | SNAP | Object snaps, tracking, polar, ortho, grid, coordinate entry, user coordinate systems, units, measurement | 34 |
| [04-selection-grips.md](04-selection-grips.md) | SEL | Selection methods, query selection and filters, selection summary, grips | 30 |
| [05-draw-2d.md](05-draw-2d.md) | DRAW | 2D drawing commands | 28 |
| [06-modify-2d.md](06-modify-2d.md) | MOD | 2D modify commands, arrays, clipboard, draw order, geometry cleanup | 36 |
| [07-layers-properties.md](07-layers-properties.md) | LAYER | Layers, layer states and filters, viewport overrides, colors, linetypes, lineweights, transparency, properties | 39 |
| [08-text-tables-fields.md](08-text-tables-fields.md) | TEXT | Text, paragraph text, styles and fonts, fields, tables, data links, data extraction | 39 |
| [09-dimensions-leaders.md](09-dimensions-leaders.md) | DIM | Dimensions, dimension styles, tolerances, leaders, center marks | 35 |
| [10-hatch-fill.md](10-hatch-fill.md) | HATCH | Hatches, patterns, gradients, solid fills | 23 |
| [11-blocks-attributes.md](11-blocks-attributes.md) | BLOCK | Blocks, attributes, in-place editing, dynamic blocks, counting, block detection, libraries | 36 |
| [12-references-underlays.md](12-references-underlays.md) | XREF | External references, path management, raster images, underlays, PDF import, 3D references, embedded objects | 34 |
| [13-parametric-constraints.md](13-parametric-constraints.md) | CNSTR | Geometric and dimensional constraints, parameter table, solving | 23 |
| [14-layouts-plotting.md](14-layouts-plotting.md) | PLOT | Layouts, viewports, annotation scaling, page setups, plot styles, plotting, publishing, sheet sets | 45 |
| [15-3d-modeling-visualization.md](15-3d-modeling-visualization.md) | M3D | Solids, surfaces, meshes, sections, drawing views, visual styles, navigation, lights, materials, rendering, video output | 55 |
| [16-point-clouds-geolocation.md](16-point-clouds-geolocation.md) | REAL | Point clouds, geographic location, base maps, georeferenced rasters | 21 |
| [17-compare-history-collaboration.md](17-compare-history-collaboration.md) | COLLAB | Drawing compare, history, markup, Arc Hub, multi-user editing | 29 |
| [18-customization-automation.md](18-customization-automation.md) | AUTO | UI customization, templates, scripts, macros, Lisp-dialect interpreter, automation protocol, CLI, plug-ins, Windows automation bridge | 43 |
| [19-interoperability-standards.md](19-interoperability-standards.md) | INTEROP | DWG and DXF, other formats, open readers, connectors and bridges, fonts, drawing standards, transmittal packages | 36 |
| [20-toolset-framework.md](20-toolset-framework.md) | TSF | Shared toolset framework: typed objects, styles, display, property sets, schedules, projects, degradation | 83 |
| [21-toolset-architecture.md](21-toolset-architecture.md) | TARCH | Architecture toolset | 68 |
| [22-toolset-mechanical.md](22-toolset-mechanical.md) | TMECH | Mechanical toolset | 58 |
| [23-toolset-building-services.md](23-toolset-building-services.md) | TMEP | Building services toolset | 51 |
| [24-toolset-electrical-controls.md](24-toolset-electrical-controls.md) | TELEC | Electrical controls toolset | 61 |
| [25-toolset-plant.md](25-toolset-plant.md) | TPLANT | Plant toolset | 56 |
| [26-toolset-mapping.md](26-toolset-mapping.md) | TMAP | Mapping toolset | 58 |
| [27-toolset-raster.md](27-toolset-raster.md) | TRAST | Raster toolset | 48 |
| [28-web-and-mobile.md](28-web-and-mobile.md) | WEB | Browser, tablet and phone builds, touch and pen input, command access on small screens, offline drawings, synchronization and shared sessions through Arc Hub | 31 |
| [29-assistance.md](29-assistance.md) | ASSIST | Optional assistance: commands by intent, drawing questions, standards and cleanup, dimensioning and labels, block recognition, routines, PDF and raster conversion, sheet data, comparison and markups | 23 |

## Capabilities and issues

- An epic issue covers a contiguous range of capabilities in one area and names the range in its
  *Scope* section; feature issues name the individual identifiers they implement.
- An issue that changes the behavior of a capability updates the row in the same pull request.
- A new capability is added as the next free number in its area; numbers are never reused.
- Acceptance scenarios in [workflows.md](../workflows.md) and the [roadmap](../../roadmap.md) refer to
  capabilities by identifier; behaviors that need a decision first are listed in
  [open-questions.md](../open-questions.md).

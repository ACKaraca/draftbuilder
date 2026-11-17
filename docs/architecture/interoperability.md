# DraftBuilder interoperability

This document lists every format DraftBuilder reads or writes with its direction and fidelity level,
the mapping principles, how exchange is validated, how losses are reported and which formats are
deliberately unsupported. Fidelity levels follow the suite policy: **view** (geometry and appearance),
**data** (semantics, properties, relationships) and **edit** (native editing with full behavior).
All levels below are targets; each is verified as described under Validation before it is stated as
supported in release notes.

## Formats

### Drawings

| Format | Read | Write | Phase | Notes |
|---|---|---|---|---|
| DWG | edit | edit (phase 2) | 1 read, 2 write | Native format; version targets in [dwg-compatibility.md](dwg-compatibility.md) ([DB-INTEROP-001](../product/capabilities/19-interoperability-standards.md), [DB-INTEROP-002](../product/capabilities/19-interoperability-standards.md)) |
| DXF (ASCII and binary) | edit | edit | 1 | Native format, implemented from the published DXF reference ([DB-INTEROP-003](../product/capabilities/19-interoperability-standards.md)) |
| DWT template | edit | edit | 1 | A DWG used as a template ([DB-AUTO-008](../product/capabilities/18-customization-automation.md)) |
| DWS standards file | data | data | 2 | A DWG holding standard layers and styles ([DB-INTEROP-024](../product/capabilities/19-interoperability-standards.md)) |
| DGN V7 | view and data (read); view (underlay) | — | 3 underlay, 5 read | From its published specification ([DB-INTEROP-033](../product/capabilities/19-interoperability-standards.md), [DB-XREF-028](../product/capabilities/12-references-underlays.md)) |
| Later DGN versions | view | view | 3 | Subject to a published specification ([OQ-04](../product/open-questions.md)) |
| `.draftb` sidecar package | edit | edit | 1 | Journal history, recovery journal, settings, Hub metadata; a documented SQLite schema |
| Other Arc documents | view | — | 3 | Attached as references through the Arc document package ([DB-XREF-029](../product/capabilities/12-references-underlays.md)) |
| Arc exchange package | data | data | 5 | Written and read by connectors inside other applications ([DB-INTEROP-035](../product/capabilities/19-interoperability-standards.md)); specified in `arc-core` |

### Sheets, images and documents

| Format | Read | Write | Phase | Notes |
|---|---|---|---|---|
| PDF | view (underlay), data (import to geometry) | view, with layers, hyperlinks, bookmarks | 1 write, 2 read | [DB-PLOT-032](../product/capabilities/14-layouts-plotting.md), [DB-XREF-023](../product/capabilities/12-references-underlays.md), [DB-XREF-027](../product/capabilities/12-references-underlays.md) |
| SVG | — | view | 1 | [DB-PLOT-033](../product/capabilities/14-layouts-plotting.md) |
| PNG, JPEG, TIFF, BMP | view (images) | view (plot and export) | 1 | Raster images and raster plotting |
| GeoTIFF and world files | view, placed by georeference | — | 3 | [DB-REAL-018](../product/capabilities/16-point-clouds-geolocation.md) |
| DWF, DWFx | view | view | 3 | Subject to a published specification ([OQ-05](../product/open-questions.md)) |
| CTB, STB plot style tables | edit | edit | 1 | [DB-PLOT-024](../product/capabilities/14-layouts-plotting.md) |
| Plotter configuration files | data (imported settings) | — | 2 | [DB-PLOT-023](../product/capabilities/14-layouts-plotting.md) |
| DSD sheet lists | data | data | 2 | [DB-PLOT-036](../product/capabilities/14-layouts-plotting.md) |
| Sheet set files | edit | edit | 2 | Format decision in [OQ-06](../product/open-questions.md) |

### Content and configuration

| Format | Read | Write | Phase | Notes |
|---|---|---|---|---|
| LIN linetype libraries | edit | edit | 1 | [DB-LAYER-029](../product/capabilities/07-layers-properties.md) |
| PAT hatch pattern libraries | edit | edit | 1 | [DB-HATCH-008](../product/capabilities/10-hatch-fill.md) |
| SHX shape and big fonts, TrueType, OpenType | view | — | 1 | [DB-INTEROP-021](../product/capabilities/19-interoperability-standards.md) |
| Alias files (PGP) | data | data | 1 | [DB-CMD-010](../product/capabilities/02-command-interface.md) |
| Scripts (SCR) | data | data | 1 | [DB-AUTO-011](../product/capabilities/18-customization-automation.md) |
| Lisp source (LSP) and dialog definitions (DCL) | data | — | 2 | [DB-AUTO-015](../product/capabilities/18-customization-automation.md) |
| Layer state files (LAS) | data | data | 1 | [DB-LAYER-012](../product/capabilities/07-layers-properties.md) |
| UI customization files of other applications | — | — | — | Decision in [OQ-08](../product/open-questions.md) |
| Embedded objects (object-linking frames) | view, edit in place on Windows | view | 1 preserve, 5 edit | [DB-XREF-022](../product/capabilities/12-references-underlays.md), [DB-XREF-031](../product/capabilities/12-references-underlays.md) |
| CSV, XLSX | data | data | 2 | Data links, extraction, table export ([DB-INTEROP-018](../product/capabilities/19-interoperability-standards.md)) |
| KML, KMZ | data (location) | data | 3 | [DB-INTEROP-017](../product/capabilities/19-interoperability-standards.md) |

### 3D and reality data

| Format | Read | Write | Phase | Notes |
|---|---|---|---|---|
| STEP, IGES | data (solids and surfaces) | data | 3 | Through `arc-brep` ([DB-INTEROP-012](../product/capabilities/19-interoperability-standards.md)) |
| SAT | data | data | 3 | Subject to specification availability ([OQ-07](../product/open-questions.md)) |
| IFC | view and data as reference | — | 3 | [DB-INTEROP-011](../product/capabilities/19-interoperability-standards.md) |
| Rhino 3DM | view and data | — | 5 | Through a permissively licensed open reader ([DB-INTEROP-034](../product/capabilities/19-interoperability-standards.md), [DB-XREF-034](../product/capabilities/12-references-underlays.md)) |
| glTF, OBJ, STL, 3MF, OpenUSD | view (meshes) | view | 3 | [DB-INTEROP-014](../product/capabilities/19-interoperability-standards.md), [DB-M3D-051](../product/capabilities/15-3d-modeling-visualization.md) |
| Arc Scene | — | view and materials | 4 | Rendering through RenderBuilder ([DB-INTEROP-015](../product/capabilities/19-interoperability-standards.md)) |
| LAS, LAZ, E57 | view | — | 3 | [DB-REAL-001](../product/capabilities/16-point-clouds-geolocation.md) |
| IES photometric files | data | — | 4 | Light distributions ([DB-M3D-042](../product/capabilities/15-3d-modeling-visualization.md)) |
| Video (WebM, MP4 with open codecs) | — | view | 5 | Camera paths and walkthroughs ([DB-M3D-052](../product/capabilities/15-3d-modeling-visualization.md)) |

## Mapping principles

1. **Format crates produce neutral models; DraftBuilder owns the mapping.** `arc-io-*` crates parse
   and write formats; `draftbuilder-io` maps between those models and the drawing database.
2. **DWG and DXF map one to one.** Every object of a modelled class becomes the matching database
   object with all its fields; nothing is approximated on the native path.
3. **Keep the original.** Content that cannot be modelled is preserved (DWG, DXF) or the source file is
   kept and referenced (other formats).
4. **Units are explicit.** Imports declare the source unit and the drawing's insertion unit decides
   scaling; a unitless source is placed at the scale the user confirms
   ([DB-INTEROP-007](../product/capabilities/19-interoperability-standards.md)).
5. **Identifiers are mapped.** Source identifiers (handles, IFC global IDs, STEP entity names) are
   kept on mapped objects so updates and comparisons can find them again.
6. **Exports describe their simplifications.** Exploding a toolset object, flattening a 3D object or
   converting a newer object for an older version is listed in the loss report.

## Validation

- Every DWG and DXF the writer produces in CI is read back by DraftBuilder and by at least one
  independent reader that shares no code with it, run out of process. Object counts, handles,
  geometry within tolerance, symbol tables and preserved content are compared.
- PDF and SVG output is checked by independent parsers and by raster comparison with reference
  images.
- STEP, IFC and mesh exports are read by independent readers of those formats.
- Each format row carries its validation status in the release notes; a level is claimed only when
  its validation suite passes on all three platforms.

## Loss reporting

Every open, import, save, save-down and export produces a loss report
([DB-INTEROP-005](../product/capabilities/19-interoperability-standards.md),
[DB-INTEROP-009](../product/capabilities/19-interoperability-standards.md)). The report lists, per
object class: preserved unchanged, mapped, approximated (with method), dropped (with reason) and
invalidated foreign data. It is shown interactively, returned as structured data by the automation
protocol and written as JSON by the CLI, so pipelines can fail on unexpected losses.

## Formats without a published specification

A proprietary format with a published specification or a permissively licensed open reader is read
natively behind an `arc-io-*` crate (DGN V7, Rhino 3DM). For every other proprietary format the
exchange goes through one of two routes defined by
[ADR-0017](https://github.com/ACKaraca/arc-suite/blob/main/docs/adr/0017-exchange-connectors-and-licensed-bridges.md):

- **Connectors.** A connector is an add-in that runs inside the user's own licensed copy of the other
  application, uses only its public API and writes or reads the Arc exchange package, which
  DraftBuilder opens with geometry, properties and identifiers ([DB-INTEROP-035](../product/capabilities/19-interoperability-standards.md)).
- **Licensed bridges.** A conversion library that its owner licenses to the user runs as a separate
  process; it is never distributed with DraftBuilder or required for a core workflow
  ([DB-INTEROP-036](../product/capabilities/19-interoperability-standards.md)).

Both routes produce loss reports like every other import and export.

## Deliberately unsupported

Reverse engineering undocumented formats or binaries is excluded because of the legal risk it carries;
the rows below follow from that rule or from license terms.

| Format or mechanism | Reason | Exchanged instead through |
|---|---|---|
| Proprietary formats without a published specification or open reader, read directly | Never reverse engineered | Connectors, licensed bridges, IFC, STEP, glTF, OpenUSD |
| Proprietary point cloud project formats read directly | No published specification | E57, LAS, LAZ, or a connector |
| FBX read directly | Available only through a proprietary SDK | glTF, OpenUSD |
| Compiled protected Lisp files | Undocumented compiled format | Lisp source files |
| Compiled native add-ins of other applications | Bound to another application's proprietary runtime libraries | WebAssembly plug-ins, the automation protocol, the Windows automation bridge |
| Raster codecs whose license forbids open distribution | License | GeoTIFF, TIFF, PNG, JPEG |

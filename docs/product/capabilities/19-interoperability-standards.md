# Interoperability and drawing standards

DraftBuilder reads and writes DWG and DXF as its native formats, exchanges other 2D and 3D formats
at stated fidelity levels, reports every loss, and helps organizations enforce drawing standards
and deliver complete packages. DWG fidelity is stated as targets that are validated with
independent readers, never assumed.

## Domain objects

- **Format version target** — a DWG or DXF version family that DraftBuilder aims to read or write, with its validation status per release.
- **Fidelity report** — a machine-readable and human-readable record of what an open, save or exchange preserved, approximated or dropped.
- **Standards file** — a DWS drawing whose layers, linetypes, text styles and dimension styles define an organization standard.
- **Layer mapping** — a saved set of rules translating layer names and properties to a standard.
- **Transmittal package** — a folder or ZIP archive with a drawing set, its dependencies and a report.

## DWG and DXF

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-INTEROP-001 | DWG read | Opens DWG files of the R13 through 2018 format families as version targets, mapping every object the drawing database models and preserving the rest; older R12-and-earlier files are a later target. Reading is implemented in pure Rust from the publicly available format specification. | T1 | 1 |
| DB-INTEROP-002 | DWG write | Saves DWG in the 2018 format family by default and in older families on request, writing preserved unknown objects back unchanged when their owners were not edited. | T1 | 2 |
| DB-INTEROP-003 | DXF read and write | Reads and writes DXF in ASCII and binary form for versions R12 through 2018 from the published DXF reference, including the header, classes, tables, blocks, entities, objects and thumbnail sections. | T1 | 1 |
| DB-INTEROP-004 | Version targets and validation | Each release publishes, per format version and object type, whether reading and writing are targeted and their validation status; round trips of a test corpus are checked with independent readers run out of process in continuous integration. | T1 | 1 |
| DB-INTEROP-005 | Fidelity report on open and save | Opening or saving a drawing produces a fidelity report listing objects preserved without interpretation, approximated, converted or dropped, grouped by type with counts and handles; a status bar indicator shows when the report has entries. | T1 | 1 |
| DB-INTEROP-006 | Third-party reader bridge | Where an optional third-party reader is configured for files the built-in reader cannot open, it runs as a separate process exchanging bytes with DraftBuilder, is never required for any workflow, and its use is recorded in the fidelity report. | T3 | 2 |
| DB-INTEROP-007 | Units and coordinates on import | Imported content is scaled by its declared insertion units into the target drawing units and placed by insertion point or by shared geographic location; the applied scale and transform are reported. | T1 | 1 |
| DB-INTEROP-008 | Protected drawings | Password-protected or digitally signed DWG files are detected on open; content that cannot be read is refused with a message naming the protection, and a signature is reported as invalidated when a signed file is saved. | T2 | 2 |
| DB-INTEROP-009 | Machine-readable loss reports | Every import, export, open, save and save-down writes its fidelity report in a documented JSON format next to the human-readable summary, so batch jobs can gate on losses. | T2 | 1 |

## Other formats

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-INTEROP-010 | SVG and raster export (`EXPORT`) | Exports the current view or a selection to SVG and to PNG, JPEG and TIFF at a chosen resolution, outside the plot pipeline. | T2 | 1 |
| DB-INTEROP-011 | IFC as reference | IFC 2x3, IFC 4 and IFC 4.3 models attach as references displayed with their geometry, object properties and spatial structure through `arc-io-ifc`, snappable and sectionable; they are not converted into drawing objects unless bound. | T2 | 3 |
| DB-INTEROP-012 | STEP and IGES | Solids and surfaces import from and export to STEP and IGES through `arc-brep`, with units and assembly structure mapped to blocks. | T2 | 3 |
| DB-INTEROP-013 | SAT exchange | Solid data in the SAT text form is imported and exported where an implementation from a published description is available. | T3 | 3 |
| DB-INTEROP-014 | Mesh formats | glTF, OBJ, STL, PLY, 3MF and OpenUSD import as meshes and export from solids, surfaces and meshes through `arc-io-mesh` and `arc-io-usd`, with tessellation settings and materials where the format carries them. | T2 | 3 |
| DB-INTEROP-015 | Arc Scene export | 3D content, cameras, lights and materials export as an Arc Scene for RenderBuilder and other Arc products, and updates stream while the drawing changes. | T2 | 4 |
| DB-INTEROP-016 | DGN exchange | DGN files import to drawing objects and export from them with a mapping of levels, colors, line styles and fonts, for the DGN versions whose format is published. | T3 | 3 |
| DB-INTEROP-017 | KML and KMZ | Locations import from KML and KMZ; selected geolocated content exports to KML with coordinates transformed to geodetic. | T3 | 3 |
| DB-INTEROP-018 | CSV and XLSX | Point lists import from CSV as points or blocks with attributes; tables, extraction results and reports export to CSV and XLSX. | T2 | 2 |
| DB-INTEROP-019 | Batch conversion | The command-line application converts files or folders between DWG versions, DXF, PDF, SVG and mesh formats in parallel, with one fidelity report per file and a summary. | T2 | 1 |
| DB-INTEROP-020 | Other Arc documents | Drawings exported by other Arc products (sheets and details) open as ordinary DWG; Arc document packages attach as references through their published views. | T2 | 2 |

## Fonts

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-INTEROP-021 | SHX font support | SHX shape fonts and big fonts referenced by drawings are read from configured font folders; an openly licensed set of replacement shape fonts ships so common font names render on every platform. | T1 | 1 |
| DB-INTEROP-022 | Substitute-font layout check | When text is shown with a substitute font (DB-TEXT-021), paragraph text whose laid-out size differs from the size recorded in the file by more than a tolerance is flagged in the font report, so overflowing text is found before plotting or exchange. | T3 | 2 |
| DB-INTEROP-023 | Cross-platform text metrics | TrueType and OpenType text lays out identically on all three platforms because fonts are rendered by the application rather than the operating system. | T1 | 1 |

## Drawing standards

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-INTEROP-024 | Associate standards files (`STANDARDS`) | One or more DWS standards files are associated with a drawing, in priority order; the association is saved in the drawing. | T2 | 2 |
| DB-INTEROP-025 | Check standards (`CHECKSTANDARDS`) | Compares layers, linetypes, text styles and dimension styles with the associated standards, lists each violation with a suggested replacement, and fixes violations singly or all at once as undoable commands; ignored violations are remembered. | T2 | 2 |
| DB-INTEROP-026 | Standards notification | When standards are associated, a status indicator and optional notification report new violations as named objects are created or changed. | T3 | 2 |
| DB-INTEROP-027 | Batch standards check | The command-line application checks a file set against standards files and writes a report per drawing and a summary, without modifying files unless fixing is requested. | T2 | 2 |
| DB-INTEROP-028 | Layer mapping (`LAYTRANS`) | Maps layers of a drawing to layers of a standard by name and by wildcard rules, optionally forcing object properties to by-layer and transferring objects; mappings save to and load from documented files and run headless. | T2 | 2 |
| DB-INTEROP-029 | Import missing standard content | Layers, linetypes and styles defined in the associated standards file or template but missing from the drawing are listed and copied in on demand, singly or all at once, as one undoable command; new-layer reconciliation itself is DB-LAYER-024. | T2 | 2 |

## Delivery packages

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-INTEROP-030 | Transmittal package | Collects a drawing or sheet set with its references, images, underlays, fonts, plot style tables, device settings and sheet set files into a folder or ZIP archive, with a report listing every file and every dependency that could not be found. | T2 | 2 |
| DB-INTEROP-031 | Package options | Options rewrite reference paths to relative or remove them, bind references, include or exclude fonts and textures, and convert all drawings to a chosen DWG version with the fidelity report included; named package setups save the options. | T2 | 2 |
| DB-INTEROP-032 | Headless packaging | The command-line application builds transmittal packages from a drawing list or sheet set for scripted delivery. | T3 | 2 |

# External references and underlays

External references and underlays bring other files into a drawing by link rather than by copy.
DraftBuilder resolves their paths predictably, reports every broken or changed link, and can repair
paths for whole projects without opening a window.

## Domain objects

- **External reference** — a link to another DWG or DXF with attachment type, saved path, found path, insertion transform, clip and load state.
- **Reference clip** — a rectangular or polygonal boundary, optionally inverted, with optional front and back clipping planes.
- **Raster image** — a linked image with size, placement, clip, display adjustments and draw order.
- **Underlay** — a linked PDF page, DGN V7 file or (subject to specification) DWF file displayed with layer control and snapping.
- **Embedded object** — a document of another application held in the drawing as an object frame with a cached image and, on Windows, edited in place through operating-system object linking.
- **3D reference** — a linked Arc document or IFC, STEP, glTF or Rhino 3DM file placed in model space.
- **Search path set** — the ordered project and support folders used to resolve references.

## Drawing references

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-XREF-001 | Attach reference (`XATTACH`, `-XREF`) | Links a DWG or DXF at an insertion point, scale and rotation as an attachment, which carries its own nested references into hosts, or as an overlay, which does not; the hyphen form provides every option for scripts. | T1 | 1 |
| DB-XREF-002 | Path types | Stores the saved path as full, relative to the host drawing, or name only; the type can be changed later for selected references and is preserved on save. | T1 | 1 |
| DB-XREF-003 | Path resolution order | Resolves a reference by its saved path, then the host drawing folder, then project search paths, then support paths; the found path is shown alongside the saved path. | T1 | 1 |
| DB-XREF-004 | Nesting and cycles | Loads nested references to any depth, detects circular references, loads each file once and reports the cycle instead of recursing. | T1 | 1 |
| DB-XREF-005 | Reload, unload, detach | Reloads changed references, unloads them while keeping the link, and detaches them with their dependent definitions; each operation is one undoable transaction. | T1 | 1 |
| DB-XREF-006 | Open referenced drawing (`XOPEN`) | Opens the source drawing of a picked reference in a new tab. | T1 | 1 |
| DB-XREF-007 | Clip (`XCLIP`) | Clips a reference or block with a rectangular or polygonal boundary or a selected polyline, optionally inverted and with front and back planes; the clip boundary can be shown, hidden, regenerated as a polyline or removed. | T1 | 1 |
| DB-XREF-008 | Bind (`XBIND`, bind) | Converts references into blocks, either keeping dependent layer and style names distinct with a collision-free prefix or merging them into host names; selected dependent definitions can be bound individually. | T1 | 1 |
| DB-XREF-009 | Change notification | Watches referenced files and notifies when a source changes on disk, offering reload of the affected references. | T1 | 1 |
| DB-XREF-010 | Partial and demand loading | Loads only the layers and regions of a referenced drawing that are visible in the host when a spatial and layer index exists, reducing memory for large reference sets. | T2 | 2 |
| DB-XREF-011 | Reference fade | References and underlays display faded by a set percentage so host geometry stands out, without affecting plotting. | T1 | 1 |

## References panel and path repair

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-XREF-012 | References panel (`XREF`) | Lists every reference, image and underlay in tree or list view with status (loaded, unloaded, not found, unresolved, unreferenced, orphaned), type, saved and found paths, size, date and preview; all operations are available from it. | T1 | 1 |
| DB-XREF-013 | Find and replace paths | Replaces a path prefix across selected or all references in one transaction and shows which references resolve after the change. | T2 | 1 |
| DB-XREF-014 | Repair missing references | For each missing reference the panel searches the project search paths and nearby folders by file name and offers matches; accepting re-paths the reference with the chosen path type. | T2 | 1 |
| DB-XREF-015 | Project search paths | A project definition holds named search folders shared by every drawing in the project, so references resolve the same on every machine regardless of drive letters. | T2 | 2 |
| DB-XREF-016 | Reference integrity report and headless repair | The command-line application lists every reference in a set of drawings with status and resolution, and applies path rules (prefix maps, path type changes) to all of them, writing a report of what changed. | T3 | 2 |

## Raster images

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-XREF-017 | Attach image (`IMAGEATTACH`) | Links PNG, JPEG, TIFF, BMP and other common raster formats by insertion point, scale and rotation, with resolution and unit information used for default size. | T1 | 1 |
| DB-XREF-018 | Image clip and frame (`IMAGECLIP`, `IMAGEFRAME`) | Clips images to rectangular or polygonal boundaries, optionally inverted; image frames can be shown and plotted, shown but not plotted, or hidden. | T1 | 1 |
| DB-XREF-019 | Image adjustments (`IMAGEADJUST`) | Sets brightness, contrast and fade per image; transparent pixels of PNG and TIFF images show the drawing behind when transparency is on. | T1 | 1 |
| DB-XREF-020 | Image display quality | Displays large images through tiled, multi-resolution caching so zooming stays responsive; plotting always uses full resolution. | T2 | 1 |
| DB-XREF-021 | Draw order of images | Images take part in draw order with other objects, typically sent behind linework. | T1 | 1 |
| DB-XREF-022 | Preserved embedded objects | Embedded object frames read from DWG are preserved and displayed and plotted from their cached image on every platform; editing their contents is available only where DB-XREF-031 applies. | T2 | 1 |

## Underlays

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-XREF-023 | PDF underlay (`PDFATTACH`) | Links a chosen page of a PDF file with insertion, scale and rotation; vector content displays at any zoom and plots as vectors. | T2 | 2 |
| DB-XREF-024 | Underlay display controls | Underlays support fade, contrast, monochrome display, rectangular or polygonal clip and frame visibility. | T2 | 2 |
| DB-XREF-025 | Underlay layers | Layers embedded in a PDF underlay are listed and turned on or off individually within the host drawing. | T2 | 2 |
| DB-XREF-026 | Underlay snapping | Object snaps (endpoint, midpoint, nearest, perpendicular, intersection and others) work on vector geometry inside underlays, and can be switched off per drawing. | T2 | 2 |
| DB-XREF-027 | PDF import (`PDFIMPORT`) | Converts the vector content of a PDF page or underlay into polylines, hatches, solid fills, text and images on layers mapped from the PDF; text drawn as shape-font strokes can be recognized and converted to text objects; raster content is optional. | T2 | 2 |
| DB-XREF-028 | DGN and DWF underlays | Links DGN V7 files (read through [DB-INTEROP-033](19-interoperability-standards.md)) and DWF files as underlays with layer control and snapping; later DGN versions and DWF are subject to a published specification for the format version. | T3 | 3 |

## 3D references

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-XREF-029 | Attach Arc documents and open 3D formats | Links other Arc documents and IFC, STEP or glTF files as 3D references in model space, positioned by insertion point or by shared coordinates, with object snaps on their geometry and clipping. | T2 | 3 |
| DB-XREF-030 | 3D reference updates | 3D references reload when the source changes and keep their placement; elements in an Arc document or IFC reference show their properties when selected. | T2 | 3 |
| DB-XREF-034 | Rhino 3DM reference | Links Rhino 3DM files as 3D references through the open reader of [DB-INTEROP-034](19-interoperability-standards.md), with units, layers and object names mapped, snapping on curves and meshes, clipping, and reload when the source changes. | T3 | 5 |

## Embedded objects

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-XREF-031 | Embedded objects with edit in place on Windows | On Windows, `INSERTOBJ` inserts a new or an existing document of another application as an embedded or linked object; activating it edits the content in place or in the other application's window where the platform provides that, and the cached image refreshes when editing ends. Elsewhere the object is preserved and shown ([DB-XREF-022](12-references-underlays.md)). | T3 | 5 |
| DB-XREF-032 | Embedded object trust | A foreign component starts only after the user confirms for that object or for a trusted location; link paths are listed by `OLELINKS` with update, change-source and break actions; a setting turns activation off, and headless runs never start a foreign component. | T3 | 5 |
| DB-XREF-033 | Embedded object properties | Size, crop, scale, rotation, layer, draw order and plot quality of an embedded object are set like those of an image; the object plots from its cached image and writes back to DWG as an object frame other applications can open. | T3 | 5 |

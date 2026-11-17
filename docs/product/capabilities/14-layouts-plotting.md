# Layouts, viewports, plotting and sheet sets

Layouts compose model-space content onto paper through scaled viewports, and the plot pipeline
turns layouts into PDF, vector and raster output, singly or as published sets. The pipeline is the
`arc-draft` sheet pipeline shared by every Arc product, so output from DraftBuilder matches output
from the other applications.

## Domain objects

- **Layout** — a named paper-space container with a paper size, a page setup, its own entities and viewports.
- **Viewport** — a window on a layout that shows model space at a scale, with its own view, visual style, UCS and layer state.
- **Annotation scale** — a named scale from the drawing's scale list; annotative objects hold one representation per scale.
- **Page setup** — a named set of output device, paper, area, scale, style table and options.
- **Output device configuration** — a device definition (system printer, PDF, SVG or raster writer) with paper sizes and margins.
- **Plot style table** — a color-dependent (CTB) or named (STB) table mapping objects to output pen properties.
- **Publish set** — an ordered list of layouts from one or more drawings with per-sheet setup and output options.
- **Sheet set** — a file-based collection of sheets, subsets, views, properties and resource locations across drawings.

## Layouts

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-PLOT-001 | Layout tabs | Model and every layout appear as tabs; a tab menu creates a new layout, creates one from a template drawing's layout, copies, moves, renames, deletes and selects all layouts. The `LAYOUT` command offers every operation in a non-interactive form. | T1 | 1 |
| DB-PLOT-002 | Layout switching | Switching layouts shows the target with its viewports regenerated in the background so the paper and paper-space entities appear immediately. Next/previous layout keyboard shortcuts cycle the tabs. | T1 | 1 |
| DB-PLOT-003 | Layout thumbnails | Hovering a drawing or layout tab shows a thumbnail preview; a gallery view shows all layouts of all open drawings with plot and page setup actions. | T2 | 1 |
| DB-PLOT-004 | Paper and model switching | Double-clicking inside a viewport activates model space through it and outside returns to paper space; `MSPACE` and `PSPACE` and a status bar control do the same. Edits made through a viewport change model space. | T1 | 1 |

## Viewports

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-PLOT-005 | Create viewports (`MVIEW`) | Creates rectangular viewports by corners, polygonal viewports by points, viewports from a closed polyline, circle, ellipse, spline or region, fit-to-paper viewports, and standard arrangements of two to four viewports; a saved tiled model configuration can be restored into a layout. | T1 | 1 |
| DB-PLOT-006 | Viewport scale | A viewport's scale is set from the scale list, typed as a ratio or set by zooming; the status bar shows and edits the scale of the active or selected viewport. When scale sync is on, the viewport scale and the annotation scale change together. | T1 | 1 |
| DB-PLOT-007 | Lock display | A locked viewport keeps its scale and view while model space is edited through it; zoom and pan in a locked viewport act on the layout instead. | T1 | 1 |
| DB-PLOT-008 | Viewport clipping (`VPCLIP`) | Clips an existing viewport to a new polygon or closed object or removes the clip; the clip boundary moves with the viewport. | T2 | 1 |
| DB-PLOT-009 | Viewport on/off and active limit | Viewports can be turned off so they neither display nor plot; a setting limits how many viewports regenerate at once, with inactive ones shown as frames until activated. | T2 | 1 |
| DB-PLOT-010 | Per-viewport visual style and shade plot | Each viewport has its own visual style and an output mode (as displayed, wireframe, hidden line, named visual style or rendered) with a raster resolution for shaded output. | T2 | 3 |
| DB-PLOT-011 | Per-viewport UCS | A viewport can keep its own UCS, which becomes current when the viewport is activated; a setting turns this off. | T2 | 1 |
| DB-PLOT-012 | Maximize viewport (`VPMAX`, `VPMIN`) | Temporarily expands a viewport to fill the drawing area for editing without changing its scale or view; restoring returns the original view even if the user zoomed. | T2 | 1 |

## Annotation scaling

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-PLOT-013 | Annotative objects | Text, multiline text, dimensions, leaders, tolerances, hatches, blocks and attributes can be annotative: authored at paper size and displayed at the size implied by the current annotation scale. Each object holds one representation per supported scale, each with its own position. | T1 | 1 |
| DB-PLOT-014 | Current annotation scale | The current annotation scale applies to model space and to the active viewport; only objects that support it are shown unless the show-all-scales setting is on. | T1 | 1 |
| DB-PLOT-015 | Add and remove object scales (`OBJECTSCALE`) | Adds or removes scales on selected annotative objects; an object always keeps at least one scale. A setting adds the current scale automatically to all annotative objects when the current scale changes. | T1 | 1 |
| DB-PLOT-016 | Reset scale positions (`ANNORESET`, `ANNOUPDATE`) | Resets the position of every scale representation of the selected objects to the current representation, and updates annotative objects to their style's current properties. | T2 | 1 |
| DB-PLOT-017 | Scale list editing (`SCALELISTEDIT`) | Adds, edits, reorders and deletes named scales in paper-to-drawing units; reset replaces the list with the defaults for the drawing's measurement system and removes duplicate and unused entries, reporting what was removed. | T1 | 1 |
| DB-PLOT-018 | Scale-list hygiene on insert | When blocks or references bring duplicate scale names into a drawing, matching scales are merged by ratio instead of added, so the list does not grow. | T3 | 1 |

## Page setups and devices

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-PLOT-019 | Page setups (`PAGESETUP`) | Named page setups store device, paper size, plot area (display, extents, limits or layout, named view, window), offset or centering, scale or fit, lineweight scaling, plot style table, shaded viewport quality and resolution, orientation, and options for lineweights, transparency, plot styles, paper space last, hiding paper-space objects and plot stamp. A setup is set current for a layout or as the default for new layouts. | T1 | 1 |
| DB-PLOT-020 | Import page setups | Page setups can be imported from another drawing or template; name collisions are reported and resolved by rename or replace. | T1 | 1 |
| DB-PLOT-021 | Output devices | Output targets include system printers on each platform, a PDF writer, an SVG writer and PNG, JPEG and TIFF raster writers. Each device definition has paper sizes, printable margins and resolution and is stored as a documented settings file. | T1 | 1 |
| DB-PLOT-022 | Custom paper sizes | Custom paper sizes with margins can be added to any device, including very long roll sizes; device paper lists are filtered to sizes the device supports. | T1 | 1 |
| DB-PLOT-023 | Device settings import | Existing PC3 device settings files are read where their content is documented, mapping paper sizes, margins and resolution onto a DraftBuilder device definition with a report of unsupported settings. | T2 | 2 |

## Plot styles

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-PLOT-024 | Color-dependent and named plot styles | A drawing uses either color-dependent styles (one entry per color index, CTB) or named styles (assignable per layer and object, STB). Both table formats are read and written. | T1 | 1 |
| DB-PLOT-025 | Plot style table editor | Edits output color, dithering, grayscale, pen number, virtual pen, screening, linetype, adaptive linetype scaling, lineweight, line end, line join and fill style per entry; changes are previewed and saved to the table file. | T1 | 1 |
| DB-PLOT-026 | Plot style mode conversion (`CONVERTCTB`, `CONVERTPSTYLES`) | Converts a color table into an equivalent named table and converts a drawing between color-dependent and named modes, assigning named styles to layers by their color. | T2 | 2 |

## Plotting

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-PLOT-027 | Plot (`PLOT`, `-PLOT`) | Plots the current layout or model view with a chosen page setup or ad-hoc settings, optionally saving them back to the layout; the hyphen form asks for every setting at the command line. Output to a file is available for every device. | T1 | 1 |
| DB-PLOT-028 | Plot preview (`PREVIEW`) | Shows the exact output, including plot styles, lineweights, transparency and paper margins, with zoom and pan, and plots from the preview. | T1 | 1 |
| DB-PLOT-029 | Background plotting | Plots and publishes run on worker threads while editing continues; a status indicator shows progress and completion, and errors are reported per sheet. Output always reflects the document revision at the time the plot started. | T1 | 1 |
| DB-PLOT-030 | Plot stamp (`PLOTSTAMP`) | Adds a configurable stamp with drawing name, layout, date and time, device, paper, scale and user-defined fields at a chosen corner and offset. | T2 | 1 |
| DB-PLOT-031 | Plot log | Every plot and publish appends a record (drawing, layout, device, paper, time, result) to a plot log file in CSV form. | T2 | 1 |
| DB-PLOT-032 | PDF output | PDF output keeps vector geometry, TrueType and SHX text as searchable text or geometry by setting, layers as optional content groups, hyperlinks, bookmarks per sheet and named views, and embedded fonts; vector resolution and raster resolution are settings. Output is written through `arc-io-pdf`. | T1 | 1 |
| DB-PLOT-033 | Raster and SVG output | Layouts and views export to PNG, JPEG and TIFF at a chosen resolution and color depth, and to SVG with layers as groups. | T1 | 1 |
| DB-PLOT-034 | Lineweight and transparency output | Object and layer lineweights plot at their physical width, scaled with the plot scale when lineweight scaling is on; transparency plots when enabled for the setup. | T1 | 1 |

## Publishing

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-PLOT-035 | Publish (`PUBLISH`, `-PUBLISH`) | Publishes an ordered list of layouts from any number of drawings to their own devices, to one multi-sheet PDF or to one file per sheet, with per-sheet page setup override, reverse order, plot stamp and output folder and naming rules. The list saves and loads as a documented file. | T2 | 2 |
| DB-PLOT-036 | Sheet list import | Existing DSD sheet list files import into a publish list, mapping drawing paths and layouts with a report of entries that could not be resolved. | T2 | 2 |
| DB-PLOT-037 | Headless batch plotting | The command-line application plots and publishes drawings, folders or sheet sets without a display, using the same page setups and producing the same output as the desktop application, with an exit code and a machine-readable report. | T2 | 1 |
| DB-PLOT-038 | DWF output | Layouts export to DWF and DWFx two-dimensional files where an implementation from a published specification is available. | T3 | 3 |

## Sheet sets

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-PLOT-039 | Sheet set file | A sheet set is a documented file listing sheets (each a layout in a drawing), nested subsets, sheet views, custom properties, resource drawing locations, template and storage locations per subset, and page setup overrides. Paths are stored relative to the sheet set file where possible. | T2 | 2 |
| DB-PLOT-040 | Sheet set panel | A dockable panel shows the sheet list, sheet views and resource drawings; it creates, opens, renames, renumbers, moves and removes sheets and subsets and shows which sheets are open or locked by another user. | T2 | 2 |
| DB-PLOT-041 | Create sheet sets | A new sheet set is created empty or from existing drawings, importing their layouts as sheets; new sheets are created from the subset's template with number and title. | T2 | 2 |
| DB-PLOT-042 | Sheet set properties | Sheet set, subset and sheet custom properties (project number, issue date and user-defined) are editable singly or for several sheets at once and are available to fields in title blocks; changes update fields on next regeneration of each sheet. | T2 | 2 |
| DB-PLOT-043 | Sheet views, callouts and labels | Placing a model view on a sheet creates a numbered sheet view; label and callout blocks reference the view and display its number and sheet through fields, updating when sheets are renumbered. | T2 | 2 |
| DB-PLOT-044 | Sheet list table | Inserts a table listing the sheets with chosen columns from sheet properties; the table updates on command or when the sheet set changes and can link to sheets as hyperlinks. | T2 | 2 |
| DB-PLOT-045 | Publish and package a sheet set | A sheet set or subset publishes to PDF or devices in one step and packages into a transmittal package with all sheets, references and resources. | T2 | 2 |

# DraftBuilder reference workflows

These end-to-end workflows are the acceptance scenarios for the roadmap. Each is a sequence of user
steps in DraftBuilder's vocabulary, followed by observable acceptance results with the capabilities
they exercise. A workflow passes when every acceptance bullet holds on Windows, macOS and Linux, both
interactively and, where marked, as a headless script.

## W01 Draft a part from a template with precision tools

1. Create a new drawing from the team's metric template ([DB-DBASE-025](capabilities/01-drawing-database.md)).
2. Turn on endpoint, midpoint, intersection, center and perpendicular in the running snap set, polar
   tracking at 45 degrees and the grid ([DB-SNAP-008](capabilities/03-precision.md),
   [DB-SNAP-014](capabilities/03-precision.md), [DB-SNAP-016](capabilities/03-precision.md)).
3. Draw the outline with `PL`, typing relative polar coordinates and direct distances
   ([DB-DRAW-004](capabilities/05-draw-2d.md), [DB-SNAP-019](capabilities/03-precision.md),
   [DB-SNAP-022](capabilities/03-precision.md)).
4. Add holes with `C` using snap-from offsets, then array them with `ARRAYRECT`
   ([DB-DRAW-009](capabilities/05-draw-2d.md), [DB-SNAP-010](capabilities/03-precision.md),
   [DB-MOD-022](capabilities/06-modify-2d.md)).
5. Round corners with `F`, offset the outline with `O`, trim with `TR`, then undo two steps and redo
   one ([DB-MOD-015](capabilities/06-modify-2d.md), [DB-MOD-018](capabilities/06-modify-2d.md),
   [DB-MOD-010](capabilities/06-modify-2d.md), [DB-DBASE-037](capabilities/01-drawing-database.md)).
6. Save as DXF and reopen ([DB-DBASE-028](capabilities/01-drawing-database.md),
   [DB-INTEROP-003](capabilities/19-interoperability-standards.md)).

**Acceptance**

- Every typed coordinate and distance produces geometry exact to the drawing's precision; snapped
  points coincide with the snapped geometry.
- The same session replayed as a token script produces an identical database.
- Undo and redo restore exactly the intermediate states.
- The reopened DXF contains the same entities, layers and handles.

## W02 Floor plan with walls, openings, furniture and annotation

1. Draw grid lines with `XLINE` and wall center lines with `PLINE` on layer A-WALL
   ([DB-DRAW-002](capabilities/05-draw-2d.md), [DB-LAYER-006](capabilities/07-layers-properties.md)).
2. Offset both sides, clean corners with zero-radius fillets and cut openings with `TRIM`
   ([DB-MOD-018](capabilities/06-modify-2d.md), [DB-MOD-015](capabilities/06-modify-2d.md),
   [DB-MOD-010](capabilities/06-modify-2d.md)).
3. Insert door and furniture blocks from the block panel with tracking from wall midpoints
   ([DB-BLOCK-002](capabilities/11-blocks-attributes.md), [DB-SNAP-013](capabilities/03-precision.md)).
4. Hatch the floors by picking inside rooms; add room names as paragraph text
   ([DB-HATCH-001](capabilities/10-hatch-fill.md), [DB-HATCH-002](capabilities/10-hatch-fill.md),
   [DB-TEXT-003](capabilities/08-text-tables-fields.md)).
5. Dimension with `DIM`, `DIMCONTINUE` and `DIMSPACE` using an annotative style
   ([DB-DIM-001](capabilities/09-dimensions-leaders.md), [DB-DIM-007](capabilities/09-dimensions-leaders.md),
   [DB-DIM-011](capabilities/09-dimensions-leaders.md), [DB-PLOT-013](capabilities/14-layouts-plotting.md)).
6. Stretch one room by 600 mm ([DB-MOD-006](capabilities/06-modify-2d.md)) and count doors with
   `COUNT` ([DB-BLOCK-032](capabilities/11-blocks-attributes.md)).

**Acceptance**

- After the stretch, associative dimensions show the new values and associative hatches fill the new
  room outline without a manual command ([DB-DIM-018](capabilities/09-dimensions-leaders.md),
  [DB-HATCH-016](capabilities/10-hatch-fill.md)).
- The door count equals the number of door references, and selecting the count entry selects them.
- Freezing A-WALL hides the walls in every view and excludes them from selection.

## W03 Block with attributes and dynamic behavior

1. Draw a door symbol on layer 0 and define attributes for tag, width and fire rating
   ([DB-BLOCK-012](capabilities/11-blocks-attributes.md)).
2. Define the block, insert it twice and answer the attribute prompts
   ([DB-BLOCK-001](capabilities/11-blocks-attributes.md), [DB-BLOCK-013](capabilities/11-blocks-attributes.md)).
3. Open block authoring mode, add a linear parameter with a stretch action, a flip parameter and two
   visibility states, test and save ([DB-BLOCK-021](capabilities/11-blocks-attributes.md) to
   [DB-BLOCK-025](capabilities/11-blocks-attributes.md), [DB-BLOCK-028](capabilities/11-blocks-attributes.md)).
4. Change the width of one reference with its custom grip and switch its visibility state
   ([DB-BLOCK-030](capabilities/11-blocks-attributes.md), [DB-SEL-029](capabilities/04-selection-grips.md)).
5. Add an attribute to the definition and synchronize references
   ([DB-BLOCK-017](capabilities/11-blocks-attributes.md)).

**Acceptance**

- Each reference keeps its own attribute values after synchronization and gains the new attribute
  with its default.
- References inherit the layer they are inserted on through layer 0
  ([DB-BLOCK-008](capabilities/11-blocks-attributes.md)).
- Saved as DWG and opened by an independent reader, the modified reference shows the stretched and
  flipped geometry ([DB-BLOCK-029](capabilities/11-blocks-attributes.md)).

## W04 Layout, viewports, annotation scaling and PDF

1. Switch to a layout, apply a page setup for A1 PDF output with a color-dependent plot style table
   ([DB-PLOT-001](capabilities/14-layouts-plotting.md), [DB-PLOT-019](capabilities/14-layouts-plotting.md),
   [DB-PLOT-024](capabilities/14-layouts-plotting.md)).
2. Insert the title block, create two viewports at 1:100 and 1:20 and lock them
   ([DB-PLOT-005](capabilities/14-layouts-plotting.md) to [DB-PLOT-007](capabilities/14-layouts-plotting.md)).
3. Freeze the furniture layer in the 1:20 viewport and gray an underlay layer with a viewport
   override ([DB-LAYER-022](capabilities/07-layers-properties.md),
   [DB-LAYER-023](capabilities/07-layers-properties.md)).
4. Add the 1:20 scale to annotative dimensions and text ([DB-PLOT-015](capabilities/14-layouts-plotting.md)).
5. Preview and plot to PDF in the background while continuing to edit
   ([DB-PLOT-027](capabilities/14-layouts-plotting.md) to [DB-PLOT-029](capabilities/14-layouts-plotting.md),
   [DB-PLOT-032](capabilities/14-layouts-plotting.md)).

**Acceptance**

- Annotative text and dimensions have the same paper height in both viewports.
- The PDF has the page size, lineweights and screening of the plot style table, with one PDF layer
  per drawing layer, and matches the preview image within the plot comparison tolerance.
- The same layout plotted headless through the CLI yields the same PDF content
  ([DB-PLOT-037](capabilities/14-layouts-plotting.md)).

## W05 Open a partner's drawing, edit it and give it back

1. Open a DWG produced by another application that contains custom objects and extended data
   ([DB-INTEROP-001](capabilities/19-interoperability-standards.md), [DB-DBASE-026](capabilities/01-drawing-database.md)).
2. Read the preservation report ([DB-DBASE-019](capabilities/01-drawing-database.md)).
3. Move the whole plan, including custom objects, and edit ordinary entities
   ([DB-DBASE-017](capabilities/01-drawing-database.md), [DB-MOD-001](capabilities/06-modify-2d.md)).
4. Save as DWG in the drawing's version and also save down to an older version
   ([DB-INTEROP-002](capabilities/19-interoperability-standards.md), [DB-DBASE-029](capabilities/01-drawing-database.md)).

**Acceptance**

- Custom objects display and plot through their proxy graphics and appear in the report with class
  name and count ([DB-DBASE-018](capabilities/01-drawing-database.md)).
- In the saved file, every object not edited has the same handle and field values, and extended data
  is byte-identical; moved custom objects carry the transform.
- The save-down loss report lists each converted object, and an independent reader opens both files
  ([DB-INTEROP-004](capabilities/19-interoperability-standards.md),
  [DB-INTEROP-005](capabilities/19-interoperability-standards.md)).
- Stable IDs of unedited objects are the same before and after the round trip
  ([DB-DBASE-016](capabilities/01-drawing-database.md)).

## W06 Coordinate with external references and underlays

1. Attach the consultant's drawing as an overlay at the origin with a relative path
   ([DB-XREF-001](capabilities/12-references-underlays.md), [DB-XREF-002](capabilities/12-references-underlays.md)).
2. Clip it to the working area and fade it ([DB-XREF-007](capabilities/12-references-underlays.md),
   [DB-XREF-011](capabilities/12-references-underlays.md)).
3. Attach a scanned site plan image and a PDF underlay, and snap to the PDF geometry
   ([DB-XREF-017](capabilities/12-references-underlays.md), [DB-XREF-023](capabilities/12-references-underlays.md),
   [DB-XREF-026](capabilities/12-references-underlays.md)).
4. Move the project folder, reopen, and repair missing references from the references panel
   ([DB-XREF-012](capabilities/12-references-underlays.md), [DB-XREF-014](capabilities/12-references-underlays.md)).
5. Receive an updated consultant drawing and reload it ([DB-XREF-009](capabilities/12-references-underlays.md)).

**Acceptance**

- The references panel shows each reference's status and saved and found paths; after repair all are
  loaded and paths stay relative.
- The overlay does not appear when this drawing is itself referenced by another drawing
  ([DB-XREF-004](capabilities/12-references-underlays.md)).
- The changed-reference notification appears without restarting, and reload shows the new content.
- The headless reference report lists the same statuses for the folder
  ([DB-XREF-016](capabilities/12-references-underlays.md)).

## W07 Data-linked schedule and data extraction

1. Create a table style and a table linked to a named range of an XLSX file
   ([DB-TEXT-032](capabilities/08-text-tables-fields.md), [DB-TEXT-034](capabilities/08-text-tables-fields.md)).
2. Change the spreadsheet and update the link ([DB-TEXT-035](capabilities/08-text-tables-fields.md)).
3. Extract door block attributes from the plan and two other drawings into a table and a CSV file
   ([DB-TEXT-036](capabilities/08-text-tables-fields.md), [DB-TEXT-037](capabilities/08-text-tables-fields.md)).
4. Add a field showing a room polyline's area in a label
   ([DB-TEXT-022](capabilities/08-text-tables-fields.md), [DB-TEXT-023](capabilities/08-text-tables-fields.md)).

**Acceptance**

- Linked cells show the new spreadsheet values and keep their table formatting; no spreadsheet
  application is needed.
- The extraction table combines identical rows with counts, and refreshing it after editing an
  attribute updates the row.
- Stretching the room updates the area field at the next evaluation trigger
  ([DB-TEXT-024](capabilities/08-text-tables-fields.md)).

## W08 Publish a sheet set

1. Create a sheet set from existing drawings, import their layouts as sheets and set project
   properties ([DB-PLOT-039](capabilities/14-layouts-plotting.md) to
   [DB-PLOT-042](capabilities/14-layouts-plotting.md)).
2. Place a sheet list table and let title-block fields show sheet numbers
   ([DB-PLOT-044](capabilities/14-layouts-plotting.md), [DB-TEXT-022](capabilities/08-text-tables-fields.md)).
3. Renumber two sheets and publish the set to one multi-sheet PDF in the background
   ([DB-PLOT-035](capabilities/14-layouts-plotting.md)).
4. Run the same publication from the CLI on a build server and package the set
   ([DB-PLOT-037](capabilities/14-layouts-plotting.md), [DB-PLOT-045](capabilities/14-layouts-plotting.md),
   [DB-INTEROP-030](capabilities/19-interoperability-standards.md)).

**Acceptance**

- The sheet list, title blocks and PDF bookmarks all show the new numbers.
- Interactive and headless PDFs have the same pages in the same order with the same content.
- The package opens on another machine with every reference, font and plot style resolved, and its
  report lists every included file.

## W09 Bring existing customization: aliases, scripts and Lisp routines

1. Import the team's alias file and confirm a collision report
   ([DB-CMD-009](capabilities/02-command-interface.md), [DB-CMD-010](capabilities/02-command-interface.md)).
2. Run a script that uses hyphen-prefixed commands to set layers and plot
   ([DB-AUTO-011](capabilities/18-customization-automation.md), [DB-CMD-013](capabilities/02-command-interface.md)).
3. Load a Lisp routine from a trusted folder that defines a command, prompts for points and keyword
   options, edits entities through group-code lists and shows a dialog definition
   ([DB-AUTO-010](capabilities/18-customization-automation.md), [DB-AUTO-015](capabilities/18-customization-automation.md)
   to [DB-AUTO-019](capabilities/18-customization-automation.md), [DB-AUTO-024](capabilities/18-customization-automation.md)).
4. Run the routine's command from its alias, undo it, then stop a second run midway.
5. Record a macro of a command sequence with a pause for input and bind it to a button
   ([DB-AUTO-013](capabilities/18-customization-automation.md), [DB-AUTO-005](capabilities/18-customization-automation.md)).

**Acceptance**

- The routine's prompts appear in the command line and cursor input exactly as built-in prompts.
- One undo reverses the whole routine command; stopping it leaves the drawing as before the run.
- A routine in an untrusted folder does not load without the user's confirmation.
- Settings read and written by the routine through widely used setting names change the mapped
  DraftBuilder settings ([DB-CMD-043](capabilities/02-command-interface.md)).

## W10 Clean up and standardize a legacy drawing

1. Recover a damaged drawing and audit it ([DB-DBASE-034](capabilities/01-drawing-database.md),
   [DB-DBASE-033](capabilities/01-drawing-database.md)).
2. Remove duplicates, flatten stray elevations and run geometry cleanup
   ([DB-MOD-030](capabilities/06-modify-2d.md), [DB-MOD-031](capabilities/06-modify-2d.md),
   [DB-MOD-033](capabilities/06-modify-2d.md)).
3. Associate the office standards file, check standards, map layers to the standard and import
   missing styles ([DB-INTEROP-024](capabilities/19-interoperability-standards.md),
   [DB-INTEROP-025](capabilities/19-interoperability-standards.md),
   [DB-INTEROP-028](capabilities/19-interoperability-standards.md),
   [DB-INTEROP-029](capabilities/19-interoperability-standards.md)).
4. Reset the scale list and purge nested unused content
   ([DB-PLOT-017](capabilities/14-layouts-plotting.md), [DB-DBASE-035](capabilities/01-drawing-database.md)).
5. Run the same standards check headless over the project folder
   ([DB-INTEROP-027](capabilities/19-interoperability-standards.md)).

**Acceptance**

- The audit and recover reports list each repaired object; repairs undo as one step.
- After cleanup, a second standards check reports no violations, and the headless check returns a
  machine-readable report with a non-zero exit status for drawings that still violate.
- Purge removes nothing that is referenced, including by extended data handles.

## W11 Compare revisions and review through Arc Hub

1. Open a drawing from Arc Hub and check it out ([DB-COLLAB-018](capabilities/17-compare-history-collaboration.md),
   [DB-COLLAB-019](capabilities/17-compare-history-collaboration.md)).
2. Compare it with the previous version ([DB-COLLAB-001](capabilities/17-compare-history-collaboration.md)
   to [DB-COLLAB-004](capabilities/17-compare-history-collaboration.md)).
3. Import a reviewer's PDF markups as an overlay, resolve them, and draw revision clouds with notes
   ([DB-COLLAB-013](capabilities/17-compare-history-collaboration.md),
   [DB-COLLAB-014](capabilities/17-compare-history-collaboration.md),
   [DB-COLLAB-016](capabilities/17-compare-history-collaboration.md)).
4. Inspect the history of one door and restore its earlier state
   ([DB-COLLAB-009](capabilities/17-compare-history-collaboration.md), [DB-DBASE-040](capabilities/01-drawing-database.md)).
5. Check in a new version for review and export the differences as revision clouds
   ([DB-COLLAB-020](capabilities/17-compare-history-collaboration.md),
   [DB-COLLAB-005](capabilities/17-compare-history-collaboration.md)).

**Acceptance**

- Compare lists added, removed and modified objects with the changed properties, and navigating a
  difference zooms to it.
- The markup overlay never alters drawing geometry and travels with the drawing version in Hub.
- The restored door is a new undoable transaction, and its history shows every earlier state.
- With the network disconnected, the drawing still opens, edits and saves locally
  ([DB-COLLAB-025](capabilities/17-compare-history-collaboration.md)).

## W12 3D solid model and drawing views

1. Extrude a closed polyline, subtract a revolved cut, fillet edges and shell the body
   ([DB-M3D-003](capabilities/15-3d-modeling-visualization.md), [DB-M3D-004](capabilities/15-3d-modeling-visualization.md),
   [DB-M3D-009](capabilities/15-3d-modeling-visualization.md), [DB-M3D-013](capabilities/15-3d-modeling-visualization.md),
   [DB-M3D-014](capabilities/15-3d-modeling-visualization.md)).
2. Draw on a face using a UCS aligned to it and press-pull a boss
   ([DB-SNAP-027](capabilities/03-precision.md), [DB-M3D-007](capabilities/15-3d-modeling-visualization.md)).
3. Create a section plane and a live section ([DB-M3D-030](capabilities/15-3d-modeling-visualization.md)).
4. On a layout, create base, projected, section and detail views
   ([DB-M3D-033](capabilities/15-3d-modeling-visualization.md)).
5. Change the extrusion height, update the views, check the solid and export STL
   ([DB-M3D-034](capabilities/15-3d-modeling-visualization.md), [DB-M3D-019](capabilities/15-3d-modeling-visualization.md),
   [DB-M3D-051](capabilities/15-3d-modeling-visualization.md)).

**Acceptance**

- The views update to the new height with hidden lines and section hatching, and their dimensions
  update.
- Mass properties report volume and centroid consistent with the geometry.
- The STL is a closed shell in the declared units and is read by an independent mesh reader.

## W13 Point cloud to georeferenced as-built plan

1. Assign the drawing a coordinate reference system and reference point
   ([DB-REAL-012](capabilities/16-point-clouds-geolocation.md)).
2. Attach an E57 scan, which indexes in the background, and crop it to one floor
   ([DB-REAL-001](capabilities/16-point-clouds-geolocation.md), [DB-REAL-002](capabilities/16-point-clouds-geolocation.md),
   [DB-REAL-006](capabilities/16-point-clouds-geolocation.md)).
3. Set a UCS from a fitted floor plane and slice the cloud at 1.2 m
   ([DB-REAL-011](capabilities/16-point-clouds-geolocation.md), [DB-REAL-008](capabilities/16-point-clouds-geolocation.md)).
4. Trace walls with polylines using point cloud snaps ([DB-REAL-010](capabilities/16-point-clouds-geolocation.md)).
5. Show a base map from a configured tile source and export the outline location to KML
   ([DB-REAL-016](capabilities/16-point-clouds-geolocation.md), [DB-INTEROP-017](capabilities/19-interoperability-standards.md)).

**Acceptance**

- Navigation stays interactive while the cloud indexes, and the point budget is respected.
- Traced geometry snaps to cloud edges and corners and keeps national-grid coordinates without
  jitter ([DB-DBASE-023](capabilities/01-drawing-database.md)).
- The KML places the outline at the correct latitude and longitude when opened by an independent
  GIS reader.

## W14 Several drafters edit one drawing

1. Two users open the same drawing from Arc Hub in a multi-user session
   ([DB-COLLAB-026](capabilities/17-compare-history-collaboration.md)).
2. User A borrows the east wing region; user B borrows a set of title-block attributes
   ([DB-COLLAB-027](capabilities/17-compare-history-collaboration.md)).
3. Both edit and save; each sees the other's changes arrive ([DB-COLLAB-028](capabilities/17-compare-history-collaboration.md)).
4. User B, offline, edits an object user A also changed, then reconnects
   ([DB-COLLAB-029](capabilities/17-compare-history-collaboration.md)).

**Acceptance**

- Neither user can modify an object borrowed by the other; the reason and owner are shown.
- Changes appear in the other session without reopening the file, and undo affects only one's own
  transactions.
- The conflict keeps both versions side by side for comparison; no work is discarded.
- The drawing saved from the session opens in an independent reader with all accepted changes.

## W15 Review and edit a drawing in the browser and on a tablet

1. A reviewer opens a Hub share link in a browser, pans, switches layouts and toggles layers
   ([DB-WEB-006](capabilities/28-web-and-mobile.md), [DB-WEB-008](capabilities/28-web-and-mobile.md)).
2. The reviewer measures a wall and marks up a detail with a pen
   ([DB-WEB-009](capabilities/28-web-and-mobile.md), [DB-WEB-010](capabilities/28-web-and-mobile.md),
   [DB-WEB-016](capabilities/28-web-and-mobile.md)).
3. On a tablet with no network, a drafter opens an offline drawing, moves a door and adds a note by
   touch with the loupe ([DB-WEB-015](capabilities/28-web-and-mobile.md),
   [DB-WEB-023](capabilities/28-web-and-mobile.md), [DB-WEB-027](capabilities/28-web-and-mobile.md)).
4. The tablet reconnects; the edit and markup synchronize while a desktop user edited another area
   ([DB-WEB-028](capabilities/28-web-and-mobile.md), [DB-WEB-029](capabilities/28-web-and-mobile.md)).
5. A command unavailable at the tablet's stage is searched for and reported with its reason
   ([DB-WEB-004](capabilities/28-web-and-mobile.md)).

**Acceptance**

- The markup appears as overlay items in the desktop application and does not change geometry.
- Offline edits survive closing the application and are applied as undoable transactions in the
  drawing's journal; a conflicting edit keeps both versions.
- The drawing saved after synchronization opens in an independent reader with all accepted changes.
- Plotting the layout in the browser matches desktop output within the plot comparison tolerance.

## W16 Windows integration, open readers and video

1. On Windows, a drafter inserts a document of another application as an embedded object, edits it
   in place and plots the sheet ([DB-XREF-031](capabilities/12-references-underlays.md),
   [DB-XREF-033](capabilities/12-references-underlays.md)).
2. A script written for the conventional automation object model is checked for coverage and run
   through the automation bridge ([DB-AUTO-038](capabilities/18-customization-automation.md),
   [DB-AUTO-040](capabilities/18-customization-automation.md)).
3. The drafter attaches a DGN V7 file as an underlay and a 3DM file as a 3D reference
   ([DB-XREF-028](capabilities/12-references-underlays.md), [DB-XREF-034](capabilities/12-references-underlays.md)).
4. The drafter records a walkthrough and writes it as video through RenderBuilder
   ([DB-M3D-054](capabilities/15-3d-modeling-visualization.md), [DB-M3D-052](capabilities/15-3d-modeling-visualization.md)).

**Acceptance**

- The embedded object opens without a prompt only in a trusted location, never in a headless run,
  and the saved drawing shows its cached image in an independent reader.
- Every script call runs as one undoable transaction; unmapped members are listed before the run and
  fail with a named error.
- The loss report lists the DGN and 3DM elements that have no counterpart.
- The video file plays in an independent player at the requested resolution and frame rate, and a
  repeated run produces the same frames.

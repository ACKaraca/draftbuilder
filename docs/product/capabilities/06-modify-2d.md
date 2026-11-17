# 2D modification

Modify commands transform, trim, join, offset, replicate and restructure existing objects. Every
command works on pre-selected or prompted objects, previews its result, keeps associative
dependents up to date in the same transaction, and has a non-interactive form.

## Domain objects

- **Associative array** — an array object that stores its source items, pattern parameters (rectangular, polar or path) and per-item overrides.
- **Cutting and boundary edges** — the objects that limit trim and extend.
- **Clipboard content** — copied objects with a base point and their dependent definitions.
- **Cleanup finding** — a detected geometric defect with location, type and proposed repair.

## Transform

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-MOD-001 | Move (`MOVE`) | Moves objects by a base point and second point or by a typed displacement; direct distance entry along ortho, polar or tracking paths gives exact moves. | T1 | 1 |
| DB-MOD-002 | Copy (`COPY`) | Copies objects by base point and repeated second points in multiple or single mode, by displacement, or as an array of a set count either spaced or fitted between the base and second points. | T1 | 1 |
| DB-MOD-003 | Rotate (`ROTATE`) | Rotates objects about a base point by an angle or by reference (current and new angle from typed values or picks), optionally keeping the original as a copy. | T1 | 1 |
| DB-MOD-004 | Scale (`SCALE`) | Scales objects uniformly about a base point by a factor or by reference length, optionally as a copy. | T1 | 1 |
| DB-MOD-005 | Mirror (`MIRROR`) | Mirrors objects about a line through two points, keeping or erasing the source; a setting decides whether text and attributes are mirrored or keep their reading direction. | T1 | 1 |
| DB-MOD-006 | Stretch (`STRETCH`) | Moves the vertices and definition points that lie inside a crossing window or polygon and stretches the connected geometry; works on lines, arcs, polylines, splines, associative hatches, dimensions and dynamic block references, and moves wholly enclosed objects. | T1 | 1 |
| DB-MOD-007 | Align (`ALIGN`) | Aligns objects using one, two or three source and destination point pairs, optionally scaling to the destination distance, in 2D and 3D. | T2 | 1 |
| DB-MOD-008 | Reference input | Rotate, scale and similar commands accept reference values as typed numbers or as picked points, deriving the resulting angle or factor. | T1 | 1 |
| DB-MOD-009 | Drag-and-drop editing | Dragging a selection moves it; dragging with a modifier copies it; dropping onto another open drawing copies the objects with their dependent definitions. | T2 | 1 |

## Trim, extend and length

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-MOD-010 | Trim (`TRIM`) | Removes the parts of objects bounded by cutting edges; quick mode treats all visible objects as edges and trims segments picked or crossed by a fence or crossing, standard mode uses selected edges. Options control projection in 3D, extension of edges to their natural continuation, erasing objects, and undo. | T1 | 1 |
| DB-MOD-011 | Extend (`EXTEND`) | Extends objects to boundary edges with the same modes and options as trim; holding Shift during either command switches to the other. | T1 | 1 |
| DB-MOD-012 | Lengthen (`LENGTHEN`) | Changes the length of open lines, arcs, polylines, splines and elliptical arcs by a delta, a percentage, a total length or angle, or dynamically by dragging, from the end nearest the pick. | T1 | 1 |
| DB-MOD-013 | Break (`BREAK`) and break at point | Removes the part of an object between two points, or splits it at one point leaving two objects with identical properties; closed objects break counterclockwise from the first point. | T1 | 1 |
| DB-MOD-014 | Join (`JOIN`) | Joins collinear lines, coaxial arcs, and end-to-end lines, arcs, polylines, splines and helixes into one object of the simplest type that represents them, and closes arcs and elliptical arcs into circles and ellipses. Objects that cannot join are reported. | T1 | 1 |

## Corners and curves

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-MOD-015 | Fillet (`FILLET`) | Rounds the corner between two objects with a radius, trimming or extending them unless no-trim is chosen; a whole polyline is filleted at every vertex, multiple mode repeats, and radius zero or Shift-pick makes a sharp corner. Parallel lines receive a semicircular end. | T1 | 1 |
| DB-MOD-016 | Chamfer (`CHAMFER`) | Bevels the corner between two lines or polyline segments by two distances or a distance and angle, with polyline, multiple, trim and method options; zero distances or Shift-pick make a sharp corner. | T1 | 1 |
| DB-MOD-017 | Blend (`BLEND`) | Creates a spline connecting the ends of two open curves with tangent or curvature-continuous transition. | T2 | 1 |
| DB-MOD-018 | Offset (`OFFSET`) | Creates parallel copies at a distance or through a point, on the source layer or the current layer, optionally erasing the source and offsetting repeatedly from the last result. Polyline corners close by extension, fillet arcs or chamfers according to a setting, and the last distance is remembered. | T1 | 1 |
| DB-MOD-019 | Polyline edit (`PEDIT`) | Opens or closes polylines, joins touching lines, arcs and polylines into one (with a fuzz distance and extend, add or both joining), sets uniform width, edits vertices (insert, move, straighten, tangent, width, break), fits curves, spline-fits with quadratic or cubic order, removes fitting, toggles continuous linetype generation and reverses direction; multiple polylines can be edited at once and lines can be converted on request. | T1 | 1 |
| DB-MOD-020 | Spline edit (`SPLINEDIT`) | Edits fit data (add, delete, move fit points, tangents, tolerance), control vertices (move, add, delete, weights, degree elevation, refinement), opens or closes, reverses and converts splines to polylines within a precision. | T1 | 1 |
| DB-MOD-021 | Reverse direction | Reverses the vertex order of lines, polylines, splines and helixes so linetypes with text and direction-dependent symbols read the other way. | T2 | 1 |

## Arrays

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-MOD-022 | Rectangular array (`ARRAYRECT`) | Replicates objects in rows, columns and levels with spacing, angle and incremental elevation; the result is associative by default and can be created as independent objects. | T1 | 1 |
| DB-MOD-023 | Polar array (`ARRAYPOLAR`) | Replicates objects about a center by item count, angle between items and fill angle, with or without rotating items, in rows and levels. | T1 | 1 |
| DB-MOD-024 | Path array (`ARRAYPATH`) | Replicates objects along a curve by divide or measure spacing, aligned to the path tangent or not, with a base point and Z direction; editing the path updates the array. | T2 | 1 |
| DB-MOD-025 | Array editing (`ARRAYEDIT`) | Changes an associative array's parameters, source objects, base point and pattern; replaces individual items with other objects; and resets item overrides. Grips edit spacing, counts and angles. | T1 | 1 |
| DB-MOD-026 | Array storage compatibility | Associative arrays are stored in the drawing so that applications without array support see the replicated items as a block reference. Explode turns an array into independent items. | T1 | 1 |

## Erase, explode and restructure

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-MOD-027 | Erase (`ERASE`) | Deletes selected objects; erased objects can be restored with undo or the restore-last-erased command. | T1 | 1 |
| DB-MOD-028 | Explode (`EXPLODE`) | Breaks block references, polylines, multilines, dimensions, hatches, regions, tables, arrays and multileaders into their component objects; components take the properties defined in the block or by-block inheritance. | T1 | 1 |
| DB-MOD-029 | Property-controlled explode | An explode variant sets the layer, color, linetype and lineweight of the resulting components, per object or for all, or makes them inherit from the parent reference. | T2 | 1 |
| DB-MOD-030 | Duplicate removal (`OVERKILL`) | Deletes duplicate objects and merges overlapping or collinear lines, arcs and polyline segments within a tolerance, with options to ignore chosen properties, break polylines and keep associativity; a report lists what changed. | T1 | 1 |
| DB-MOD-031 | Flatten | Projects selected 2D and 3D curves to the current UCS XY plane with zero elevation and thickness, optionally converting hidden lines of 3D objects. | T2 | 1 |
| DB-MOD-032 | Change (`CHANGE`) | Edits the endpoint of lines, the radius of circles, the insertion and content of text and the base point of blocks by a change point; property changes through the same command are DB-LAYER-038. | T2 | 1 |
| DB-MOD-033 | Geometry cleanup | Scans a selection for gaps below a tolerance, disjoint or overlapping segments, zero-length objects and segments off the nearest snap angle, marks each finding in the drawing and lists it; each repair is accepted individually or in bulk as one undoable transaction. | T3 | 2 |

## Clipboard and order

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-MOD-034 | Clipboard copy and paste | Cut, copy and paste move objects through the system clipboard with their dependent definitions; copy with a base point stores the point for exact placement at paste. Content from another drawing keeps its layers and styles unless names conflict, in which case the current drawing's definitions win and the conflict is reported. | T1 | 1 |
| DB-MOD-035 | Paste as block and to original coordinates | Pasted content can become a new anonymous-named block reference or be placed at the coordinates it had in the source drawing. | T1 | 1 |
| DB-MOD-036 | Draw order (`DRAWORDER`) | Moves objects to the front or back of their space or above or below reference objects; bulk commands bring all text, dimensions and leaders to the front or send all hatches to the back. Draw order changes are saved with the drawing. | T1 | 1 |

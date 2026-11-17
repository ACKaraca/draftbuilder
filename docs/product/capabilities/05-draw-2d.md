# 2D drawing

Drawing commands create the curves, construction lines, regions and masking objects that make up
2D drawings. Each command accepts every coordinate form, snap and tracking mode, has a
non-interactive form driven by its parameters, and creates its objects with the current
properties.

## Domain objects

- **Line**, **construction line** (infinite both ways) and **ray** (infinite one way).
- **Polyline** — lightweight 2D polyline with line and arc segments, per-vertex widths, elevation and closure; legacy 2D and 3D polylines.
- **Circle**, **arc**, **ellipse** and **elliptical arc**.
- **Spline** — NURBS curve defined by fit points or control vertices.
- **Point** — with a drawing-wide display style.
- **Helix**, **revision cloud** (a polyline with arc bulges), **wipeout** (masking polygon), **2D solid** and **trace**.
- **Multiline** and **multiline style** — parallel element lines with offsets, colors, linetypes and end caps.
- **Region** — a closed planar area with holes.

## Lines and polylines

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DRAW-001 | Line (`LINE`) | Draws a chain of independent line segments from successive points, with options to close the chain, undo the last segment and continue from the end of the last line or arc. Each segment is a separate object. | T1 | 1 |
| DB-DRAW-002 | Construction line (`XLINE`) | Draws infinite lines through a point by second point, horizontal, vertical, angle (absolute or from a reference line), bisector of an angle, or offset from a line; repeats until cancelled. Construction lines are excluded from extents. | T1 | 1 |
| DB-DRAW-003 | Ray (`RAY`) | Draws semi-infinite lines from one start point through successive points. | T2 | 1 |
| DB-DRAW-004 | Polyline (`PLINE`) | Draws one polyline object of line and arc segments, with start and end widths or half-widths per segment, length continuation along the previous direction, close and undo. Arc mode accepts angle, center, direction, radius, second point and close, and returns to line mode on request. | T1 | 1 |
| DB-DRAW-005 | Rectangle (`RECTANG`) | Draws a closed polyline rectangle by corners, area, dimensions or rotation, with optional chamfered or filleted corners, width, elevation and thickness. | T1 | 1 |
| DB-DRAW-006 | Polygon (`POLYGON`) | Draws a closed regular polyline of 3 to 1024 sides inscribed in or circumscribed about a circle, or by one edge. | T1 | 1 |
| DB-DRAW-007 | 3D polyline (`3DPOLY`) | Draws a polyline of straight segments through 3D points with close and undo. | T2 | 1 |
| DB-DRAW-008 | Legacy polylines | Legacy 2D polylines (including fit-curve and spline-fit forms) and polyface meshes read, display, edit and write back without conversion; a command converts legacy 2D polylines to lightweight polylines and back. | T1 | 1 |

## Circles, arcs and ellipses

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DRAW-009 | Circle (`CIRCLE`) | Draws a circle by center and radius or diameter, two points on a diameter, three points, tangent-tangent-radius or tangent to three objects. Tangent constructions solve against lines, arcs, circles and polyline segments and choose the solution nearest the picks. | T1 | 1 |
| DB-DRAW-010 | Arc (`ARC`) | Draws an arc by three points; by start, center and end, angle or chord length; by start, end and angle, direction or radius; by center, start and end, angle or length; or continuing tangent from the last line or arc. The positive angle direction follows the drawing's unit settings. | T1 | 1 |
| DB-DRAW-011 | Ellipse (`ELLIPSE`) | Draws an ellipse by axis endpoints or center, with the second axis by distance or rotation, and elliptical arcs by start and end angles or parameters. In isometric mode an option draws an isometric circle on the current plane. | T1 | 1 |
| DB-DRAW-012 | Donut (`DONUT`) | Draws filled or outlined rings from inner and outer diameters at repeated centers, stored as closed wide polylines; an inner diameter of zero produces a filled disc. | T2 | 1 |

## Curves and points

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DRAW-013 | Spline (`SPLINE`) | Draws a spline through fit points with chord, square-root or uniform knot parameterization, fit tolerance and end tangents, or by control vertices with a chosen degree. An option converts spline-fit polylines to true splines. | T1 | 1 |
| DB-DRAW-014 | Helix (`HELIX`) | Draws a 2D spiral or 3D helix from base and top radii, turns, height or turn height and twist direction; the helix stays editable by grips and properties and can be a sweep path. | T2 | 1 |
| DB-DRAW-015 | Point (`POINT`) | Places point objects; a drawing-wide point style sets their shape and size, either absolute or relative to the screen, and changing the style updates every point. | T1 | 1 |
| DB-DRAW-016 | Divide (`DIVIDE`) | Places points or a chosen block at equal segments along a curve, optionally aligning the blocks to the curve. | T1 | 1 |
| DB-DRAW-017 | Measure (`MEASURE`) | Places points or a chosen block at a fixed interval along a curve from the end nearest the pick, optionally aligned. | T1 | 1 |
| DB-DRAW-018 | Sketch (`SKETCH`) | Records free-hand pointer movement as connected line, polyline or spline segments of a set increment and tolerance. | T3 | 1 |

## Areas and masks

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DRAW-019 | Region (`REGION`) | Converts closed loops of curves into region objects, reporting loops that are open or self-intersecting; source curves are kept or removed according to a setting. | T1 | 1 |
| DB-DRAW-020 | Boundary (`BOUNDARY`) | Creates a closed polyline or region around the area enclosing a picked point, detecting islands inside it, using all visible objects or a chosen boundary set. Gaps up to a tolerance are bridged, and unclosed areas are reported with the gap marked. | T1 | 1 |
| DB-DRAW-021 | Wipeout (`WIPEOUT`) | Creates a masking polygon from points or a closed polyline that hides objects drawn below it in draw order; frames can be shown, hidden or shown but not plotted for all wipeouts. | T1 | 1 |
| DB-DRAW-022 | Revision cloud (`REVCLOUD`) | Draws a revision cloud as a rectangle, polygon, free-hand path or from an existing object, with minimum and maximum arc length and a normal or calligraphic style; the cloud is a polyline that can be reversed and edited by grips. | T1 | 1 |
| DB-DRAW-023 | 2D solid and trace | Draws filled triangles and quadrilaterals from three or four points, and reads, displays and writes legacy trace objects; fill display follows the drawing's fill setting. | T2 | 1 |
| DB-DRAW-024 | Fill display | A drawing setting shows or hides the fill of wide polylines, 2D solids, solid hatches and donuts in display and plots, for speed while editing. | T2 | 1 |

## Multilines and symbols

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DRAW-025 | Multiline (`MLINE`) | Draws parallel element lines along picked points with top, zero or bottom justification, a scale and a multiline style; close and undo are available. | T2 | 1 |
| DB-DRAW-026 | Multiline styles | Multiline styles define up to sixteen element lines with offset, color and linetype, start and end caps, fill and joint display; styles are stored in the drawing and can be loaded from style files. | T2 | 1 |
| DB-DRAW-027 | Multiline joint editing (`MLEDIT`) | Edits multiline intersections into closed, open or merged crosses and tees, corner joints, added or deleted vertices, and cut or welded segments. | T3 | 1 |
| DB-DRAW-028 | Break-line symbol | Draws a break-line polyline between two points with a configurable zig-zag symbol size and extension. | T3 | 2 |

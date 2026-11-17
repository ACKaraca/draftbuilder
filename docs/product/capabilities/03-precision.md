# Precision and coordinates

Precision tools turn approximate pointer input into exact geometry: object snaps, tracking,
constrained cursor modes, typed coordinates and user coordinate systems, plus the units and
measurement commands that report exact values back.

## Domain objects

- **Object snap mode** — a geometric rule that locates a point on an object (endpoint, center, tangent and others).
- **Running snap set** — the snap modes applied automatically at every point prompt.
- **Tracking point** — an acquired point from which alignment paths are offered.
- **Polar angle set** — the increment and additional angles used by polar tracking.
- **Grid and snap configuration** — spacing, angle, base point and style per viewport.
- **User coordinate system (UCS)** — a named origin and axis orientation used for input and display.
- **Unit format** — length and angle display and entry formats with precision.

## Object snaps

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-SNAP-001 | Endpoint, midpoint and center snaps | Snap to ends and midpoints of lines, arcs, polyline segments, splines, multilines and region edges, and to centers of circles, arcs, ellipses and polyline arc segments. Snapping returns the exact point in world coordinates, not a screen approximation. | T1 | 1 |
| DB-SNAP-002 | Geometric center snap | Snap to the area centroid of closed polylines, regions, closed splines and planar faces. | T2 | 1 |
| DB-SNAP-003 | Node, insertion and quadrant snaps | Snap to point objects and dimension definition points, to insertion points of blocks, text, attributes and images, and to the 0, 90, 180 and 270 degree points of circles, arcs and ellipses in the current UCS. | T1 | 1 |
| DB-SNAP-004 | Intersection snaps | Snap to actual intersections of coplanar objects, to apparent intersections of objects that cross only in the current view, and to extended intersections of objects' natural extensions. | T1 | 1 |
| DB-SNAP-005 | Extension and parallel snaps | Hovering an object end shows its extension path along which a point can be placed; hovering a line then moving parallel to it offers a path through the last point parallel to that line. Both combine with other snaps and typed distances. | T1 | 1 |
| DB-SNAP-006 | Perpendicular, tangent and nearest snaps | Snap to the foot of the perpendicular and to tangent points on arcs, circles, ellipses and splines from the last point; when no last point exists the snap is deferred until the second point fixes the geometry. Nearest returns the closest point on an object to the cursor. | T1 | 1 |
| DB-SNAP-007 | Snap overrides | A snap mode typed by its three-letter code or chosen from the snap menu overrides the running set for one pick; `NON` disables snapping for one pick. A setting decides whether typed coordinates take precedence over running snaps. | T1 | 1 |
| DB-SNAP-008 | Running snap set | The set of snap modes applied automatically is chosen in settings or with `OSNAP` and toggled as a whole with one key, without losing the chosen modes. | T1 | 1 |
| DB-SNAP-009 | Snap markers, tooltips and cycling | Each candidate snap shows a mode-specific marker and tooltip; Tab cycles candidates under the cursor; marker size, color and the pick aperture size are settings. | T1 | 1 |
| DB-SNAP-010 | Snap from, temporary track and mid-between | `FROM` takes a base point and a relative offset; a temporary track point acquires a one-time alignment origin; mid between two points returns the midpoint of two picked or snapped points. | T1 | 1 |
| DB-SNAP-011 | Point filters | Point filters build a point from coordinates of different picks (X from one point, YZ from another and the other combinations) in 2D and 3D. | T2 | 1 |
| DB-SNAP-012 | 3D object snaps | Snap to vertices, edge midpoints, face centers, knots, perpendicular points on faces and nearest points on faces of solids, surfaces and meshes; a setting projects snapped points to the current elevation or keeps their Z. | T2 | 3 |

## Tracking and constrained cursor

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-SNAP-013 | Object snap tracking | Hovering a snap point acquires it, after which horizontal, vertical or polar alignment paths from acquired points and their intersections are offered; acquisition can be automatic or require a modifier key. Tracking combines with extension, parallel and typed distances. | T1 | 1 |
| DB-SNAP-014 | Polar tracking | The cursor locks to alignment paths at a chosen angle increment plus any additional angles, measured absolute to the UCS or relative to the last segment, with a tooltip showing distance and angle. Polar snap optionally restricts the distance along the path to increments. | T1 | 1 |
| DB-SNAP-015 | Ortho mode | Ortho restricts cursor movement to the UCS axes, or to the axes of the current isometric plane; it excludes polar tracking while on and can be inverted while a key is held. | T1 | 1 |
| DB-SNAP-016 | Grid | A grid of dots or lines with configurable spacing and major lines shows in each viewport, adapts its density to the zoom level, can follow the dynamic UCS and can be limited to the drawing limits. | T1 | 1 |
| DB-SNAP-017 | Snap modes | Cursor snap restricts picks to increments with spacing, angle and base point, in rectangular, isometric or polar style. | T1 | 1 |
| DB-SNAP-018 | Isometric drafting | An isometric mode switches grid, snap, ortho and the cursor to the left, top and right isometric planes, cycled by a key, and draws circles as isometric ellipses on the active plane. | T2 | 1 |

## Coordinate entry

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-SNAP-019 | Cartesian and polar coordinates | Points can be typed as absolute or relative (prefixed `@`) cartesian `x,y[,z]` or polar `distance<angle`, interpreted in the current UCS and current units. `@` alone means the last point. | T1 | 1 |
| DB-SNAP-020 | Cylindrical and spherical coordinates | 3D points can be typed in cylindrical and spherical forms, absolute or relative. | T2 | 1 |
| DB-SNAP-021 | World and absolute overrides | A prefix forces world coordinates regardless of the current UCS, and another forces absolute coordinates when cursor input defaults to relative. | T1 | 1 |
| DB-SNAP-022 | Direct distance and angle override | With a point established, pointing a direction and typing a distance places the next point along that direction, respecting ortho, polar and tracking; typing `<angle` locks the direction for the next distance. | T1 | 1 |
| DB-SNAP-023 | Tracking entry mode | A tracking mode accepts successive reference points whose offsets accumulate into one point, finished with Enter. | T3 | 1 |
| DB-SNAP-024 | Unit-aware entry | Lengths are accepted in the drawing's unit format, including feet-and-inches and fractions, and angles in decimal degrees, degrees-minutes-seconds, grads, radians and surveyor bearings. | T1 | 1 |

## User coordinate systems

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-SNAP-025 | Define a UCS | `UCS` defines a coordinate system by origin, three points, an object, a face, the current view, the Z axis or rotation about any axis, and returns to the world system or the previous UCS. | T1 | 1 |
| DB-SNAP-026 | Named and preset UCSs | UCSs are saved by name in the drawing, restored, renamed and deleted; six orthographic presets relative to a base UCS are provided. | T2 | 1 |
| DB-SNAP-027 | Dynamic UCS | While a drawing or editing command is active, hovering a planar face of a solid or mesh temporarily aligns the UCS to it and restores the previous UCS when the command ends. | T2 | 3 |
| DB-SNAP-028 | Plan view and UCS-following views | `PLAN` shows the plan view of the current UCS, a named UCS or the world system, and an option makes views follow UCS changes. Per-viewport UCS is DB-PLOT-011. | T2 | 1 |
| DB-SNAP-029 | UCS icon | The UCS icon shows the axes in 2D or 3D style at the origin or the corner, per viewport, and can be selected and edited by grips to move or rotate the UCS. | T1 | 1 |

## Units and measurement

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-SNAP-030 | Unit formats | `UNITS` sets the length format (decimal, engineering, architectural, fractional, scientific), angle format, display precision, angle base direction and positive angle direction, all saved in the drawing. Changing the format changes display only, never stored coordinates. | T1 | 1 |
| DB-SNAP-031 | Distance and point inquiry | `DIST` reports the distance, the deltas and the angles between two points; `ID` reports the coordinates of a point and makes it the last point. | T1 | 1 |
| DB-SNAP-032 | Area inquiry | `AREA` reports area and perimeter of a polygon by points or of closed objects, with running totals that add and subtract regions. | T1 | 1 |
| DB-SNAP-033 | Measure tool | A measure command reports distance, radius, angle, area and volume of picked geometry, and a quick mode shows distances and angles under the cursor between nearby geometry as the pointer moves. | T2 | 1 |
| DB-SNAP-034 | Region mass properties | `MASSPROP` on regions reports area, perimeter, bounding box, centroid, moments and products of inertia and radii of gyration, and can write the report to a file. Solids are covered in the 3D area. | T2 | 1 |

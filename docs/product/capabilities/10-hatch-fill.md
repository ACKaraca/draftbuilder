# Hatches and fills

Hatches fill closed areas with line patterns, solid color or gradients. Associative hatches follow
their boundaries through the dependency graph, and pattern generation runs on worker threads so
large and dense fills do not block editing.

## Domain objects

- **Hatch** — a fill with one or more boundary loops, a pattern or gradient definition, origin, scale, angle and associativity flag.
- **Hatch pattern** — a named line-family definition from a PAT file or the user-defined parallel-line form.
- **Gradient** — a one- or two-color fill with a shape type, angle and centering.
- **Boundary loop** — an outer or island loop made of lines, arcs, elliptical arcs, splines or polylines, optionally linked to source objects.

## Creating hatches

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-HATCH-001 | Hatch (`HATCH`, `-HATCH`) | Fills areas found by picking internal points or by selecting closed objects, with live preview before commit; the hyphen form sets every option through prompts for scripts. | T1 | 1 |
| DB-HATCH-002 | Boundary detection | Picking a point finds the smallest enclosing boundary among visible objects in the current boundary set and detects islands; text and attribute extents are treated as islands so fills do not run through text. | T1 | 1 |
| DB-HATCH-003 | Island modes | Island handling fills alternate nested areas, only the outermost area, or ignores islands entirely. | T1 | 1 |
| DB-HATCH-004 | Gap tolerance | A gap tolerance in drawing units lets boundaries with small gaps close; when the boundary is open the command marks the gap location instead of failing silently. | T1 | 1 |
| DB-HATCH-005 | Separate or combined | Several picked areas produce one hatch object or separate hatch objects per area, as chosen. | T2 | 1 |
| DB-HATCH-006 | Draw order on creation | A new hatch is placed behind its boundary, in front of it, at the back or at the front of the draw order, as chosen. | T1 | 1 |
| DB-HATCH-007 | Default hatch layer | New hatches go to a designated layer regardless of the current layer when that setting is used. | T2 | 1 |

## Patterns and gradients

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-HATCH-008 | Predefined and custom patterns | Loads patterns from PAT files, including user-supplied files from the support path, and shows them as swatches in a browsable pattern palette. | T1 | 1 |
| DB-HATCH-009 | User-defined pattern | Generates parallel lines at a set angle and spacing, optionally doubled at 90 degrees. | T1 | 1 |
| DB-HATCH-010 | Scale, angle and pen width | Patterns scale and rotate; metric pen-width patterns scale by a chosen pen width; scale may be relative to paper-space units in a viewport or follow annotation scaling (see DB-PLOT). | T1 | 1 |
| DB-HATCH-011 | Origin control | The pattern origin is the drawing origin, a picked point, a boundary extent corner or center, or the current origin, and can be stored as the default. | T1 | 1 |
| DB-HATCH-012 | Solid fill | Solid fills use any index or true color, plot as filled areas and keep their boundary associativity; the outline-only display setting is DB-DRAW-024. | T1 | 1 |
| DB-HATCH-013 | Gradients | Fills with one-color tint or two-color gradients in linear, cylindrical, inverted cylindrical, spherical, hemispherical and curved forms, with angle and centering. | T2 | 1 |
| DB-HATCH-014 | Background color and transparency | Pattern hatches can have a background fill color, and every hatch has its own transparency. | T2 | 1 |
| DB-HATCH-015 | Block or image fill | Fills an area by tiling a block or raster image clipped to the boundary, stored as standard objects with the source recorded. | T3 | 3 |

## Associativity and editing

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-HATCH-016 | Associative hatch | An associative hatch regenerates when its boundary objects are moved, stretched or edited; if an edit leaves the boundary open, the hatch keeps its last shape, loses association and is reported. | T1 | 1 |
| DB-HATCH-017 | Edit hatch (`HATCHEDIT`) | Changes pattern, scale, angle, origin, gradient, islands, associativity and layer of existing hatches, and adds or removes boundary loops. | T1 | 1 |
| DB-HATCH-018 | Recreate boundary | Creates a polyline or region on a chosen layer from the hatch's loops, optionally re-associating the hatch to the new boundary. | T2 | 1 |
| DB-HATCH-019 | Hatch grips | Grips move the origin, change angle and scale, and stretch boundary vertices or edges of non-associative hatches directly. | T2 | 1 |
| DB-HATCH-020 | Match hatch properties | Match properties copies pattern, scale, angle, origin and gradient from one hatch to others. | T1 | 1 |

## Performance and safeguards

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-HATCH-021 | Dense pattern guard | When a pattern would generate more lines than a configurable limit, the hatch displays as a solid or outline preview with a warning instead of exhausting memory; the stored definition is unchanged. | T1 | 1 |
| DB-HATCH-022 | Parallel generation | Pattern lines for large or complex boundaries (thousands of vertices) are clipped and generated on worker threads and cached per revision, so editing continues while fills complete. | T3 | 1 |
| DB-HATCH-023 | Preservation on exchange | Hatches read from DWG or DXF keep pattern definition data, seed points and boundary links exactly, even for pattern names not present in local PAT files. | T1 | 1 |

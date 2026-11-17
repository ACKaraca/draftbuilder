# 3D modeling and visualization

DraftBuilder models solids, surfaces and meshes in the drawing database, derives 2D drawing views
and sections from them, and presents them with visual styles, cameras, lights and materials. Exact
solid operations run through `arc-brep`, drawing views through `arc-view`, and photoreal rendering
through RenderBuilder over the Arc Scene contract.

## Domain objects

- **Solid** — a closed boundary-representation body stored as DWG solid data, with optional primitive history.
- **Surface** — a procedural (associative, regenerated from its inputs) or NURBS surface.
- **Mesh** — a polygon mesh with subdivision smoothing level and creases.
- **Section plane** — a plane, slice, boundary or volume object that cuts the display and generates section geometry.
- **Drawing view** — an associative 2D projection, section or detail of 3D content placed on a layout.
- **Visual style** — a named set of face, edge, lighting and environment display settings.
- **Camera** — a saved eye point, target, lens and clipping, with an optional named view.
- **Light** — a point, spot, distant, photometric or sun light.
- **Material** — an OpenPBR-aligned appearance definition with texture mapping.

## Solid creation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-M3D-001 | Solid primitives | `BOX`, `SPHERE`, `CYLINDER`, `CONE`, `WEDGE`, `PYRAMID` and `TORUS` create solids from typed or picked dimensions, including elliptical bases, frustums and side counts; grips edit their defining dimensions while primitive history is kept. | T2 | 3 |
| DB-M3D-002 | Polysolid (`POLYSOLID`) | Creates a wall-like solid of given height, width and justification along picked points or an existing line, arc, polyline or circle. | T2 | 3 |
| DB-M3D-003 | Extrude (`EXTRUDE`) | Extrudes closed profiles to solids and open profiles to surfaces by height, direction, path or taper angle, where the height can be an expression; a mode setting forces surfaces. Source profiles are kept or deleted by setting. | T2 | 3 |
| DB-M3D-004 | Revolve (`REVOLVE`) | Revolves profiles about an axis given by points, an object or a UCS axis through a given angle, producing solids or surfaces. | T2 | 3 |
| DB-M3D-005 | Sweep (`SWEEP`) | Sweeps profiles along a path with options for profile alignment to the path, base point, scale along the path, twist and banking; invalid self-intersecting results are refused with the location shown. | T2 | 3 |
| DB-M3D-006 | Loft (`LOFT`) | Lofts through ordered cross sections, optionally controlled by guide curves or a path, with ruled, smooth, normal-to-section and draft-angle controls and an option to close the loft. | T2 | 3 |
| DB-M3D-007 | Press or pull (`PRESSPULL`) | Dragging or typing a distance inside a bounded area or on a solid face extrudes the area or offsets the face, adding to or removing from the solid. | T2 | 3 |
| DB-M3D-008 | Planar surface and conversions | `PLANESURF` creates a planar surface from corners or closed objects; `CONVTOSOLID`, `CONVTOSURFACE` and `THICKEN` turn closed meshes, thick polylines and surfaces into solids or surfaces; sculpting a watertight set of surfaces creates a solid. | T2 | 3 |

## Solid editing

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-M3D-009 | Booleans (`UNION`, `SUBTRACT`, `INTERSECT`) | Combines solids, surfaces and regions by union, difference and intersection; a failed operation leaves the inputs unchanged and reports the reason. | T2 | 3 |
| DB-M3D-010 | Interference check (`INTERFERE`) | Finds overlapping volumes between two sets of solids, highlights each pair and optionally creates the interference solids. | T2 | 3 |
| DB-M3D-011 | Slice (`SLICE`) | Cuts solids by a plane, surface or planar object, keeping one or both sides. | T2 | 3 |
| DB-M3D-012 | Face, edge and body editing (`SOLIDEDIT`) | Extrudes, moves, rotates, offsets, tapers, deletes, copies and colors faces; copies and colors edges; imprints, cleans, separates, shells and checks bodies. Each operation is one undoable command. | T2 | 3 |
| DB-M3D-013 | Edge fillet and chamfer (`FILLETEDGE`, `CHAMFEREDGE`) | Rounds or bevels selected edges or edge chains with a radius or two distances, with grips to adjust the value before applying. | T2 | 3 |
| DB-M3D-014 | Shell, imprint, separate, clean | `SHELL` hollows a solid with a wall thickness and removed faces; `IMPRINT` presses curves onto faces; `SEPARATE` splits disjoint lumps into separate solids; `CLEAN` removes redundant edges and vertices. | T2 | 3 |
| DB-M3D-015 | Extract and offset edges (`EXTRACTEDGES`, `OFFSETEDGE`) | Creates wireframe curves from the edges of solids, surfaces and regions, and creates a closed curve offset within a planar face. | T2 | 3 |
| DB-M3D-016 | Primitive history | When history is kept, composite solids remember their primitives and booleans; a history-display setting shows them and selecting a primitive inside a composite edits its dimensions and regenerates the composite. | T3 | 3 |
| DB-M3D-017 | Subobject selection and gizmos | Faces, edges and vertices are selected with a modifier key and a filter; move, rotate and scale gizmos constrain changes to an axis or plane and can be relocated. | T2 | 3 |
| DB-M3D-018 | 3D transforms | `3DMOVE`, `3DROTATE`, `3DSCALE`, `3DARRAY` (rectangular with levels, polar about an axis), `MIRROR3D` and `3DALIGN` transform objects in three dimensions with the same prompt conventions as their 2D counterparts. | T2 | 3 |
| DB-M3D-019 | Solid check and 3D mass properties | Checks solids for validity and reports defects; mass properties of solids report volume, bounding box, centroid, moments and products of inertia, radii of gyration and principal moments, writable to a text file. | T2 | 3 |

## Surfaces

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-M3D-020 | Procedural and NURBS surfaces | A setting chooses whether new surfaces are procedural (associative to their source curves and other surfaces, regenerated when those change) or NURBS (editable by control vertices). | T2 | 3 |
| DB-M3D-021 | Surface creation | Surfaces are created by extrude, revolve, sweep, loft, network of curves in two directions, patch over a closed edge loop with continuity and bulge, and blend between edges with continuity control. | T2 | 3 |
| DB-M3D-022 | Surface editing | Surfaces are offset, filleted with a radius, extended by distance with merge or append, trimmed by curves, surfaces or regions with projection direction, and untrimmed. | T2 | 3 |
| DB-M3D-023 | Control-vertex editing | Control vertices of NURBS surfaces and splines are shown, hidden, added, removed and rebuilt to a given degree and count; procedural surfaces convert to NURBS on request. | T2 | 3 |
| DB-M3D-024 | Surface analysis | Zebra stripes, curvature color mapping and draft-angle mapping display on surfaces and solids for continuity and manufacturability checks. | T3 | 3 |

## Meshes and faces

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-M3D-025 | Mesh primitives and legacy surfaces | Mesh box, cone, cylinder, pyramid, sphere, wedge and torus are created with tessellation divisions; ruled, tabulated, revolved and edge-defined surfaces build meshes from curves. | T2 | 3 |
| DB-M3D-026 | Mesh smoothing and refinement | Meshes smooth up and down through subdivision levels with a face-count warning before large increases; refine turns smoothed faces into editable faces; creases keep edges sharp at all or chosen levels. | T2 | 3 |
| DB-M3D-027 | Mesh editing | Faces are split, extruded, merged, collapsed, spun (triangle edge flip) and holes capped; edits apply to the selected subobjects. | T2 | 3 |
| DB-M3D-028 | Mesh conversion | Solids and surfaces convert to meshes using tessellation settings (face type, deviation, angle, aspect ratio, edge length); meshes convert to smooth or faceted solids and surfaces when watertight. | T2 | 3 |
| DB-M3D-029 | 3D faces (`3DFACE`, `EDGE`) | Creates three- or four-sided planar faces with per-edge visibility, and toggles edge visibility of existing faces. | T2 | 3 |

## Sections and drawing views

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-M3D-030 | Section planes (`SECTIONPLANE`) | Places a section object as a plane, slice of given thickness, boundary or volume; jogs can be added; live sectioning cuts the display of intersected solids, surfaces and meshes with fill and edge styling from section settings. | T2 | 3 |
| DB-M3D-031 | Generate section geometry | From a section object, generates a 2D section or elevation block or a 3D cut block with intersection fill, background and foreground lines and hidden lines styled per settings; regenerating updates the block. | T2 | 3 |
| DB-M3D-032 | Flat projection (`FLATSHOT`) | Projects visible 3D content onto the current view plane as 2D linework, inserted as a block or written to a file, with separate styling for foreground and obscured lines. | T2 | 3 |
| DB-M3D-033 | Drawing views from 3D | Base, projected, section (full, half, offset, aligned) and detail views of model-space 3D content are placed on layouts, computed by `arc-view` with hidden-line removal and cut patterns. Views carry styles for hatching, line visibility and labels. | T2 | 3 |
| DB-M3D-034 | View update | Drawing views are associative: when the 3D content changes they are marked out of date and update automatically or on command; annotations attached to view geometry follow it. Updates run in the background against a document revision. | T2 | 3 |

## Display and navigation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-M3D-035 | Visual styles | Ships 2D wireframe, wireframe, hidden, shaded, shaded with edges, realistic, conceptual, shades of gray, sketchy and x-ray styles; custom styles set face shading and opacity, lighting quality, shadows, background, edge mode, silhouettes, intersection and obscured edges, jitter and overhang. | T2 | 3 |
| DB-M3D-036 | Display quality settings | Settings control curve tessellation, surface facet resolution, isolines, silhouette display and hardware effects; GPU display through `arc-gpu` keeps interaction responsive by lowering detail during navigation. | T2 | 3 |
| DB-M3D-037 | Shaded display at large coordinates | Shaded visual styles, textures, shadows, section fills and 3D object snaps keep full precision for content at survey-scale coordinates, using the camera-relative display of the drawing database. | T2 | 3 |
| DB-M3D-038 | 3D navigation | Orbit (constrained, free, continuous), walk and fly with step size and speed settings, preset orthographic and isometric views, and zoom and pan in perspective; navigation runs transparently inside commands. | T2 | 3 |
| DB-M3D-039 | View orientation control | An on-canvas control shows the current orientation, switches to faces, edges and corners of a cube, returns to a home view and toggles perspective and parallel projection; a navigation bar offers the same actions. | T2 | 3 |
| DB-M3D-040 | Cameras (`CAMERA`) | Camera objects store location, target, lens length or field of view and front and back clipping; they are grip-editable, can be shown or hidden, and create named views. | T2 | 3 |
| DB-M3D-041 | Perspective and named 3D views | Views switch between perspective and parallel projection with a lens length; named views store camera, visual style, background, live section and layer snapshot. | T2 | 3 |

## Lights, materials and rendering

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-M3D-042 | Lights (`LIGHT`) | Point, spot (hotspot and falloff), distant and photometric lights with IES distribution files are placed and grip-edited, with intensity, color or filter color, attenuation and shadow type; a lights list panel shows and edits all lights. | T2 | 4 |
| DB-M3D-043 | Sun and sky | A sun light derives its direction from the drawing's geographic location, date and time; sky background and exposure follow it. | T2 | 4 |
| DB-M3D-044 | Lighting units | Lighting is generic or photometric in international or imperial units, with photometric values converted consistently with drawing units. | T2 | 4 |
| DB-M3D-045 | Shadows in the viewport | Visual styles can show ground and full shadows from the sun and lights. | T3 | 4 |
| DB-M3D-046 | Materials | OpenPBR-aligned materials are created from a bundled library or edited (color, image textures, roughness, metalness, transparency, refraction, emission, bump and normal maps, procedural textures) and attached to objects, layers or faces. | T2 | 4 |
| DB-M3D-047 | Material mapping | Planar, box, cylindrical and spherical mapping are applied and adjusted with a gizmo, including tiling, offset, rotation and scale. | T2 | 4 |
| DB-M3D-048 | Rendering through RenderBuilder | Rendering exports an Arc Scene of the current view to a RenderBuilder process and shows returned frames in a render window; presets set quality, the target is the window, a viewport region or a file, and render history is kept. When RenderBuilder is not installed the command explains how to obtain it. | T2 | 4 |
| DB-M3D-049 | Backgrounds and environment | Named views have solid, gradient, image or sun-and-sky backgrounds; image-based lighting environments and depth cueing are passed to the renderer. | T2 | 4 |
| DB-M3D-050 | Camera path sequences | A camera moves along a path or between named views to produce a frame sequence rendered by RenderBuilder; DraftBuilder does not encode video. | T3 | 4 |

## Output for fabrication

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-M3D-051 | 3D print output (`STLOUT`, `3DPRINT`) | Writes selected solids and watertight meshes to STL (binary or text) or 3MF with unit choice; non-watertight or empty selections are refused and the output size is reported against an optional build volume. | T2 | 3 |

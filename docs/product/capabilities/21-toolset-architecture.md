# Architecture toolset

The architecture toolset adds building objects to DraftBuilder: walls, doors, windows, openings, curtain walls, slabs, roofs, stairs, railings, spaces, grids and levels, with generated sections and elevations, detail components, keynotes, schedules and renovation states. Objects are typed and style-driven, built on the framework in [20-toolset-framework.md](20-toolset-framework.md), and they produce ordinary drawing output at every level of detail.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Wall | Path (line or arc segments), height, justification, style, base and top conditions | Hosts doors, windows and openings; bounds spaces; joins other walls; sits on a level |
| Wall style | Component stack (width, offset, function, priority, material, hatch), end caps | Referenced by walls; defines cleanup behavior |
| Cleanup group | Member walls, join rule, radius | Resolves component priorities at wall junctions |
| Door, window | Style, size, offset along wall, swing, open percentage, sill height | Hosted by a wall; tagged; scheduled |
| Door/window assembly | Grid of cells with sidelights, transoms, panels | Hosted by a wall; contains door and window cells |
| Opening | Profile, depth, display control | Voids a wall, slab or roof; may be hosted or free |
| Curtain wall | Path or face, grid division rules, mullion profiles, infill panels, units | Hosted on a wall path or mass face; hosts doors in units |
| Slab | Boundary polyline, thickness, edge profile, holes, style | Bounds spaces; hosts openings; sits on a level |
| Roof, roof slab | Boundary, slope per edge, thickness, edge profile, style | Interacts with wall tops; hosts openings |
| Stair | Flight layout, rise, run, width, landings, style, code limits | Connects two levels; hosts a railing |
| Railing | Path, post, baluster and handrail rules, style | Hosted by a stair, slab edge or free path |
| Space | Boundary, height, name, number, function, associative flag, style | Bounded by walls, slabs and roofs; belongs to zones and area groups |
| Zone | Name, member spaces, function | Groups spaces |
| Area group | Classification, member spaces, gross and net area rules | Aggregates spaces under an area calculation standard |
| Structural member | Shape from catalog, length, justification, material | Placed on grids; scheduled |
| Mass element | Volume, floor divisions | Converts faces to walls, slabs and roofs |
| Column grid, ceiling grid | Axes, spacing, labels, bubble style | Anchors for columns, ceilings and layout |
| Level | Name, elevation, floor-to-floor height | Contains placed objects; drives views and sections |
| Section line, elevation line | Cut line, depth, height range, direction, target | Produces generated views |
| Generated view | Source line, mode (live 3D, 2D flattened), layers, manual overrides | Derived from model objects; placed on sheets |
| Callout | Boundary, base view, scale | Produces a detail view |
| Detail component | Catalog item, size, keynote | Placed in 2D detail drawings |
| Keynote | Code, text, source database | Attached to details and objects; collected in legends |
| Renovation state | Name (existing, demolish, new, temporary), display rule | Assigned to any building object |

## Walls

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TARCH-001 | Wall creation | Draws walls from a style by picking line or arc segments, with height, justification (left, center, right, baseline) and base and top conditions. Walls drawn in sequence join automatically. | T1 | 4 |
| DB-TARCH-002 | Wall styles | A wall style is a component stack; each component has width, offset, function, priority, material and hatch, and end caps can be defined. Editing a style regenerates every wall that uses it. | T1 | 4 |
| DB-TARCH-003 | Wall joins and cleanup | Resolves junctions (corner, T, cross) by component priority so the highest-priority component runs through the junction; cleanup groups and a custom cleanup radius override the default. | T1 | 4 |
| DB-TARCH-004 | Wall editing | Provides extend, trim, split, merge, reverse, add and remove vertex, and offset as commands and grips. Hosted objects stay attached to their wall through the edit where the host segment still exists. | T1 | 4 |
| DB-TARCH-005 | Wall top and bottom conditions | Sets wall tops and bases to follow a roof, slab, polyline or fixed height, with interference conditions controlling how a wall ends where it meets a roof or slab. | T1 | 4 |
| DB-TARCH-006 | Wall sweeps and projections | Adds reveals, cornices and bases to a wall from a profile, with start and end offsets and a per-component display. | T2 | 4 |
| DB-TARCH-007 | Wall and polyline conversion | Converts polylines to walls and wall paths to polylines in both directions, keeping the style or mapping a layer to a style by table. | T1 | 4 |
| DB-TARCH-008 | Wall openings | Cuts a custom-profile void through a wall with its own display, retained when the wall style changes. | T2 | 4 |
| DB-TARCH-009 | Wall component inspection | Lists how the components of selected walls meet and which priority decides each junction, and highlights junctions where two components have equal priority. | T3 | 4 |
| DB-TARCH-010 | Wall modifiers and custom shape | Replaces a wall's height profile by a polyline in elevation, producing gables and stepped tops, with the profile editable by grips. | T2 | 4 |

## Doors, windows and openings

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TARCH-011 | Hosted doors and windows | Places a door or window in a wall from a style with an offset from a wall end or centerline; the wall is cut automatically, and the object follows wall edits. | T1 | 4 |
| DB-TARCH-012 | Door and window styles | A style defines parametric width and height, frame and sash geometry, swing arc and open percentage. Plan, elevation and model representations are generated from the same parameters. | T1 | 4 |
| DB-TARCH-013 | Door and window assemblies | Composes cells in a grid with sidelights, transoms and fixed panels, each cell a door, window or panel with its own style. | T2 | 4 |
| DB-TARCH-014 | Openings | Creates a void in a wall, slab or roof by picking a profile or from a host edge, with depth and display control; deleting the host removes the opening. | T1 | 4 |
| DB-TARCH-015 | Standard sizes and layout | Restricts sizes to a list defined in the style and places doors and windows at even spacing along a wall, a curve or against grid lines. | T2 | 4 |
| DB-TARCH-016 | Door and window tags | Tags carry the object's number and size from a property set; renumbering updates the tag and the schedule. | T1 | 4 |
| DB-TARCH-017 | Swing and clearance display | Draws door swing arcs at the style's open percentage and optionally shows required clearance zones, with each hidden in non-plan representations. | T2 | 4 |

## Curtain walls

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TARCH-018 | Curtain wall creation | Creates a curtain wall from a wall path or a face with grid division rules (fixed size, count, divide by distance), mullion profiles and infill panels. | T2 | 4 |
| DB-TARCH-019 | Grid division rules | Divides horizontally and vertically by fixed spacing, equal count or an explicit list, with a rule order and the remainder placed at start, end or distributed. | T2 | 4 |
| DB-TARCH-020 | Mullions and infill | Assigns mullion profiles per grid line type (primary, secondary, perimeter) and infill panels per cell, with per-cell override to door, vent or opaque panel. | T2 | 4 |
| DB-TARCH-021 | Curtain wall units | Groups cells into prefabricated units with unit joints, and lists units and their dimensions in a schedule. | T3 | 4 |

## Slabs and roofs

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TARCH-022 | Slabs | Creates a slab from a boundary polyline with thickness, edge profile and style-driven hatch, with holes added by openings or inner boundaries. | T1 | 4 |
| DB-TARCH-023 | Slab edges and offsets | Sets slab thickness direction, offsets the top from the level, and applies edge profiles to selected edges with a per-edge override. | T2 | 4 |
| DB-TARCH-024 | Roofs | Creates a roof from a boundary with a slope or overhang per edge, thickness and edge profile; slopes edited per edge update the ridges and valleys of the resulting surface. | T1 | 4 |
| DB-TARCH-025 | Roof from walls | Creates a roof over a closed set of walls with a default slope and overhang, associative to the walls when requested. | T2 | 4 |
| DB-TARCH-026 | Roof modifiers | Adds dormers, hips, gables and edge trims through roof edge and vertex commands, each editable after creation. | T3 | 4 |
| DB-TARCH-027 | Slab and roof conversion | Converts closed polylines, mass faces and solid faces to slabs and roofs, with the source optionally retained. | T2 | 4 |

## Stairs and railings

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TARCH-028 | Stair creation | Creates straight, U-shaped, multi-landing and spiral stairs between two levels from a path or boundary, calculating the number of risers from the level heights. | T1 | 4 |
| DB-TARCH-029 | Stair code limits | Checks rise, run, width, headroom and landing size against limits stored in the style and flags violations in the properties panel and the audit. | T2 | 4 |
| DB-TARCH-030 | Stair styles | Defines tread, riser, stringer and nosing geometry and the plan representation, including the cut line and direction arrow. | T1 | 4 |
| DB-TARCH-031 | Railings | Creates railings along a path, a stair or a slab edge with post spacing, balusters and handrail profile from the style; length and corner posts update when the path changes. | T1 | 4 |
| DB-TARCH-032 | Stair and railing association | A railing hosted on a stair follows its flights and landings; editing the stair updates the railing, and detaching converts it to a free railing. | T2 | 4 |

## Structure, masses, grids and levels

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TARCH-033 | Structural members | Places beams, columns and braces from a shape catalog with length, justification and end trims, and snaps them to grid intersections. | T2 | 4 |
| DB-TARCH-034 | Mass elements and groups | Models conceptual volumes with floor divisions and combines them into groups; shows floor area and gross volume per mass. | T2 | 4 |
| DB-TARCH-035 | Mass to building objects | Converts selected mass faces to walls, slabs or roofs with the chosen styles, and keeps the objects associated to the face when requested. | T2 | 4 |
| DB-TARCH-036 | Column and ceiling grids | Creates orthogonal or angled grids with axis labels and bubbles; ceiling grids carry tile modules and light placement points. | T1 | 4 |
| DB-TARCH-037 | Levels | Defines levels with name, elevation and floor-to-floor height; objects reference levels, and changing an elevation moves hosted objects. | T1 | 4 |
| DB-TARCH-038 | Level attachment of views | Assigns each view drawing a level range and cut height, so plans of different levels are generated from the same model drawings. | T1 | 4 |

## Spaces, zones and areas

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TARCH-039 | Space generation | Detects a boundary from enclosing walls, slabs and open edges by a pick inside a region and creates a space with area, perimeter and height. | T1 | 4 |
| DB-TARCH-040 | Associative spaces | An associative space tracks the objects that bound it and updates its boundary and area when they move; a non-associative space keeps a fixed boundary. | T1 | 4 |
| DB-TARCH-041 | Space styles and data | Spaces carry name, number, function, finish data and occupancy from a property set, with styles setting the plan hatch and label. | T1 | 4 |
| DB-TARCH-042 | Zones | Groups spaces into zones by function, department or fire compartment, with area totals and a display by zone. | T2 | 4 |
| DB-TARCH-043 | Area groups and evaluation | Computes gross, net, usable and circulation areas under an area calculation standard defined as rules (inclusion, wall measurement line, deductions) and reports per group. | T2 | 4 |
| DB-TARCH-044 | Space tags and finishes | Tags show space data; finish schedules list floor, wall and ceiling finishes per space with quantities from the boundary. | T1 | 4 |
| DB-TARCH-045 | Space adjacency | Lists adjacent spaces and the separating objects (walls, doors), supporting an adjacency matrix report and fire-separation checks. | T3 | 5 |

## Sections, elevations and detail views

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TARCH-046 | Section and elevation lines | Defines a cut or view direction with a depth and a height range, and displays a marker that references the generated view and its sheet. | T1 | 4 |
| DB-TARCH-047 | Generated views | Generates a section or elevation as live-linked 3D, or as a flattened 2D view, with cut geometry and background geometry on separate layers and exact hidden-line results. | T1 | 4 |
| DB-TARCH-048 | Updating generated views | Regeneration reflects model changes; manual linework added to a flattened view is kept as overrides and reported if its underlying geometry moved. | T1 | 4 |
| DB-TARCH-049 | Callouts and detail views | Defines a boundary on a base view and generates a detail view at a chosen scale with a higher detail level. | T2 | 4 |
| DB-TARCH-050 | View marks and references | Generated bubbles, titles and sheet numbers update when a view moves to another sheet or is renamed. | T1 | 4 |
| DB-TARCH-051 | Cut pattern from materials | Section cut faces are hatched from the material of each cut component, with consistent joins between touching components of the same material. | T1 | 4 |

## Detail components and keynotes

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TARCH-052 | Detail component library | A catalog of 2D components (lumber, concrete, insulation, fasteners, steel shapes) with parametric size, placed in detail drawings with the section hatch and keynote preset. | T2 | 4 |
| DB-TARCH-053 | Linear and repeat placement | Places a detail component along a path at an interval or repeated count, with trim at the ends. | T2 | 4 |
| DB-TARCH-054 | Keynote database | Holds hierarchical keynotes from a classification system or user-defined table, in an open file with code, text and parent; attaches them to detail components, objects and plain entities. | T2 | 4 |
| DB-TARCH-055 | Keynote legends | Generates a legend per drawing or sheet listing only the keynotes in use, updating when keynotes are added or removed. | T2 | 4 |
| DB-TARCH-056 | Keynote editor | Edits keynote text and codes in the database and updates all drawings in a project at the next project update, with a report of keynotes with no match. | T3 | 4 |

## Schedules, tags and dimensions

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TARCH-057 | Building schedules | Provides ready schedule definitions for doors, windows, rooms, finishes, walls and stairs that use the framework schedule engine with columns from property sets. | T1 | 4 |
| DB-TARCH-058 | Associative building dimensions | Dimension chains that attach to wall faces, openings, grid lines and structural members, and update when those move; groups for exterior, interior and structural chains. | T1 | 4 |
| DB-TARCH-059 | Quantity take-off | Extracts lengths, areas and volumes per class, style, material or level, including wall areas net of openings, into a schedule. | T2 | 4 |
| DB-TARCH-060 | Layer rules for architecture | Provides layer rule tables per layering standard that place every class and style component on the right layer, with an audit of deviations. | T2 | 4 |

## Renovation states

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TARCH-061 | Renovation state assignment | Assigns a state (existing, to be demolished, new, temporary) to any building object, singly or by selection, with state inherited by hosted objects unless overridden. | T2 | 4 |
| DB-TARCH-062 | State-driven display | A display configuration shows each state with its own linetype, color and hatch, so one model produces existing, demolition and proposed plans. | T2 | 4 |
| DB-TARCH-063 | State filters | Hides or isolates objects by state in views, schedules and quantity reports, such as new areas only or demolition quantities. | T2 | 4 |
| DB-TARCH-064 | Demolition handling | Marks a demolished wall with the demolition representation while keeping hosted objects; a new wall in the same position creates no join with the demolished one. | T3 | 4 |
| DB-TARCH-065 | State change history | Records state changes with date and author in the journal, so a project can produce the plan of any recorded stage. | T3 | 5 |

## Interoperability

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TARCH-066 | IFC export of building objects | Exports walls, doors, windows, slabs, roofs, stairs, spaces and levels as IFC entities with their quantities and property sets, and reports objects that export only as generic proxies. | T2 | 4 |
| DB-TARCH-067 | IFC import as objects | Imports IFC walls, slabs, doors and windows as typed objects where the geometry can be expressed as parametric objects and as plain solids elsewhere, with a loss report. | T3 | 5 |
| DB-TARCH-068 | Plain-entity output | Exploded output follows the framework rules, grouping components of each wall, door and stair so the plan reads correctly in other applications. | T1 | 4 |

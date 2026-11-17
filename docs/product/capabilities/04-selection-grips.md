# Selection and grips

Selection builds the set of objects a command acts on, by pointer gestures, prompt options,
property queries and similarity; grips let selected objects be edited directly by their control
points.

## Domain objects

- **Selection set** — an ordered set of object references, optionally with sub-object references.
- **Previous set** — the most recent selection set, kept for reuse.
- **Selection filter** — a named, saved predicate over object types and properties with logical grouping.
- **Grip** — an editable control point exposed by an object; dynamic block references expose custom grips.
- **Isolation state** — the set of objects temporarily hidden or isolated in a view.

## Selecting objects

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-SEL-001 | Pick | Clicking an object selects it when the pick box touches it; the pick box size is a setting and hidden, frozen and unloaded objects are never picked. | T1 | 1 |
| DB-SEL-002 | Window and crossing | Dragging from left to right selects objects fully inside the rectangle; right to left selects objects inside or crossing it. The two rectangles are drawn in distinct styles, and click-click or press-drag gestures are a setting. | T1 | 1 |
| DB-SEL-003 | Polygon and fence | Window polygon, crossing polygon and fence (a polyline that selects what it crosses) are available at any selection prompt. | T1 | 1 |
| DB-SEL-004 | Lasso | Press-dragging a free-hand outline selects by window or crossing depending on drag direction, and Space cycles window, crossing and fence while dragging. | T2 | 1 |
| DB-SEL-005 | Selection prompt options | At a selection prompt the conventional option keywords select all, last, previous, by group, by box, single, multiple without highlighting, sub-object and object, switch between add and remove, and undo the last addition. | T1 | 1 |
| DB-SEL-006 | Previous and last | Previous reuses the last selection set and last selects the most recently created visible object; previous is cleared when its objects are erased or the space changes. | T1 | 1 |
| DB-SEL-007 | Adding and removing | Shift-click removes an object from the set; a setting makes new picks replace or add to the set. Escape clears the set. | T1 | 1 |
| DB-SEL-008 | Selection cycling | When objects overlap, a key or modifier-click cycles through the candidates, optionally showing a list of them with type and layer to choose from. | T1 | 1 |
| DB-SEL-009 | Noun-verb and verb-noun | Objects selected before a command are used by it without a selection prompt; commands started first prompt for selection. Pre-selection can be disabled. | T1 | 1 |
| DB-SEL-010 | Implicit windowing | Clicking empty space at a selection prompt starts a window or crossing without an option. | T1 | 1 |
| DB-SEL-011 | Selection preview and highlight | Objects under the cursor and inside a dragged window are highlighted before the pick, and selected objects stay highlighted; both effects can be turned off for speed. | T1 | 1 |
| DB-SEL-012 | Rollover tips | Hovering an object shows a tip with its type, layer and configurable properties. | T2 | 1 |
| DB-SEL-013 | Select all respects state | Select all and window selection skip locked, frozen and hidden objects according to settings, and select only visible objects while objects are isolated. | T1 | 1 |
| DB-SEL-014 | Locked-layer behavior | Objects on locked layers cannot be selected for editing but remain snappable, and are drawn faded by a configurable amount. | T1 | 1 |
| DB-SEL-015 | Group selection | Picking a member of a selectable group selects the whole group; a toggle turns group selection off temporarily and the group's members can be edited individually. | T2 | 1 |
| DB-SEL-016 | Sub-object selection filter | A filter restricts picks to vertices, edges, faces or solid history sub-objects of 3D objects; modifier-click picks a sub-object without changing the filter. | T2 | 3 |
| DB-SEL-017 | Large selection sets | Window, select-all and query selections over millions of objects complete using the spatial index without freezing the interface, and selection sets have no fixed size limit. | T3 | 1 |

## Selection by query

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-SEL-018 | Query selection | Objects are selected by type, property, comparison operator (equal, not equal, greater, less, wildcard) and value, over the whole drawing or the current set, including or excluding matches and appending to the current set. | T1 | 1 |
| DB-SEL-019 | Advanced filters | `FILTER` builds a predicate over object types and properties with nested AND, OR, XOR and NOT groups, saves it by name and applies it at any selection prompt, transparently. | T2 | 1 |
| DB-SEL-020 | Select similar | Selects every object sharing chosen properties with the selected ones (type, layer, color, linetype, linetype scale, lineweight, plot style, style name, block or reference name, size). | T2 | 1 |
| DB-SEL-021 | Isolate and hide objects | Selected objects can be isolated (all others hidden) or hidden, and all can be shown again; isolation is a display state that can be kept with the drawing and never modifies the objects. | T1 | 1 |

## Properties on selection

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-SEL-022 | Selection summary | The selection summary lists the selected object types with counts; choosing a type narrows the active selection to that type or removes it, and the properties panel (DB-LAYER-036) then edits only the narrowed set. | T1 | 1 |
| DB-SEL-023 | Properties on select | A compact panel appears near the selection with the most used properties for the selected type; which properties it shows per type is configurable. | T2 | 1 |

## Grips

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-SEL-024 | Grip display | Selected objects show grips at their control points; size, colors and whether grips appear inside blocks are settings, and grips are suppressed above a configurable object count to keep large selections fast. | T1 | 1 |
| DB-SEL-025 | Grip modes | Clicking a grip makes it hot and starts stretch; Space or Enter cycles stretch, move, rotate, scale and mirror, each with base point, copy, reference and undo options. Holding the copy modifier on the first move creates copies. | T1 | 1 |
| DB-SEL-026 | Multiple hot grips | Shift-clicking several grips makes them all hot so they move together in one stretch. | T1 | 1 |
| DB-SEL-027 | Multifunctional grips | Hovering a grip shows the edits valid for it (for polylines: stretch vertex, add vertex, remove vertex, convert segment to arc or line, lengthen ends; for arcs, dimensions, hatches and text: their specific edits), and a modifier key cycles the variants. | T1 | 1 |
| DB-SEL-028 | Grip distance entry | While a grip is hot, typed distances and angles, and cursor-input fields, set the exact displacement, radius or length. | T1 | 1 |
| DB-SEL-029 | Custom grips of dynamic blocks | Dynamic block references show grips with distinct shapes for their parameters (linear, rotation, flip, visibility, lookup, alignment) and dragging or choosing from them drives the parameter. | T2 | 2 |
| DB-SEL-030 | Grips of associative objects | Grips on associative arrays, hatches, dimensions, multileaders and constrained geometry edit the defining parameters and the dependent geometry updates in the same transaction. | T1 | 1 |

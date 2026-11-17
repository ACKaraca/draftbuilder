# Parametric constraints

Parametric constraints capture design intent in 2D geometry: geometric relationships and named,
expression-driven dimensions that a solver keeps satisfied while the drawing is edited. The same
solver drives constraints in block definitions.

## Domain objects

- **Geometric constraint** — a relationship between objects or their defining points (coincident, parallel, tangent and so on).
- **Dimensional constraint** — a named distance, radius, diameter or angle whose value is a number or an expression.
- **Parameter** — a named user or reference value in the drawing's parameter table, with an expression and unit.
- **Constraint network** — the set of constraints and parameters attached to a space or block definition, solved together.
- **Constraint bar** — the on-canvas marker group that shows and edits the constraints of an object.

## Geometric constraints

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-CNSTR-001 | Geometric constraint types | The solver supports coincident, collinear, concentric, fix, parallel, perpendicular, horizontal, vertical, tangent, smooth (curvature-continuous), symmetric and equal constraints between lines, polyline segments, arcs, circles, ellipses, splines and their defining points. Each constraint is stored as a drawing object referencing the constrained objects by stable ID. | T2 | 2 |
| DB-CNSTR-002 | Apply constraint by type (`GEOMCONSTRAINT`) | The user chooses a type and picks objects or points in order; the second object moves to satisfy the relationship while the first stays where it is. A constraint that would contradict existing ones is refused with a message naming the conflicting constraints. | T2 | 2 |
| DB-CNSTR-003 | Auto-constrain (`AUTOCONSTRAIN`) | Applies geometric constraints to a selection where the geometry already satisfies a relationship within a distance and angle tolerance. The types considered, their priority and the tolerances are settings; the command reports how many constraints of each type were added. | T2 | 2 |
| DB-CNSTR-004 | Constraint inference while drawing | When inference is on, coincident, horizontal, vertical, parallel, perpendicular and tangent constraints are created from the object snaps and tracking used while drawing and editing. Inference is toggled from the status bar and a keyboard shortcut. | T2 | 2 |
| DB-CNSTR-005 | Constraint bars (`CONSTRAINTBAR`) | Each constrained object shows a bar of constraint icons that can be shown, hidden or toggled for a selection or all objects. Hovering an icon highlights the constrained objects; deleting an icon deletes the constraint. | T2 | 2 |
| DB-CNSTR-006 | Delete constraints (`DELCONSTRAINT`) | Removes all geometric and dimensional constraints from the selected objects in one undoable step and reports the count removed. Geometry keeps its current position. | T2 | 2 |
| DB-CNSTR-007 | Constraint settings (`CONSTRAINTSETTINGS`) | Settings control which constraint types appear in bars, bar transparency, whether bars show after applying, inference, auto-constrain tolerances and priorities, and dimensional-constraint label format (name, value, or name and expression). | T2 | 2 |

## Dimensional constraints

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-CNSTR-008 | Dimensional constraint types (`DIMCONSTRAINT`) | Linear (horizontal, vertical, aligned), radial, diameter and angular constraints are placed by picking objects or points. Each gets a unique name and a value or expression; changing it moves geometry according to the solver. | T2 | 2 |
| DB-CNSTR-009 | Names and expressions | A dimensional constraint value can reference other constraints and parameters by name with arithmetic, trigonometric, rounding, minimum/maximum and unit-aware functions. Circular references are refused with the cycle shown; renaming a parameter updates every expression that uses it. | T2 | 2 |
| DB-CNSTR-010 | Dynamic and annotational forms | A dimensional constraint is either dynamic (displayed at constant screen size, never plotted, hidden when constraints are hidden) or annotational (formatted by a dimension style and plotted). The form can be switched per constraint without changing its value. | T2 | 2 |
| DB-CNSTR-011 | Convert dimension to constraint | An associative dimension can be converted into a dimensional constraint that keeps its measured value, position and style. Non-associative dimensions are refused with a message. | T2 | 2 |
| DB-CNSTR-012 | Reference constraints | A dimensional constraint can be marked as reference: it measures and reports a driven value usable in expressions but does not drive geometry, so it never causes over-constraint. | T2 | 2 |

## Parameter table

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-CNSTR-013 | Parameter table (`PARAMETERS`) | A panel lists dimensional constraints, reference parameters and user parameters with name, expression, value and unit; values and expressions are editable in place and edits re-solve the drawing in one transaction. The table offers a non-interactive form for scripts and the automation protocol. | T2 | 2 |
| DB-CNSTR-014 | User parameters | User parameters hold values or expressions that are not attached to geometry and can be used by any constraint expression. Deleting a parameter in use is refused with the list of dependants. | T2 | 2 |
| DB-CNSTR-015 | Filters and groups | Parameters can be filtered by name pattern or by whether they are used and organized into named groups; filters and groups are stored in the drawing. | T3 | 2 |
| DB-CNSTR-016 | Expression functions | Expressions support the four operators, powers, parentheses, constants (such as pi), trigonometric and inverse functions in degrees or radians, square root, logarithms, absolute value, rounding, minimum and maximum, and unit conversion. Evaluation uses the shared `arc-solve` expression engine. | T2 | 2 |

## Solving

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-CNSTR-017 | Constraint-aware editing | Grip edits, `MOVE`, `STRETCH`, `ROTATE`, `SCALE` and property edits on constrained geometry re-solve the network so that every constraint stays satisfied, with the preview showing the solved result. An edit that cannot be satisfied is refused and the drawing is unchanged. | T2 | 2 |
| DB-CNSTR-018 | Over-constraint detection | Adding a constraint that is redundant or contradictory is rejected with an explanation naming the constraints involved and the option to add it as a reference constraint instead. | T2 | 2 |
| DB-CNSTR-019 | Degrees-of-freedom feedback | Each constrained object can show its remaining degrees of freedom by color and a count in the properties panel, and a command highlights under-constrained objects and the motions still free. A fully constrained network is reported as such. | T3 | 2 |
| DB-CNSTR-020 | Solver behavior and relaxation | The `arc-solve` solver minimizes movement from the current position, keeps fixed objects in place and solves independent sub-networks in parallel. Settings control whether constraints relax (are removed with notice) when geometry is edited by a command that cannot honor them, such as `EXPLODE` or `TRIM`. | T2 | 2 |
| DB-CNSTR-021 | Deterministic results | The same network and edit produce the same solved geometry on every platform and in headless runs, so scripts and comparisons are reproducible. | T3 | 2 |
| DB-CNSTR-022 | Constraints in block definitions | Constraints and parameters inside a block definition drive the block's adjustable properties; the authoring and testing of such blocks are covered by the block capabilities. | T2 | 2 |
| DB-CNSTR-023 | Storage and preservation | Constraints and parameters are written to DWG in the dictionary structures the format defines for them, so other applications that support constraints can read them. Constraint data DraftBuilder cannot interpret is preserved unchanged and reported. | T2 | 2 |

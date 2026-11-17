# Dimensions, leaders and tolerances

Dimensions measure geometry and stay associated with it; leaders and tolerance frames attach notes
and inspection data to the drawing. Every dimension is styled, overridable per object and
regenerated through the dependency graph when the geometry it measures changes.

## Domain objects

- **Dimension** — linear, aligned, rotated, angular, radial, diameter, jogged radial, arc length or ordinate measurement with definition points and display geometry.
- **Dimension style** — a named set of line, arrow, text, fit, unit, alternate-unit and tolerance settings, with optional sub-styles per dimension type.
- **Dimension association** — the link from a dimension's definition points to points on geometry.
- **Multileader** — one or more leader lines with a landing and content (paragraph text, block or none), styled by a multileader style.
- **Feature control frame** — a geometric tolerance frame with characteristic symbol, tolerance values, material modifiers and datums.
- **Center mark and centerline** — associative annotation objects for circles, arcs and pairs of lines.

## Creating dimensions

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DIM-001 | Smart dimension (`DIM`) | Hovering an object previews the fitting dimension type (linear, aligned, angular, radial, diameter, arc length or ordinate) and a click places it; within the command the user can switch to baseline, continue, ordinate, align and distribute modes and choose the target layer. | T1 | 1 |
| DB-DIM-002 | Linear and aligned (`DIMLINEAR`, `DIMALIGNED`) | Dimensions the distance between two points or along an object, horizontally, vertically, rotated or parallel to the measured segment, with text and angle options before placement. | T1 | 1 |
| DB-DIM-003 | Angular (`DIMANGULAR`) | Dimensions the angle between two lines, of an arc, of part of a circle or through three points, with the quadrant chosen by cursor position. | T1 | 1 |
| DB-DIM-004 | Radial, diameter and jogged (`DIMRADIUS`, `DIMDIAMETER`, `DIMJOGGED`) | Dimensions arcs and circles with optional center mark or centerlines; the jogged form uses an overridden center location for large radii. | T1 | 1 |
| DB-DIM-005 | Arc length (`DIMARC`) | Dimensions the length of an arc or polyline arc segment, whole or partial, with the arc-length symbol before or above the text or omitted. | T2 | 1 |
| DB-DIM-006 | Ordinate (`DIMORDINATE`) | Places X or Y datum dimensions measured from the current coordinate system origin, with the leader jogging as needed. | T2 | 1 |
| DB-DIM-007 | Baseline and continue (`DIMBASELINE`, `DIMCONTINUE`) | Chains new dimensions from the last or a selected dimension, sharing a baseline at the style's spacing or continuing end to end. | T1 | 1 |
| DB-DIM-008 | Quick dimension (`QDIM`) | Dimensions all selected geometry at once as continuous, staggered, baseline, ordinate, radius or diameter sets, with a selectable datum point and later editing of the set. | T2 | 1 |
| DB-DIM-009 | Isometric dimensions | Obliques extension lines and rotates text so linear dimensions align with the isometric planes. | T2 | 1 |
| DB-DIM-010 | Dimension layer | New dimensions go to a designated layer regardless of the current layer when the setting is used. | T2 | 1 |

## Editing and arranging

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DIM-011 | Space and align (`DIMSPACE`) | Evenly spaces parallel linear or angular dimensions from a base dimension, by a value or automatically, and aligns them when the spacing is zero. | T2 | 1 |
| DB-DIM-012 | Dimension break (`DIMBREAK`) | Breaks dimension and extension lines where they cross other objects, automatically or between two picked points; automatic breaks update when crossing objects move, and breaks can be removed. | T2 | 1 |
| DB-DIM-013 | Jog line (`DIMJOGLINE`) | Adds or removes a jog symbol on a linear dimension line to show a non-true-length value. | T2 | 1 |
| DB-DIM-014 | Inspection dimension (`DIMINSPECT`) | Adds an inspection frame with a label and inspection rate to a dimension, in round, angular or no frame shape. | T3 | 1 |
| DB-DIM-015 | Text and line edits (`DIMEDIT`, `DIMTEDIT`) | Replaces or restores dimension text, rotates it, moves it left, center or right or to a picked position, and obliques extension lines; grips on text and dimension line do the same interactively. | T1 | 1 |
| DB-DIM-016 | Dimension text content | The measured value appears through a placeholder token that can be combined with prefix, suffix, fields and paragraph-text formatting; overridden text is flagged so it can be found. | T1 | 1 |
| DB-DIM-017 | Fit and placement rules | When text and arrows do not fit between extension lines, the style decides what moves outside, whether a leader is drawn, and whether the dimension line is forced between extension lines. | T1 | 1 |

## Associativity

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DIM-018 | Associative dimensions | Dimensions created on object snap points associate with the geometry; moving, stretching or scaling the geometry updates the dimension in the same transaction, including dimensions in paper space measuring through a viewport. | T1 | 1 |
| DB-DIM-019 | Association management (`DIMREASSOCIATE`, `DIMDISASSOCIATE`, `DIMREGEN`) | Re-associates definition points to new geometry, removes association, and refreshes display of dimensions after view or reference changes. | T1 | 1 |
| DB-DIM-020 | Association modes | A drawing setting creates dimensions as exploded geometry, as non-associative dimension objects, or as associative dimensions. | T2 | 1 |
| DB-DIM-021 | Lost-association monitor | Flags dimensions that lost their association with a marker in the drawing and a list, so they can be reassociated before issue. | T2 | 1 |

## Styles and units

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DIM-022 | Dimension styles (`DIMSTYLE`) | Defines named styles for lines, arrows, text, fit, primary units, alternate units and tolerances, sets the current style, renames styles and applies a style to selected dimensions. | T1 | 1 |
| DB-DIM-023 | Sub-styles | A style may hold sub-styles for linear, angular, radial, diameter, ordinate and leader dimensions that override the parent for that type only. | T2 | 1 |
| DB-DIM-024 | Overrides and comparison | Per-dimension overrides show in the properties panel and can be saved back to the style or cleared; a comparison lists every setting that differs between two styles or between a style and its overrides. | T1 | 1 |
| DB-DIM-025 | Unit formats | Primary values format as decimal, engineering, architectural, fractional or scientific, with precision, rounding, decimal separator, leading and trailing zero suppression, a linear scale factor and an automatic factor for dimensions placed in paper space through viewports. | T1 | 1 |
| DB-DIM-026 | Alternate units | Shows a second value in other units with its own multiplier, format, precision and placement after or below the primary value. | T2 | 1 |
| DB-DIM-027 | Arrowheads | Uses a library of standard terminators per end and per leader, or a block named as a custom arrowhead, scaled with the style. | T1 | 1 |
| DB-DIM-028 | Overall scale | A style's overall scale or annotation scaling (see DB-PLOT) sizes arrows, text and gaps for the plot scale. | T1 | 1 |

## Tolerances

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DIM-029 | Dimensional tolerances | Displays symmetric, deviation, limits or basic tolerances with their own precision, height scale, vertical justification and zero suppression. | T2 | 1 |
| DB-DIM-030 | Feature control frames (`TOLERANCE`) | Creates geometric tolerance frames with characteristic symbols, diameter symbols, tolerance values, material condition modifiers, up to three datums with modifiers, projected tolerance zones and datum identifiers; frames can attach to leaders. | T2 | 1 |

## Leaders and center marks

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DIM-031 | Multileaders (`MLEADER`) | Creates leaders with straight or spline lines or none, a landing of set length, and paragraph text, block (with attributes) or no content; arrowheads come from the style, and placement order (arrow first, landing first or content first) is selectable. | T1 | 1 |
| DB-DIM-032 | Multileader styles (`MLEADERSTYLE`) | Named styles define leader format, arrow, landing, content type, text style and attachment, block content and scale; editing a style updates every multileader using it. | T1 | 1 |
| DB-DIM-033 | Multileader editing (`MLEADEREDIT`, `MLEADERALIGN`, `MLEADERCOLLECT`) | Adds or removes leader lines on an existing multileader, aligns and spaces multileaders along a line, and collects block-content multileaders into one leader with a horizontal or vertical stack. | T2 | 1 |
| DB-DIM-034 | Legacy leaders (`LEADER`, `QLEADER`) | Reads, displays, edits and creates the older leader object with text, tolerance, block or copied content, so existing drawings and scripts keep working. | T2 | 1 |
| DB-DIM-035 | Center marks and centerlines (`CENTERMARK`, `CENTERLINE`) | Creates associative center marks on circles and arcs and centerlines between two lines; they follow geometry edits, use their own layer and linetype settings, and extend by grip or by setting. | T2 | 1 |

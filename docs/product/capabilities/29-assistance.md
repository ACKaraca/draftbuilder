# Assistance

This area specifies what the optional assistant does in DraftBuilder: running commands from a
described intent, answering questions about a drawing, bringing drawings to a standard, proposing
dimensions and annotation, recognizing blocks, authoring and explaining routines, converting PDF
and raster content, and filling sheet data, always as previewed commands checked by the product's
own audits and standards checks. The suite rules for levels, grounding, data handling, provenance
and evaluation are in
[assistance](https://github.com/ACKaraca/arc-suite/blob/main/docs/architecture/assistance.md); the
panel is [DB-AUTO-037](18-customization-automation.md) and the protocol endpoints are
[DB-AUTO-027 and DB-AUTO-028](18-customization-automation.md). Every capability here also has a
manual and scripted path.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Assistance request | Text, level, scope (selection, layers, layout, drawings), protected objects, base revision | Produces answers or proposals |
| Proposal | Items, command invocations, change summary, checker findings, citations | Bound to a base revision; applied as one undo group |
| Layer mapping proposal | Source layer, target layer, reason, object count, confidence | Becomes a layer translation the user saves with the standards file |
| Routine draft | Language (script, Lisp dialect, Python), source, explanation, static-check result, dry-run result | Saved as a file or package only by the user |
| Provenance mark | Origin, proposal, revision | On objects created or changed by an applied proposal until reviewed |

## Commands and questions

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-ASSIST-001 | Command by intent | A request such as "offset these walls 20 inward and trim the corners" becomes a proposal of DraftBuilder commands on the selection with resolved parameters in drawing units; the preview shows the resulting geometry, and gaps and overlaps introduced by the edit are reported by the geometry cleanup check (DB-MOD-033). | T3 | 2 |
| DB-ASSIST-002 | Drawing questions | The assistant answers questions about the drawing ("how many doors on level 1", "total length of the pipes on layer P-WATER", "areas of the closed boundaries on layer A-ROOM") from queries, block counts (DB-BLOCK-032) and computed properties, and cites the objects so they select in the drawing; numbers come from the queries. | T2 | 2 |
| DB-ASSIST-003 | Command tutor | When a user types an unknown command name or describes a habit from other drafting tools, the assistant names the DraftBuilder command, explains its prompts and options from the help, starts it on request, and can propose a user alias (DB-CMD) that the user saves. | T2 | 2 |
| DB-ASSIST-004 | Data extraction by question | From a question about the drawing's contents, the assistant proposes a data extraction definition (DB-TEXT-036) with the objects, properties and grouping, previewed as the resulting table that the user places or exports. | T3 | 2 |

## Standards and cleanup

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-ASSIST-005 | Standards conformance | Against the standards files associated with the drawing (DB-INTEROP-024), the assistant runs the standards check (DB-INTEROP-025), groups the findings, and proposes the fixes as a diff per object: layer, linetype, lineweight, text style and dimension style changes; the standards check runs again on the preview. | T3 | 2 |
| DB-ASSIST-006 | Layer mapping | For drawings with nonstandard layer names in any language, the assistant proposes a mapping from each layer to a layer of the project standard, using names, content and object types, with a reason and object count per row; the user edits the table before it applies, and it can be saved as a translation for batch use (DB-INTEROP-027). | T3 | 2 |
| DB-ASSIST-007 | Drawing cleanup | The assistant runs audit, purge and geometry cleanup (DB-DBASE-033, DB-DBASE-035, DB-MOD-033), explains each defect group (duplicates, overlaps, zero-length objects, unreferenced definitions, gaps), and proposes the cleanup with counts per group; the same checks run on the preview. | T3 | 2 |
| DB-ASSIST-008 | Annotation consistency | The assistant lists dimensions whose overridden text disagrees with the measured value (DB-DIM-016), text with heights or styles outside the standard for its scale, misspellings (DB-TEXT-027) and lost dimension associations (DB-DIM-021), and proposes corrections that the user accepts per item. | T3 | 2 |

## Annotation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-ASSIST-009 | Dimensioning proposals | For selected geometry or a layout viewport, the assistant proposes associative dimension chains (DB-DIM-007, DB-DIM-018) following the project's dimensioning conventions (overall, openings, grid) in the dimension style for the viewport's scale; overlap and association checks run on the preview, and the user accepts chains individually. | T3 | 2 |
| DB-ASSIST-010 | Labels and leaders | The assistant proposes labels, leaders and attribute-driven tags for chosen objects (for example room names and areas from closed boundaries), placed to avoid overlap, with area values taken from the computed boundary. | T3 | 2 |

## Blocks and attributes

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-ASSIST-011 | Block recognition and replacement | Repeated geometry detection (DB-BLOCK-033) finds groups of identical linework; the assistant names and classifies them, proposes block definitions or matching library blocks, and proposes the replacement with counts before and after; geometry matching within the detection tolerance is checked for every replaced group. | T3 | 2 |
| DB-ASSIST-012 | Attribute filling | From a table, a description or another drawing's attributes, the assistant proposes attribute values for selected block references, showing current and proposed values per reference before apply. | T3 | 2 |

## Sheets

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-ASSIST-013 | Title block and sheet set data | The assistant proposes sheet numbers, titles and sheet set properties (DB-PLOT-042) from the layouts' contents and the project's numbering standard, and fills title-block fields (DB-TEXT-022) from them; field evaluation runs on the preview. | T3 | 2 |
| DB-ASSIST-014 | Publish setup | From a request such as "all architectural sheets as one PDF at the sheet scales", the assistant proposes the page setups and publish list (DB-PLOT-045), previewed as the list of sheets with paper size and scale. | T3 | 2 |

## Routines

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-ASSIST-015 | Routine authoring | The assistant writes a script, Lisp-dialect routine (DB-AUTO-015) or Python script for a described task, shows it as source with an explanation, runs the interpreter's static check, and runs it as a dry run on a copy of the drawing (DB-AUTO-031) to show the intended changes; the user saves and runs it. | T3 | 2 |
| DB-ASSIST-016 | Routine explanation and migration | For an existing routine or script, the assistant explains what it does line by line, lists functions the compatibility layer does not support (DB-AUTO-012), and proposes a rewrite that the static check and a dry run validate. | T3 | 2 |

## Conversion

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-ASSIST-017 | PDF content reconstruction | After a PDF import (DB-XREF-027), the assistant proposes joining segments into polylines, rebuilding text from glyph outlines, recovering hatches, assigning layers by line weight and color, and scaling from a dimension the user confirms; geometry cleanup checks the result. | T3 | 2 |
| DB-ASSIST-018 | Raster to drawing | On a scanned drawing, automatic vectorization and text recognition (DB-TRAST-027, DB-TRAST-032) produce linework and text; the assistant proposes layers, corrected text and dimension values with the source region of each, for review item by item. | T3 | 4 |
| DB-ASSIST-019 | Sketch to drawing | From a photograph of a hand sketch with written dimensions, the assistant proposes a dimensioned drawing whose dimensions match the written values, with any value it could not read left for the user. | T3 | 5 |

## Review

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-ASSIST-020 | Comparison explained | After a drawing comparison (DB-COLLAB-001), the assistant describes the changes in words grouped by area, layer and kind ("the stair core moved 300 to the east; four doors were added on level 2"), each statement citing the compared objects. | T3 | 2 |
| DB-ASSIST-021 | Markups to edits | For markups attached to a drawing (from Arc Hub or an imported PDF review), the assistant proposes the edits each markup asks for, linked to the markup, and marks the markup resolved only when the user applies the edit. | T3 | 3 |
| DB-ASSIST-022 | Linework to toolset objects | In a toolset project (DB-TSF), the assistant proposes conversion of plain linework and blocks into typed objects such as walls, doors and equipment, with the source objects of each, checked by the toolset's object audit (DB-TSF-014). | T3 | 4 |
| DB-ASSIST-023 | Assisted content review | Objects carrying a provenance mark show a distinct overlay and filter; the user reviews and clears marks, and plotting or publishing reports unreviewed objects on the published sheets before it proceeds. | T3 | 2 |

## Readiness

Readiness classes are defined in
[assistance](https://github.com/ACKaraca/arc-suite/blob/main/docs/architecture/assistance.md#readiness-classes).

| Class | ID | Checked by | Depends on |
|---|---|---|---|
| R1 | DB-ASSIST-001 | Command validation, geometry cleanup check | DB-AUTO-027, DB-AUTO-037, DB-MOD-033 |
| R1 | DB-ASSIST-002 | Queries, block count | DB-BLOCK-032, DB-AUTO-028 |
| R1 | DB-ASSIST-003 | Command catalog | Help and command catalog in both languages |
| R1 | DB-ASSIST-004 | Extraction preview | DB-TEXT-036 |
| R1 | DB-ASSIST-005 | Standards check | DB-INTEROP-024, DB-INTEROP-025 |
| R1 | DB-ASSIST-006 | Standards check | DB-INTEROP-027, DB-LAYER-026 |
| R1 | DB-ASSIST-007 | Audit, purge, geometry cleanup | DB-DBASE-033, DB-DBASE-035, DB-MOD-033 |
| R1 | DB-ASSIST-008 | Measured dimension values, spell check, association monitor | DB-DIM-016, DB-DIM-021, DB-TEXT-027 |
| R1 | DB-ASSIST-009 | Association and overlap checks | DB-DIM-007, DB-DIM-018 |
| R1 | DB-ASSIST-010 | Overlap check, computed areas | Leader and text commands |
| R2 | DB-ASSIST-011 | Geometry matching | DB-BLOCK-033, a block library |
| R1 | DB-ASSIST-012 | Attribute definitions | Block attribute commands |
| R1 | DB-ASSIST-013 | Field evaluation | DB-PLOT-042, DB-TEXT-022 |
| R1 | DB-ASSIST-014 | Publish list preview | DB-PLOT-045 |
| R1 | DB-ASSIST-015 | Static check, dry run | DB-AUTO-015, DB-AUTO-031 |
| R1 | DB-ASSIST-016 | Static check, dry run | DB-AUTO-012, DB-AUTO-015 |
| R2 | DB-ASSIST-017 | Geometry cleanup check | DB-XREF-027, DB-MOD-033 |
| R2 | DB-ASSIST-018 | Source region comparison | DB-TRAST-027, DB-TRAST-032 |
| R3 | DB-ASSIST-019 | Dimension consistency | Image input in the provider |
| R1 | DB-ASSIST-020 | Drawing comparison | DB-COLLAB-001 |
| R2 | DB-ASSIST-021 | Drawing comparison after apply | Markup objects and Arc Hub review |
| R2 | DB-ASSIST-022 | Toolset object audit | DB-TSF, DB-TSF-014 |
| R1 | DB-ASSIST-023 | Provenance query | Provenance marks in `arc-core` |

## Evaluation

The assistance task suite is built from the [reference workflows](../workflows.md): W01 and W02
(DB-ASSIST-001, DB-ASSIST-009, DB-ASSIST-010), W03 (DB-ASSIST-012), W04 and W08 (DB-ASSIST-013,
DB-ASSIST-014), W07 (DB-ASSIST-002, DB-ASSIST-004), W09 (DB-ASSIST-015, DB-ASSIST-016), W10
(DB-ASSIST-005 to DB-ASSIST-008, DB-ASSIST-011) and W11 (DB-ASSIST-020, DB-ASSIST-021). Thresholds
and the first offered interactions are settled under [OQ-19](../open-questions.md).

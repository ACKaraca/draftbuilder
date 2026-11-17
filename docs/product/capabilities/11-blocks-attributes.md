# Blocks, attributes and dynamic blocks

Blocks define reusable content once and place it many times; attributes attach per-instance data to
each placement; dynamic blocks add parameters and actions so one definition covers many variants.
Block definitions, attribute values and dynamic block state are kept exactly through DWG and DXF.

## Domain objects

- **Block definition** — a named set of entities with a base point, units, description and behavior flags.
- **Block reference** — a placement of a definition with insertion point, scale, rotation and attribute values; a grid reference repeats it in rows and columns.
- **Attribute definition and attribute** — a tagged, prompted field in a definition and its per-reference value.
- **Dynamic block definition** — a definition with parameters, actions, parameter sets, visibility states, lookup tables and property tables.
- **Dynamic block reference** — a reference with current parameter values and the anonymous definition that represents its current geometry.
- **Block library** — a folder or drawing of block definitions with version records.

## Defining and placing blocks

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-BLOCK-001 | Define block (`BLOCK`, `-BLOCK`) | Defines a block from selected objects and a base point; the source objects are kept, converted to a reference or deleted; flags set annotation scaling, orientation to layout, uniform scaling and whether exploding is allowed; units, description and hyperlink are stored. | T1 | 1 |
| DB-BLOCK-002 | Insert block (`INSERT`) | Places blocks from a block panel with current-drawing, recent, favorite and library views, by click or drag, with insertion point, scale, rotation, repeated placement and optional explode on insert. | T1 | 1 |
| DB-BLOCK-003 | Command-line insert (`-INSERT`) | Inserts by prompts accepting the conventional option keywords for base point, scale per axis, rotation and preview values; a name prefixed for explode inserts the parts, and a name with a file assignment redefines the block from that file. | T1 | 1 |
| DB-BLOCK-004 | Grid insert (`MINSERT`) | Places one reference repeated in rows and columns at given spacing as a single object. | T2 | 1 |
| DB-BLOCK-005 | Write block (`WBLOCK`) | Writes selected objects, a block definition or the whole drawing to a new DWG or DXF with its dependent definitions and chosen units; writing the whole drawing omits unreferenced definitions. | T1 | 1 |
| DB-BLOCK-006 | Drawings as blocks | Inserting any drawing file creates a block from its model space with its layers, styles and nested definitions; name conflicts keep the existing definition unless redefinition is requested. | T1 | 1 |
| DB-BLOCK-007 | Nesting and redefinition | Blocks nest to any depth with cycle detection; redefining a definition updates every reference in the same transaction, and nested references follow. | T1 | 1 |
| DB-BLOCK-008 | Property inheritance | Content on layer 0 takes the reference's layer, and ByBlock properties take the reference's color, linetype, lineweight and transparency, resolved level by level for nested blocks. | T1 | 1 |
| DB-BLOCK-009 | Annotative blocks | A block flagged for annotation scaling sizes its references by the current annotation scale (see DB-PLOT). | T1 | 1 |
| DB-BLOCK-010 | Content browser | Browses other drawings and library folders for blocks, layers, linetypes, text, dimension, table and multileader styles, layouts and external references, with preview, and copies chosen items into the current drawing by drag or command. | T2 | 1 |
| DB-BLOCK-011 | Explode attributes to text (`BURST`) | Explodes block references and converts their attribute values to single-line text with matching appearance instead of attribute definitions. | T2 | 1 |

## Attributes

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-BLOCK-012 | Attribute definition (`ATTDEF`) | Defines tag, prompt, default value (which may contain fields) and text settings, with invisible, constant, verify, preset, locked-position and multiline modes and annotation scaling. | T1 | 1 |
| DB-BLOCK-013 | Attribute values on insert | Inserting a block asks for attribute values in a panel or at the command line according to a setting, or uses defaults when prompting is off; verify mode asks twice. | T1 | 1 |
| DB-BLOCK-014 | Edit attribute values (`ATTEDIT`, `EATTEDIT`, `ATTIPEDIT`) | Edits values per reference in a panel or in place; double-clicking an attributed reference opens the editor. | T1 | 1 |
| DB-BLOCK-015 | Global attribute edit (`-ATTEDIT`) | Changes values, position, height, angle, style, layer or color across many references filtered by block name, tag and value, with wildcard matching. | T2 | 1 |
| DB-BLOCK-016 | Attribute definition panel | Lists attribute definitions of a block for reordering, editing modes and text properties, and removing; applying the changes synchronizes existing references. | T2 | 1 |
| DB-BLOCK-017 | Synchronize references (`ATTSYNC`, `ATTREDEF`) | Updates references to match changed attribute definitions, keeping existing values for tags that remain and adding defaults for new tags. | T1 | 1 |
| DB-BLOCK-018 | Attribute display (`ATTDISP`) | Shows attributes per their own visibility, shows all including invisible ones, or hides all. | T1 | 1 |
| DB-BLOCK-019 | Attribute export and import | Exports attribute values of selected references to a CSV or XLSX table keyed by handle and reads edited values back, reporting rows whose references no longer exist. | T2 | 2 |

## In-place editing

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-BLOCK-020 | Edit reference in place (`REFEDIT`, `REFSET`, `REFCLOSE`) | Opens a block or external reference for editing in the host drawing, with a working set that objects can be added to or removed from and other geometry faded; closing saves changes back to the definition or source drawing, or discards them. | T2 | 1 |

## Block authoring mode

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-BLOCK-021 | Block authoring mode (`BEDIT`, `BSAVE`, `BSAVEAS`) | Opens a definition in a separate editing environment with its own panels for parameters, actions, parameter sets and constraints; saving updates all references, and save-as creates a new definition. | T2 | 2 |
| DB-BLOCK-022 | Parameters | Adds point, linear, polar, XY, rotation, alignment, flip, visibility, lookup and base-point parameters, each with value sets (list or increment with limits) and grip count. | T2 | 2 |
| DB-BLOCK-023 | Actions | Adds move, scale, stretch, polar stretch, rotate, flip, array and lookup actions bound to parameters and to selection sets, with stretch frames and distance multipliers. | T2 | 2 |
| DB-BLOCK-024 | Parameter sets | Adds common parameter and action pairs in one step, with the selection set assigned afterward. | T3 | 2 |
| DB-BLOCK-025 | Visibility states | Defines named states that show or hide chosen geometry; references switch states from a grip or property. | T2 | 2 |
| DB-BLOCK-026 | Lookup and property tables | Lookup tables map parameter values to named property values in both directions; block property tables drive several parameters from rows of a size table, and invalid combinations are refused. | T2 | 2 |
| DB-BLOCK-027 | Constraints in definitions | Geometric and dimensional constraints in a definition drive geometry from parameters (see DB-CNSTR), solved when references change. | T2 | 2 |
| DB-BLOCK-028 | Test mode (`BTESTBLOCK`) | Opens a temporary window to try parameters, grips and states of an unsaved definition without changing the drawing. | T2 | 2 |

## Dynamic block references

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-BLOCK-029 | Display and preservation | Dynamic block references read from DWG or DXF display in their saved state, and their parameter data and anonymous representations are preserved exactly on save even before evaluation is available. | T1 | 1 |
| DB-BLOCK-030 | Custom grips and properties | Selected references show custom grips per parameter and list parameter values in the properties panel; changing a value evaluates actions and regenerates the anonymous representation in one transaction. | T2 | 2 |
| DB-BLOCK-031 | Reset and convert | A reference can be reset to definition defaults, and its current state can be converted to a new static definition. | T2 | 2 |

## Counting, detection and libraries

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-BLOCK-032 | Count (`COUNT`) | Counts references by block name and dynamic state, or counts identical geometry, in the selection or whole drawing; counted items highlight, counts show as labels, and a count table updates when items are added or removed. | T2 | 1 |
| DB-BLOCK-033 | Repeated geometry detection | Finds groups of loose objects that match a chosen sample or each other within a tolerance, using a deterministic algorithm that gives the same result for the same input; each candidate is listed with its location and transform. | T3 | 2 |
| DB-BLOCK-034 | Convert to block | Converts confirmed candidates, one case or all at once, into references of a new or existing definition; a report records every replaced object handle and the resulting reference, and the whole conversion is one undoable transaction. | T3 | 2 |
| DB-BLOCK-035 | Replace block | Replaces all or selected references of one definition with another, keeping insertion, scale, rotation and matching attribute values. | T2 | 1 |
| DB-BLOCK-036 | Library versioning | Library blocks carry a version record; references remember the library and version they came from, the drawing lists references whose library version is newer, and updating is explicit and reported. | T3 | 2 |

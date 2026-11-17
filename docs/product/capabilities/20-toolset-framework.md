# Toolset framework

This area specifies what every DraftBuilder toolset shares: typed domain objects stored in the drawing database, styles and display representations, property sets and classifications, schedules and tags, project structure across drawing files, content catalogs and tool palettes, standards synchronization, and the rules for degrading toolset objects to plain entities when a drawing leaves DraftBuilder. Toolsets are optional domain packs; a drawing records which ones it uses, and a drawing never becomes unreadable because a toolset is absent.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Toolset module | Identifier, version, declared object classes, declared dependencies on other modules | Registers object classes, styles, commands, palettes and catalogs; enabled per drawing |
| Object class | Class name, schema version, parameter schema, representations, grips, context actions | Instantiated as typed objects; owned by one toolset module |
| Typed object | Stable ID, drawing handle, class, parameters, style reference, overrides, property sets, cached graphics | Instance of a class; references a style; hosts or is hosted by other typed objects; attached to tags and schedules |
| Style | Name, class, property bundle, component list, default property sets | Referenced by typed objects; stored in a drawing style table; sourced from a library or standards drawing |
| Display representation | Name, view direction, detail level, display components (layer, color, linetype, lineweight, visibility, hatch) | Belongs to a class; grouped into display sets |
| Display set | Name, representations enabled per class | Selected by a display configuration |
| Display configuration | Name, display set per view direction and detail level, cut plane height range | Assigned to a model-space view or a layout viewport |
| Material | Name, hatch, color, render mapping, classification | Referenced by style components |
| Property set definition | Name, applicable classes, typed property list, source kind per property | Attached to objects, styles, classes or plain entities; read by schedules and tags |
| Classification system | Name, edition, hierarchical code table | Assigned to objects or styles; mapped to export schemas |
| Schedule definition | Source scope, classes, columns, sort, group, filter, totals, format | Produces schedule tables; reads property sets |
| Schedule table | Definition reference, row-to-object map, refresh state | A table entity in a drawing bound to a definition |
| Tag | Target object ID, tag style, displayed properties, leader | Follows its target; annotative |
| Project | Name, member list, levels, divisions, shared settings, standards source | Contains member drawings; stored as a sidecar document package |
| Project member | Relative path, kind (model, element, view, sheet), level, division, status | Belongs to a project; referenced by external references and sheets |
| Catalog | Name, source layer (user, project, shared), items, size tables | Supplies items to browsers and palettes; instances record item ID and revision |
| Tool palette | Name, tools (class, style, preset properties, catalog item) | Belongs to a toolset module; switches with the workspace |
| Standards source | A drawing or library holding styles, layers, display settings, property sets | Compared against and synchronized into drawings |

## Typed objects and the drawing database

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TSF-001 | Typed object storage | Stores each toolset object in the drawing database as a registered class with a class name, schema version and parameter payload, addressed by drawing handle and stable ID. Objects take part in selection, grips, undo, copy, blocks and external references like any other entity. | T1 | 4 |
| DB-TSF-002 | Object class registry | Each toolset registers its classes with parameter schemas, representations, grips and context actions; the registry lists the classes and versions present in the open drawing. A class that is not registered is handled as preserved foreign data. | T1 | 4 |
| DB-TSF-003 | Schema versioning and migration | Every class payload carries a schema version. A drawing with an older version migrates in memory and reports what changed; a newer minor version opens with unknown fields preserved and written back unchanged. | T1 | 4 |
| DB-TSF-004 | Unknown object preservation | Class data, extension dictionaries and cached graphics that no installed toolset understands are kept byte-identical and written back on save. Opening such a drawing lists the preserved classes and counts. | T1 | 2 |
| DB-TSF-005 | Typed associations | Stores relationships (host and hosted, boundary and space, system and member, tag and target) as references by stable ID with a declared rule per relationship type for copy, delete and move. A delete either removes, detaches or blocks according to that rule; no reference dangles. | T1 | 4 |
| DB-TSF-006 | Dependency-driven regeneration | Derives object geometry, joins and display graphics from parameters through the document dependency graph. An edit recomputes only dependents on worker threads, and a result computed from an older revision is discarded. | T1 | 4 |
| DB-TSF-007 | Object grips and context actions | Each class declares grips (move, stretch, flip, add vertex, offset) and a context action list. Every grip and action has a command-line equivalent with the same parameters. | T1 | 4 |
| DB-TSF-008 | Commands for every object operation | Creation, modification, conversion, update and reporting operations are typed commands with parameter schemas. Scripts, the automation protocol and the headless CLI run them without opening a dialog. | T1 | 4 |
| DB-TSF-009 | Object properties panel | Shows typed parameters grouped by category with unit-aware input, multi-selection editing and a marker that distinguishes a style value from an object override. A "reset to style" action clears overrides on the selection. | T1 | 4 |
| DB-TSF-010 | Convert plain geometry to objects | Converts selected lines, polylines, arcs, blocks or solids into a chosen class with a parameter mapping step. The source geometry is kept on a named layer or erased by choice, and entities that cannot convert are listed in a report. | T2 | 4 |
| DB-TSF-011 | Copy, mirror and array semantics | Copy, mirror, rotate and array preserve parameters, overrides and property data. Mirroring flips handedness-dependent parameters (door swing, stair run direction) by rule per class. | T1 | 4 |
| DB-TSF-012 | Objects in blocks and external references | Typed objects inside blocks and referenced drawings stay typed and queryable. Referenced objects are read-only for editing but available to schedules, tags and reports. | T2 | 4 |
| DB-TSF-013 | Unit-typed parameters | Stores object parameters as typed quantities with the drawing unit system, so a drawing in millimeters and a drawing in inches exchange objects without scaling errors. | T1 | 4 |
| DB-TSF-014 | Object audit | Checks every typed object for missing style references, broken associations, out-of-range parameters and orphaned tags, and repairs what has an unambiguous fix. The result is a report with a navigate-to-object action per finding. | T2 | 4 |

## Styles and display

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TSF-015 | Style tables | Stores named styles per class in the drawing. Changing a style updates every object that references it; creating, renaming, copying and deleting styles are undoable commands, and deleting a referenced style is blocked. | T1 | 4 |
| DB-TSF-016 | Override resolution order | Resolves every property in one fixed order: class default, style, object override. The properties panel shows the source of each value, and an override can be cleared per property or for the whole object. | T1 | 4 |
| DB-TSF-017 | Style libraries | Reads and writes style sets in a documented library file. Importing a style whose name exists offers keep, replace or rename, and reports every object that changes appearance. | T2 | 4 |
| DB-TSF-018 | Unused style purge | Lists styles, materials and property set definitions with no references and removes the selected ones. | T2 | 4 |
| DB-TSF-019 | Display representations | Each class defines representations by view direction (plan, reflected plan, side, section, model, symbolic) and each representation is a list of display components with layer, color, linetype, lineweight, visibility and hatch. Representations are data, edited without changing object parameters. | T1 | 4 |
| DB-TSF-020 | Display sets and configurations | A display configuration chooses a display set for each view direction and detail level, covering every class in the drawing. Switching configuration regenerates graphics only; object data is untouched. | T1 | 4 |
| DB-TSF-021 | Detail level selection | Selects low, medium or high detail for each object from the viewport scale or from an explicit setting in the configuration. Changing the annotation scale of a viewport changes the detail level that applies. | T1 | 4 |
| DB-TSF-022 | Per-viewport configuration | Assigns a display configuration to the model-space view and to each layout viewport independently, with viewport-level overrides for layer color and visibility. | T1 | 4 |
| DB-TSF-023 | Cut plane display | Applies a plan cut plane with a height range per configuration. Objects crossing the plane are drawn cut with their section representation; objects above or below follow the configuration rule (hidden, dashed, or not shown). | T1 | 4 |
| DB-TSF-024 | Display overrides at three levels | Allows display component overrides at configuration, style and object level, resolved in that order of increasing priority, with a report of every override that applies to a selected object. | T2 | 4 |
| DB-TSF-025 | Materials | Defines materials with hatch pattern, color, render mapping and classification, referenced by style components so a material change updates every dependent representation. | T1 | 4 |
| DB-TSF-026 | Display by property | Colors or hatches objects by a property value (system, classification code, renovation state, fire rating) using a rule table, shown as a display layer over the configuration and removable without touching data. | T2 | 4 |
| DB-TSF-027 | Display graphics cache | Caches generated display graphics per object, representation and detail level, and regenerates only invalidated entries so panning and plotting do not trigger regeneration. | T2 | 4 |

## Property sets and classifications

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TSF-028 | Property set definitions | Defines named property sets with typed properties (text, number with unit, boolean, list, date), applicable classes, default values and a source kind per property. | T1 | 4 |
| DB-TSF-029 | Property source kinds | Supports manual, automatic (read from an object parameter), formula (expression over other properties using the shared unit-aware expression engine), classification-derived, location-derived (containing space or level) and project-derived properties. Derived values recompute when their inputs change. | T1 | 4 |
| DB-TSF-030 | Property set attachment | Attaches a set to individual objects, to a style (all instances inherit) or to a class. Removing or migrating a set keeps existing values unless the user chooses to discard them. | T1 | 4 |
| DB-TSF-031 | Property data on plain entities | Attaches property sets to any entity or block reference, not only typed objects. The data is stored as extended data so it survives a save for other applications. | T2 | 4 |
| DB-TSF-032 | Classification systems | Holds hierarchical classification tables, imported from CSV or user-defined, and assigns codes to objects or styles. An object can carry codes from several systems; assigning a code that is not in the table is rejected. | T1 | 4 |
| DB-TSF-033 | Classification mapping | Maps codes between two classification systems with a many-to-one table and converts a drawing's assignments in one command, listing unmapped codes. | T3 | 5 |
| DB-TSF-034 | Property validation rules | Attaches rules to properties (required, numeric range, list membership, pattern) evaluated at edit time and by the audit command. Violations are listed with a navigate-to-object action. | T2 | 4 |
| DB-TSF-035 | Property mapping to IFC | Maps property sets and classification assignments to IFC property sets and classification references on IFC export through an editable mapping table. Unmapped properties are written as custom sets and reported. | T2 | 4 |
| DB-TSF-036 | Property search and select | Selects objects by property value across the drawing, a project or referenced drawings, and saves the query by name for reuse in schedules and display rules. | T2 | 4 |

## Schedules and tags

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TSF-037 | Schedule definitions | Defines a schedule from a source scope (selection, drawing, referenced drawings, whole project), a set of classes, columns drawn from properties, sort keys, grouping, filters, totals and number formats. | T1 | 4 |
| DB-TSF-038 | Schedule tables | Inserts a schedule as a table entity bound to its definition. Refresh rebuilds the rows from current objects and highlights changed cells until acknowledged; an option refreshes automatically on save. | T1 | 4 |
| DB-TSF-039 | Project-wide schedules | Aggregates objects across all member drawings of a project without opening them, reading object data from the drawing database directly. | T2 | 4 |
| DB-TSF-040 | Schedule export and import | Exports schedule rows to CSV and XLSX. Importing a file updates editable manual properties by object ID key and reports unmatched rows and read-only columns. | T2 | 4 |
| DB-TSF-041 | Edit in schedule | Editing a cell writes the underlying property through a command, so the change is undoable and journaled; read-only columns are visibly locked. | T2 | 4 |
| DB-TSF-042 | Schedule styles | Stores column headings, fonts, border and fill rules, unit formats and header layout in named schedule styles reusable across definitions. | T2 | 4 |
| DB-TSF-043 | Tags | Places annotation tags bound to a target object that display chosen properties through fields. A tag follows its target on move, is annotative, and is listed as orphaned if its target is deleted. | T1 | 4 |
| DB-TSF-044 | Batch tagging | Tags every object of selected classes visible in a view in one command, placing tags to avoid overlap with each other and with their targets, and skipping objects that already have a tag. | T2 | 4 |

## Project structure and drawing management

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TSF-045 | Project definition | Stores a project as a sidecar document package listing member drawings by relative path with kind, level and division, plus shared settings and the standards source. The package is readable without DraftBuilder. | T1 | 4 |
| DB-TSF-046 | Project browser | Shows the project as a tree of model drawings, shared element drawings, view drawings and sheets, with open, rename, move and "reference into current drawing" actions that update the project list. | T1 | 4 |
| DB-TSF-047 | Levels and divisions | Defines project levels with elevation and floor-to-floor height, and divisions for splitting a building or site. Member drawings are assigned a level and division, and view drawings compose the drawings of selected levels. | T1 | 4 |
| DB-TSF-048 | View composition | Builds a view drawing by referencing model drawings at their level elevation, applies a display configuration, and keeps the references live so edits in a model drawing appear in every view that uses it. | T1 | 4 |
| DB-TSF-049 | Relative paths and relinking | Stores references relative to the project root, detects missing members on open, and offers a relink command that matches by file identity before file name. | T1 | 4 |
| DB-TSF-050 | Project-wide update | Refreshes schedules, tags, cross-references and title block fields across all member drawings in one run, in the application or headless, with progress, cancel and a per-drawing report. | T1 | 4 |
| DB-TSF-051 | Project properties in title blocks | Project name, number, client, phase and per-drawing properties feed title block fields and sheet properties; editing a project property updates every affected sheet at the next project update. | T1 | 4 |
| DB-TSF-052 | Shared project settings | Holds units, tag formats, standards, layer rules and display configuration defaults at project level, applied to member drawings and overridable per drawing with the override flagged. | T2 | 4 |
| DB-TSF-053 | Project integrity check | Detects missing members, drawings not in the member list, mismatched standards, duplicate IDs and stale references, and repairs those with an unambiguous fix. | T2 | 4 |
| DB-TSF-054 | Project archive | Packs all members, referenced images and the project package into one archive with paths rewritten, and restores it to a new location with all references intact. | T2 | 4 |
| DB-TSF-055 | Sheet set linkage | Places views on sheets through the sheet set, keeping each sheet viewport tied to its source view drawing so renaming or moving a view updates its sheet references. | T2 | 4 |
| DB-TSF-056 | Multi-user project work | Opens members with drawing-level check-out through Arc Hub; a second user sees who holds a member and can open it read-only. Element-level borrowing follows the suite multi-user model when available. | T3 | 5 |
| DB-TSF-057 | Object-level drawing comparison | Compares two revisions of a drawing by stable object ID and reports added, deleted, moved and parameter-changed typed objects with property-level differences, drawn as a highlighted overlay. | T3 | 5 |

## Content catalogs and tool palettes

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TSF-058 | Open content catalogs | Stores catalog items (styles, parts, symbols, details, size tables) in a documented database file with metadata and tags. User, project and shared catalogs layer in a defined search order. | T1 | 4 |
| DB-TSF-059 | Catalog browser | Searches and filters items by text, classification, standard and manufacturer, shows a preview, and keeps favorites and recent items. Placing an item starts the placement command with the item's parameters. | T1 | 4 |
| DB-TSF-060 | Catalog linkage of instances | Each placed instance records its catalog item ID and revision. An update-from-catalog command refreshes instances to the current revision, and a missing-item report lists instances whose item no longer exists. | T2 | 4 |
| DB-TSF-061 | Catalog authoring | Creates a catalog item from selected drawing content with named parameters, constraints and size table rows, and publishes it to a chosen catalog as a new revision. | T2 | 4 |
| DB-TSF-062 | Tool palettes | A palette tool presets class, style, property values and catalog item. Palettes are user-editable, grouped per toolset, exportable and switched automatically with the active toolset workspace. | T1 | 4 |
| DB-TSF-063 | Content packages | Installs, verifies and removes catalogs, palettes, styles and templates from a package with a manifest and checksums. Content outside a package is never overwritten silently. | T2 | 4 |

## Standards synchronization

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TSF-064 | Standards sources | Designates a drawing or library as the standards source holding styles, layers, linetypes, display settings, property set definitions and title blocks, at drawing or project level. | T1 | 4 |
| DB-TSF-065 | Standards comparison | Lists every difference between a drawing and its standards source as missing, different or extra, grouped by kind, with a side-by-side view of the properties that differ. | T1 | 4 |
| DB-TSF-066 | Standards synchronization | Applies selected differences to the drawing as one undoable command, with a choice per difference of source wins or keep local; runs across all project drawings in a batch. | T1 | 4 |
| DB-TSF-067 | Layer rule standards | Maps classes and style components to layers through a rule table for a layering standard. Applying the table moves objects to the mapped layers, and the audit lists objects on layers that disagree with the table. | T2 | 4 |
| DB-TSF-068 | Drafting standard switch | Changes the drafting standard of a drawing by mapping text, dimension, hatch and symbol styles through conversion tables, and reports styles with no mapping. | T2 | 4 |

## Degradation and export

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TSF-069 | Cached graphics on every object | Stores with each typed object the graphics of its current display, so an application without the toolset shows the correct picture and can select and move the object as a unit. | T1 | 4 |
| DB-TSF-070 | Save for other applications | Offers three outputs per save or export: keep typed objects with cached graphics, explode objects to plain entities, or write one block per object with its properties as attributes. The choice is stored per target and repeatable in batch. | T1 | 4 |
| DB-TSF-071 | Deterministic explosion | Explodes each object to lines, polylines, arcs, hatches, 3D solids and text on layers mapped by the layer rule table, grouped per object so selection by original object remains possible. The same drawing always explodes to the same entities. | T1 | 4 |
| DB-TSF-072 | Property data on export | Writes property sets as extended data on exploded entities and as attributes on object blocks, and offers a CSV sidecar of all objects with their properties keyed by stable ID. | T2 | 4 |
| DB-TSF-073 | Degradation report | Lists every class converted, the output used, the object counts and what is lost (parametric behavior, associations, styles) for each save for other applications. | T1 | 4 |
| DB-TSF-074 | DXF output | Writes explosions only to DXF, since the format has no custom object path, and places the degradation report next to the file. | T1 | 4 |
| DB-TSF-075 | Opening without a toolset | A drawing whose toolset is not enabled opens with cached graphics, objects selectable as units, property data readable and parameters locked. Saving it preserves every typed object unchanged. | T1 | 4 |
| DB-TSF-076 | Re-association after round trip | When an exploded drawing returns with unchanged geometry and intact extended data, offers to re-attach the original stable IDs and associations so schedules and tags keep working. | T3 | 5 |
| DB-TSF-077 | IFC export of toolset objects | Maps classes to IFC entity types and property sets for building toolsets, with a loss report for objects that map only to generic proxies. | T2 | 4 |

## Toolset enablement

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TSF-078 | Enablement record per drawing | A drawing stores the toolsets and versions it uses. Opening it with a toolset disabled offers to enable it; declining leaves the drawing in the state described in DB-TSF-075. | T1 | 4 |
| DB-TSF-079 | Toolset workspaces | Each toolset supplies a workspace of ribbon or menu groups, palettes, command aliases and templates. Several toolsets can be enabled at once, and switching workspaces changes only the user interface. | T1 | 4 |
| DB-TSF-080 | Namespaced names | Prefixes styles, layers, property sets and classes of each toolset so two toolsets in one drawing cannot collide, and reports any collision found in an imported drawing. | T1 | 4 |
| DB-TSF-081 | Toolset dependencies | A toolset manifest declares dependencies on other modules (building services on architecture backgrounds is optional; plant requires the framework only). Enabling a toolset enables its required modules and reports optional ones. | T2 | 4 |
| DB-TSF-082 | Toolset templates | Each toolset ships drawing templates with units, standards, layers, styles, display configurations and title blocks, and new drawings can be created from them in the application and the headless CLI. | T1 | 4 |
| DB-TSF-083 | Headless toolset runner | Runs project update, schedule refresh, standards synchronization, validation and export in the headless CLI with the same results as the interactive application. | T1 | 4 |

# Layers and object properties

Layers organize every object and supply its default appearance; object properties decide how each
object displays and plots. Layer settings, states and per-viewport overrides are stored in the
drawing so that they survive exchange through DWG and DXF.

## Domain objects

- **Layer** — a symbol-table record with visibility, freeze, lock, plot and appearance properties.
- **Layer state** — a named snapshot of selected layer properties that can be restored or exchanged.
- **Layer filter** — a property filter (rule-based) or group filter (explicit membership) over layers.
- **Viewport layer override** — a per-viewport freeze flag or replacement color, linetype, lineweight, transparency or plot style.
- **Linetype** — a dash pattern, optionally carrying embedded shapes or text.
- **Object property set** — layer, color, linetype, linetype scale, lineweight, transparency, plot style and their ByLayer/ByBlock sources.

## Layer properties

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-LAYER-001 | Layer records | Each layer stores name, on/off, frozen/thawed, locked/unlocked, color, linetype, lineweight, transparency (0 to 90 percent), plot style name, plot flag, description and a freeze-in-new-viewports flag; names follow DWG naming rules and reserved characters are rejected with a message. | T1 | 1 |
| DB-LAYER-002 | Visibility semantics | Objects on layers that are off are hidden but take part in regeneration; objects on frozen layers are hidden, excluded from regeneration and excluded from selection; objects on non-plotting layers display but never plot. | T1 | 1 |
| DB-LAYER-003 | Locked layers | Objects on locked layers can be snapped to and measured but not modified; they display faded by a user-set percentage so editable geometry stands out. | T1 | 1 |
| DB-LAYER-004 | Current layer and current properties | New objects take the current layer, color, linetype, lineweight, transparency and plot style; the current values are shown and changed from a properties strip and through commands. | T1 | 1 |
| DB-LAYER-005 | Special layers | Layer 0 cannot be renamed or deleted and lets block content drawn on it inherit the layer of the reference; the definition-point layer created by dimensions holds points that never plot. | T1 | 1 |
| DB-LAYER-006 | Layer creation and command form (`LAYER`, `-LAYER`) | Creates, renames, deletes (when empty and not current) and sets layers; the hyphen form edits every layer property through prompts for scripts. | T1 | 1 |

## Layer panel

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-LAYER-007 | Layer panel | A dockable, non-modal panel lists layers with a filter tree, search box, sortable columns for every property and multi-selection editing; it stays open while commands run and refreshes after each transaction. | T1 | 1 |
| DB-LAYER-008 | Property filters | Rule-based filters select layers by name wildcard and by any property value; filters nest, can be inverted, and update their membership automatically when layers change. | T2 | 1 |
| DB-LAYER-009 | Group filters | Explicit layer groups hold chosen layers regardless of their properties; layers are added by drag or by selecting objects in the drawing. | T2 | 1 |
| DB-LAYER-010 | Large layer tables | The panel and filters stay responsive with tens of thousands of layers by virtualized rows and incremental filtering. | T2 | 1 |

## Layer states

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-LAYER-011 | Save and restore layer states (`LAYERSTATE`) | Saves named snapshots of the chosen layer properties and restores them, optionally turning off layers absent from the state and optionally restoring as viewport overrides in the active viewport. | T2 | 1 |
| DB-LAYER-012 | Layer state exchange | Exports and imports layer states as LAS files and copies states between drawings; states whose layers are missing restore the matching subset and report the rest. | T2 | 1 |
| DB-LAYER-013 | Layer previous (`LAYERP`) | Undoes the last layer-setting changes without undoing geometry edits, keeping a bounded history of layer-setting changes. | T2 | 1 |

## Layer commands

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-LAYER-014 | Isolate and unisolate (`LAYISO`, `LAYUNISO`) | Isolates the layers of selected objects by turning off, locking and fading, or freezing in the current viewport the other layers, per setting; unisolate restores exactly the prior settings. | T1 | 1 |
| DB-LAYER-015 | Off, on, freeze, thaw, lock, unlock by pick (`LAYOFF`, `LAYON`, `LAYFRZ`, `LAYTHW`, `LAYLCK`, `LAYULK`) | Changes the layer of each picked object, including layers of nested block and reference content when chosen, without opening the panel. | T1 | 1 |
| DB-LAYER-016 | Current-layer helpers (`LAYMCUR`, `LAYCUR`) | Makes the picked object's layer current, or moves selected objects to the current layer. | T1 | 1 |
| DB-LAYER-017 | Match and change layer (`LAYMCH`) | Moves selected objects to the layer of a picked object or to a layer named or chosen from a list. | T1 | 1 |
| DB-LAYER-018 | Delete layer with contents (`LAYDEL`) | Deletes a layer and every object on it, including references inside block definitions, after listing what will be removed; the deletion is one undoable transaction. | T2 | 1 |
| DB-LAYER-019 | Merge layers (`LAYMRG`) | Moves all objects from source layers, including those inside block definitions, onto a target layer and removes the source layers. | T2 | 1 |
| DB-LAYER-020 | Viewport isolate (`LAYVPI`) | Freezes the selected objects' layers in every layout viewport except the current one. | T2 | 1 |
| DB-LAYER-021 | Layer walk (`LAYWALK`) | Steps through layers or wildcard groups showing only the chosen layers temporarily; on exit the original visibility returns unless the user keeps the result. | T2 | 1 |

## Viewport overrides and notification

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-LAYER-022 | Per-viewport freeze (`VPLAYER`) | Freezes or thaws layers in selected layout viewports, sets the freeze default for new viewports and resets viewports to the global setting. | T1 | 1 |
| DB-LAYER-023 | Per-viewport property overrides | Replaces color, linetype, lineweight, transparency or plot style of a layer inside one viewport; overridden values are marked in the layer panel and can be removed per layer or per viewport, and display of overrides can be toggled. | T2 | 1 |
| DB-LAYER-024 | New-layer notification and reconcile | Records which layers existed at the last reconcile; layers added since, including from references, are flagged when saving, plotting or reloading references, and the user marks them reconciled. | T2 | 2 |
| DB-LAYER-025 | Reference layers | Layers from external references appear with the reference name as a prefix and are read-only in name; whether local changes to reference layers persist across reload is a drawing setting, and bound layers receive a collision-free name. | T1 | 1 |
| DB-LAYER-026 | Layer standards validation | Checks layer names and properties against a naming rule set or a standards drawing, lists violations in the panel and offers mapped corrections; the same check runs headless (see DB-INTEROP). | T3 | 2 |

## Colors, linetypes, lineweights and transparency

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-LAYER-027 | Color model | Colors are index colors (1 to 255), true colors (24-bit RGB) or entries of an imported color book, plus ByLayer and ByBlock; colors round-trip through DWG and DXF and index colors define CTB plot mapping. | T1 | 1 |
| DB-LAYER-028 | Color book import | Imports color books from a documented file format and keeps the book and entry name on each object so the reference survives exchange. | T3 | 2 |
| DB-LAYER-029 | Linetype loading (`LINETYPE`) | Loads simple and complex linetypes from LIN files, sets the current linetype, shows dash definitions and deletes unused linetypes. | T1 | 1 |
| DB-LAYER-030 | Complex linetypes | Draws linetypes with embedded shapes or text scaled and rotated per definition; when the referenced shape file is missing the dashes still draw and the missing file is reported. | T2 | 1 |
| DB-LAYER-031 | Linetype scaling | Applies a global scale, a per-object scale and optional paper-space scaling so dashes look the same in every viewport scale; model space can follow the annotation scale (see DB-PLOT). | T1 | 1 |
| DB-LAYER-032 | Continuous pattern on polylines | Per polyline, the dash pattern either restarts at each vertex or flows continuously along the whole polyline. | T1 | 1 |
| DB-LAYER-033 | Lineweights (`LWEIGHT`) | Lineweights come from a fixed list in millimeters plus ByLayer, ByBlock and Default; display can be toggled and scaled on screen, while plotted lineweights are exact. | T1 | 1 |
| DB-LAYER-034 | Transparency | Objects, layers and blocks carry transparency; its display can be toggled and plotting of transparency is a page-setup choice. | T1 | 1 |
| DB-LAYER-035 | ByLayer and ByBlock resolution | Every inheritable property resolves from object to ByBlock (the containing reference) to ByLayer (the object's layer, then viewport override); nested blocks resolve level by level and the resolved value is shown alongside the stored value. | T1 | 1 |

## Editing object properties

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-LAYER-036 | Properties panel (`PROPERTIES`) | Shows the properties of the selection grouped by category; for mixed selections it lists shared properties, lets the user filter to one object type, and applies an edit to every applicable object in one transaction. | T1 | 1 |
| DB-LAYER-037 | Match properties (`MATCHPROP`) | Copies chosen properties (layer, color, linetype, scale, lineweight, transparency, plot style, thickness and type-specific properties such as text style, dimension style, hatch pattern, table and viewport settings) from a source object to destination objects. | T1 | 1 |
| DB-LAYER-038 | Command-line property edit (`CHPROP`, `CHANGE`) | Changes layer, color, linetype, linetype scale, lineweight, transparency, thickness and elevation of selected objects through prompts, for keyboard work and scripts. | T1 | 1 |
| DB-LAYER-039 | Property accessibility | Every property field has an accessible name, can be reached and edited by keyboard alone, and uses unit-aware numeric input. | T1 | 1 |

# Drawing database

The drawing database is the in-memory model of a DWG/DXF drawing: header settings, symbol tables,
dictionaries, block records and the entities they own. Every other area reads and changes the
drawing only through this model, inside journaled transactions, and everything it does not
understand is carried through unchanged.

## Domain objects

- **Drawing** — one database: header settings, owner tree, handle seed, fingerprint and version identifiers.
- **Object** — any database record with a handle, an owner and an optional extension dictionary; graphical objects are **entities**.
- **Symbol table** and **symbol table record** — named, non-graphical definitions (layers, linetypes, text styles, dimension styles, views, UCSs, viewport configurations, registered applications, block records).
- **Dictionary** — named map of objects; the named-object dictionary is the root of non-table objects.
- **Xrecord** — a dictionary entry holding an arbitrary typed value list.
- **Extended data** — typed value groups attached to an object under a registered application name.
- **Unknown object** — an object whose class DraftBuilder does not model, kept as raw data with its proxy graphics.
- **Group** — a named, ordered set of entities.

## Database structure

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DBASE-001 | Typed object store aligned with the DWG model | The drawing is held as typed objects organized as header, symbol tables, named-object dictionary and block records that own entities, with every object recording its owner. Reading and writing DXF or DWG maps one-to-one onto this structure without an intermediate format. | T1 | 1 |
| DB-DBASE-002 | Header settings | Drawing-scoped settings (units, current layer and styles, linetype scale, dimension defaults, annotation scale and similar) are stored in the header and saved with the drawing. A change to a header setting is an undoable transaction and is visible to scripts under both its DraftBuilder name and its widely used compatibility name. | T1 | 1 |
| DB-DBASE-003 | Owner tree integrity | Every object except the root has exactly one owner, and deleting an owner deletes or reassigns its owned objects in the same transaction. Operations that would create an owner cycle or an orphan are rejected with a diagnostic. | T1 | 1 |
| DB-DBASE-004 | Symbol table records | Creating, renaming and deleting a symbol table record (layer, linetype, style, view, UCS, block and others) are commands; names are case-insensitive, unique per table, up to 255 characters, and reject reserved characters. Records still referenced cannot be deleted; a rename updates every reference because references are by object, not by name. | T1 | 1 |
| DB-DBASE-005 | Reserved records | Layer `0`, the continuous linetype, the standard text and dimension styles, model space and the paper space records always exist and cannot be renamed or deleted. A drawing missing any of them on load has them recreated and the repair is reported. | T1 | 1 |
| DB-DBASE-006 | Dictionaries | Named-object and nested dictionaries store groups, layouts, styles, scales, materials, visual styles, plot settings and application data. Dictionary entries are addressable by key from commands, the automation protocol and the Lisp-dialect interpreter. | T1 | 1 |
| DB-DBASE-007 | Extension dictionaries and xrecords | Any object can own an extension dictionary whose xrecords hold typed value lists for application data. Xrecords are read, written and copied with their owner and survive save and load unchanged. | T1 | 1 |
| DB-DBASE-008 | Extended data | Objects carry extended data groups keyed by registered application name, with the per-object size limit of the format enforced on write. Extended data written by any application is kept, copied with the object and saved back without reordering. | T1 | 1 |
| DB-DBASE-009 | Registered application names | Application names used by extended data are registered in the drawing; registering is idempotent and names without remaining references can be purged. | T1 | 1 |
| DB-DBASE-010 | Spaces | Model space holds design geometry at real-world size; each layout owns a paper space block record that holds its sheet content. Commands target the current space, and objects can be moved between spaces with coordinates transformed through a chosen viewport. | T1 | 1 |
| DB-DBASE-011 | Draw order storage | Each block record keeps a display order for its entities, which DXF and DWG round trips preserve. Display, selection preview and plotting follow this order; new entities are placed on top unless a command states otherwise. | T1 | 1 |
| DB-DBASE-012 | Groups | Named and unnamed groups hold an ordered set of entities with a description and a selectable flag; members can be added, removed and reordered. Erasing all members removes the group, and groups are kept through copy into other drawings when all members are copied. | T2 | 1 |
| DB-DBASE-013 | Hyperlinks | Any object can carry a hyperlink with address, sub-location and description, stored in the format's standard location. Links are listed in properties, followed on request and exported to PDF output. | T2 | 1 |
| DB-DBASE-014 | Drawing properties | Title, subject, author, keywords, comments, hyperlink base and any number of named custom properties are stored with the drawing. Fields, sheet title blocks and the automation protocol read them by name. | T1 | 1 |

## Identity and preservation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DBASE-015 | Handles | Every object has a hexadecimal handle that is unique within the drawing and unchanged across save and reload. New objects take handles from the drawing's handle seed; handles are never reused within a drawing, including after undo. | T1 | 1 |
| DB-DBASE-016 | Stable object identifiers | Each object has a 128-bit identifier derived from the drawing fingerprint and its handle, so the same object has the same identifier in every session and in every copy of the file. Journal entries, Hub issues, links and change sets refer to objects by this identifier. | T1 | 1 |
| DB-DBASE-017 | Lossless preservation of unknown objects | Objects and entity classes DraftBuilder does not model are kept with their class record, raw data and references and written back unchanged on save. Copy, move, rotate, scale and mirror of such entities apply the transform to their proxy graphics and record the edit in the loss report; edits that would invalidate their raw data are refused or reported per object. | T1 | 1 |
| DB-DBASE-018 | Proxy graphics display | Unknown entities display, plot and snap using the proxy graphics stored with them, and properties shows their class name, owner application and layer. A setting shows them as graphics, as a bounding frame, or hidden. | T1 | 1 |
| DB-DBASE-019 | Preservation report | On load and save, a report lists every unknown class, object count, preserved data volume and every object whose foreign data was dropped or invalidated, with the reason. The report is available interactively and as structured output from the headless CLI. | T1 | 1 |
| DB-DBASE-020 | Reference integrity of handles | Handle references inside entities, dictionaries, extended data and xrecords are kept valid through copy, wblock, insert and paste by remapping to the new handles. References that cannot be resolved are kept as raw values and reported. | T1 | 1 |

## Units and coordinates

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DBASE-021 | Drawing units | The drawing declares its length unit (unitless, metric or imperial units) and insertion unit; inserting blocks, references and content from another drawing scales by the ratio of the two units unless either side is unitless. Coordinates are stored as 64-bit floating point in drawing units. | T1 | 1 |
| DB-DBASE-022 | World and object coordinate systems | Planar entities store their geometry in the object coordinate system derived from their normal, exactly as the format defines, and expose world coordinates to commands. Reading and writing never re-derives a normal or reorders vertices. | T1 | 1 |
| DB-DBASE-023 | Large-coordinate precision | Drawings with coordinates far from the origin, including national grid coordinates in the millions, edit, snap and display without jitter because display is camera-relative and geometry computations use tolerances relative to model size. | T1 | 1 |
| DB-DBASE-024 | Drawing limits | Optional rectangular limits are stored per space; when limit checking is on, point entry outside the limits is rejected with a message. Grid display can be restricted to the limits. | T3 | 1 |

## Files and recovery

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DBASE-025 | New drawing from template | A new drawing is created from a template drawing (DWG or DXF used as a template) or from a built-in metric or imperial default, copying its tables, styles, layouts, page setups and units. A default template can be set so a new drawing needs no prompt. | T1 | 1 |
| DB-DBASE-026 | Open | DXF (ASCII and binary) and DWG drawings open into the database; a newer unsupported version is refused without touching the file. Opening can be read-only or with a chosen initial view. | T1 | 1 |
| DB-DBASE-027 | Partial load | Large drawings open with only chosen layers, views or a spatial region loaded, and further content loads on demand. Unloaded content is kept in the file unchanged on save. | T2 | 2 |
| DB-DBASE-028 | Save and save as | Save writes the drawing in its current format and version; save as chooses DXF or DWG and a target version. Saves are atomic (write to a temporary file, then replace), so a failed save never leaves a truncated drawing. | T1 | 1 |
| DB-DBASE-029 | Save to an older version | Saving to an older format version converts or simplifies objects the target cannot represent and lists every conversion in a loss report before the file is written. The open drawing keeps its full content. | T2 | 2 |
| DB-DBASE-030 | File locks | Opening a drawing for editing writes a lock file that names the user and machine in the conventional lock-file format; a second opener is offered read-only access and the lock owner. Locks are removed on close and stale locks are detected. | T1 | 1 |
| DB-DBASE-031 | Autosave and backup | Changes are kept in a crash-safe recovery journal in the sidecar package at a configurable interval, and saving keeps the previous file as a backup copy. Both can be disabled per profile. | T1 | 1 |
| DB-DBASE-032 | Crash recovery | After an abnormal exit, the next start lists drawings with unsaved recovery data and restores any of them to the last journaled transaction. Recovery never overwrites the original file without a save. | T1 | 1 |
| DB-DBASE-033 | Audit | `AUDIT` checks owner links, handle references, table integrity, entity geometry validity and dictionary structure, and either reports or repairs each finding. Repairs are one undoable transaction and the report lists each object changed. | T1 | 1 |
| DB-DBASE-034 | Recover damaged files | `RECOVER` reads a damaged DXF or DWG, salvaging every readable object, rebuilding tables and owner links, and opening the result with a report of what was lost. A batch form recovers a drawing and its external references. | T1 | 1 |
| DB-DBASE-035 | Purge | `PURGE` removes unreferenced blocks, layers, linetypes, styles, registered application names, zero-length geometry and empty text, optionally repeating until nothing remains. A preview lists what would be removed, and the non-interactive form takes the categories as parameters. | T1 | 1 |
| DB-DBASE-036 | Sidecar package | Data that the drawing format cannot carry (journal history, recovery journal, Hub metadata, per-drawing DraftBuilder settings) is kept in a `.draftb` package beside the drawing. The drawing opens fully without the package; deleting it loses only history. | T1 | 1 |

## Change history

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DBASE-037 | Undo and redo | Every command, including script, macro and automation-protocol commands, is one undo step built from the operation journal; redo replays undone steps until a new change is made. A command that fails rolls back completely and leaves no undo step. | T1 | 1 |
| DB-DBASE-038 | Undo marks, groups and counts | `UNDO` accepts a count, sets and returns to marks, groups several commands into one step, and can be limited or switched to single-step for memory. Undo depth is bounded only by a configurable memory budget. | T1 | 1 |
| DB-DBASE-039 | Restore last erased | `OOPS` restores the objects removed by the most recent erase, block definition or wblock, even after other commands have run. | T2 | 1 |
| DB-DBASE-040 | Journal-based object history | Each object's history (created, each modification with before and after values, deleted) is queryable from the journal for the session and, through the sidecar package, across sessions. History survives save and reopen and can restore an object to an earlier state as a new transaction. | T3 | 2 |

## Inquiry and display cache

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-DBASE-041 | List object data | `LIST` reports the type, layer, space, handle, properties and geometry of selected objects in the history window; `DBLIST` does the same for the whole drawing. Output is also returned as structured data by the non-interactive form. | T1 | 1 |
| DB-DBASE-042 | Drawing status | `STATUS` reports object counts, extents, limits, current settings, space and memory use; a time report shows creation, last update and total editing time. | T2 | 1 |
| DB-DBASE-043 | Regeneration of display data | Display geometry (tessellation of curves, text and hatch, linetype patterns) is derived data that rebuilds automatically when its source or a controlling setting changes; `REGEN` and `REGENALL` force a rebuild of the current or every viewport. Curve smoothness is a display setting that never changes stored geometry. | T1 | 1 |
| DB-DBASE-044 | Multi-threaded regeneration | Regeneration and file loading run on worker threads in parallel per block record and spatial tile, carry the revision they were computed from, and are discarded if the drawing changed meanwhile. The interface stays responsive and cancellable during long rebuilds. | T3 | 1 |

# Plant toolset

The plant toolset adds process and piping design to DraftBuilder: project set-up, P&ID drafting with intelligent lines and symbols, tagging and annotation, a data manager with validation, pipe specifications and catalogs, 3D equipment, structure and piping routing, supports, orthographic and isometric drawing generation, reports and material take-off, consistency checks between the P&ID and the 3D model, and project collaboration. It builds on the framework in [20-toolset-framework.md](20-toolset-framework.md).

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Plant project | Name, units, class definitions, tag rules, folder layout, project database | Contains P&ID, 3D, orthographic, isometric and report drawings |
| Class definition | Class name, properties with types, symbol, tag format | Defines equipment, line, valve and instrument classes |
| P&ID symbol | Class, tag, properties, ports | Placed on a P&ID; connected by lines; linked to a 3D object |
| P&ID line | Path, line group, line number, size, spec, medium | Connects symbols; carries line number tags |
| Line group | Number, service, spec, insulation, pressure data | Groups P&ID lines and 3D pipes |
| Equipment | Tag, class, nozzles, dimensions | Appears in the P&ID and as a 3D object |
| Nozzle | Size, rating, face, position, direction | Belongs to equipment; connects to piping |
| Instrument, signal line | Function code, loop, tag | Connects to a process line or equipment |
| Pipe spec | Pipe class, service, rating, size range, rules, parts | Selected by lines and 3D pipes |
| Catalog part | Parametric geometry, ports, size table, end types | Selected by specs |
| 3D pipe, fitting, valve | Spec, size, line number, route, slope | Belongs to a line group; connects through ports |
| Support | Kind, position, load data | Attaches to pipe and structure |
| Structural member | Shape, length, justification | Forms platforms, ladders, stairs and frames |
| Orthographic view | Source model area, direction, scale, annotations | Derived from the 3D model |
| Isometric | Line or group, style, spools, welds, bill of materials | Derived from 3D piping |
| Data table | Class, columns, filter | Views and edits project data |
| Validation finding | Rule, object, severity | Produced by P&ID and consistency checks |

## Project set-up

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TPLANT-001 | Project creation | Creates a plant project with folders for P&ID, 3D areas, orthographic views, isometrics and reports, a project database and a settings file. | T1 | 4 |
| DB-TPLANT-002 | Project settings | Holds units, class definitions, tag and line number formats, drawing templates, styles and the standards source, applied to every project drawing with per-drawing overrides marked. | T1 | 4 |
| DB-TPLANT-003 | Class and property definitions | Edits the classes of equipment, line, valve, instrument and signal objects and their typed properties, symbols and tag formats; changes propagate to existing objects with a report of added and removed properties. | T1 | 4 |
| DB-TPLANT-004 | Drawing management | Lists project drawings by type and area, creates drawings from templates, and keeps the project list in sync when drawings are renamed or moved. | T1 | 4 |
| DB-TPLANT-005 | Project database | Stores tags, properties and links in an open, documented database file, so reports and other tools read project data without opening drawings. | T1 | 4 |
| DB-TPLANT-006 | Project import and export | Exports and imports project settings, class definitions and specifications as a package so a new project starts from an existing one. | T2 | 4 |

## P&ID drafting

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TPLANT-007 | P&ID symbol placement | Places equipment, valve and instrument symbols from palettes grouped by class and standard; each symbol carries ports and properties from its class definition. | T1 | 4 |
| DB-TPLANT-008 | Intelligent lines | Draws process lines that connect to symbol ports and to other lines; a line splits when a valve or fitting is placed on it, and the connected line segments share one line number. | T1 | 4 |
| DB-TPLANT-009 | Line groups and line numbers | Assigns lines to line groups with service, spec, size and insulation, generating the line number from the project format and flagging duplicates. | T1 | 4 |
| DB-TPLANT-010 | Line crossings and flow arrows | Breaks one line at a crossing by rule and places flow direction arrows, which update when a line is reversed. | T1 | 4 |
| DB-TPLANT-011 | Valve auto-connect | A valve dropped on a line inserts itself and splits the line while preserving the line group on both sides, and a valve moved off a line closes the gap. | T1 | 4 |
| DB-TPLANT-012 | Instruments and signals | Places instruments and draws signal lines (electric, pneumatic, software) from an instrument to a process line or controller, with loop tag assignment. | T1 | 4 |
| DB-TPLANT-013 | Off-page connectors | Places connectors that link lines across P&ID sheets by a reference, checking that each has a matching partner and that the line number agrees. | T1 | 4 |
| DB-TPLANT-014 | Equipment and nozzles | Places equipment with tag, class data and nozzle definitions, with nozzle data (size, rating, face) available to connected lines. | T1 | 4 |
| DB-TPLANT-015 | Inline components | Adds reducers, strainers, relief devices and spec breaks on a line; a spec break changes the spec on the downstream segment and checks the part compatibility. | T2 | 4 |
| DB-TPLANT-016 | P&ID editing | Moves, copies and replaces symbols with connected lines following, stretches lines with ports preserved, and replaces a symbol by another of the same class. | T1 | 4 |

## Tagging and annotation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TPLANT-017 | Tag generation | Generates equipment, valve and instrument tags from a project format with prefix, area and sequence, and prevents duplicates across the project. | T1 | 4 |
| DB-TPLANT-018 | Line number tag annotation | Places line number tags that show the line group data, and follow the line when it moves; several formats can show service, size and spec. | T1 | 4 |
| DB-TPLANT-019 | Automatic annotation | Annotates all lines or equipment on a drawing from a format combining size, service, spec, insulation and flow, and updates annotations when data changes. | T2 | 4 |
| DB-TPLANT-020 | Property annotation | Places annotations that show any property of an object, and lists objects with no annotation. | T2 | 4 |

## Data manager and validation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TPLANT-021 | Data manager | Shows P&ID and 3D object data in tables by class with filter, sort and edit; edits are written through commands and appear in the drawings at once. | T1 | 4 |
| DB-TPLANT-022 | Data import and export | Imports property data from CSV and XLSX keyed by tag and exports tables to the same, reporting unmatched keys and read-only columns. | T1 | 4 |
| DB-TPLANT-023 | P&ID validation | Checks unconnected lines, lines without a line number, missing tags, duplicate tags, ports without connections and mismatched line groups, listing each finding with navigation. | T1 | 4 |
| DB-TPLANT-024 | Validation rule sets | Lets the user enable, disable and set the severity of rules per project, and exports the result for review. | T2 | 4 |
| DB-TPLANT-025 | Data validation by property | Checks required properties, value lists and numeric ranges per class and reports violations in the data manager. | T2 | 4 |

## Specifications and catalogs

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TPLANT-026 | Pipe specification editor | Defines pipe specs by service, rating and size range, with rules that select the part for each component and size; a spec belongs to the project and can be exported. | T1 | 4 |
| DB-TPLANT-027 | Catalog editor | Defines catalog parts with size tables, dimensions, ports, end types and parametric geometry for pipe, elbow, tee, reducer, flange, valve, gasket and bolt families. | T1 | 4 |
| DB-TPLANT-028 | Spec viewer and sheets | Shows the parts a spec selects for each size and produces a spec sheet as a table or document. | T1 | 4 |
| DB-TPLANT-029 | Branch tables | Defines which branch connection (tee, weldolet, stub-in) applies for each pair of header and branch sizes within a spec. | T2 | 4 |
| DB-TPLANT-030 | Substitution rules | Defines part substitution for reducers and nozzle fittings when the preferred part does not exist in a size, with a log of every substitution applied. | T2 | 4 |
| DB-TPLANT-031 | Spec import and export | Moves specs and catalogs between projects with a check for missing parts and conflicts. | T2 | 4 |

## Three-dimensional equipment, structure and piping

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TPLANT-032 | Equipment modeling | Models vessels, tanks, pumps and exchangers from parametric templates with nozzles and a plan footprint, and imports solid bodies from STEP to attach nozzles to. | T1 | 4 |
| DB-TPLANT-033 | Pipe routing | Routes pipe by spec and size between equipment nozzles and existing pipe, with fittings placed automatically, slope applied and elbows and tees chosen from the spec. | T1 | 4 |
| DB-TPLANT-034 | Fittings, valves and flange sets | Places fittings and valves from the spec with automatic trimming of the pipe; flanged connections add gasket and bolt sets from the spec. | T1 | 4 |
| DB-TPLANT-035 | Insulation and tracing | Adds insulation by line group with thickness by size, included in take-off and in clearance checks. | T2 | 4 |
| DB-TPLANT-036 | Pipe editing | Moves, resizes, changes spec, replaces parts and straightens runs, with connected parts and line group data updating. | T1 | 4 |
| DB-TPLANT-037 | Line list driven routing | Takes line data from the P&ID so that a 3D pipe created for a line number receives its line group, spec, size and medium. | T1 | 4 |
| DB-TPLANT-038 | Structural modeling | Models steel members, platforms, ladders, stairs, handrails and foundations from shape catalogs, with member end cuts and connections defined as plates, bolts or simple end rules. | T2 | 4 |
| DB-TPLANT-039 | Supports | Places pipe supports and hangers from the catalog on pipe and structure, with load and spacing checks against the spec. | T2 | 4 |
| DB-TPLANT-040 | Interference checking | Checks pipe, equipment and structure against each other with clearance distances and lists results with status, shared with the building services format. | T2 | 4 |
| DB-TPLANT-041 | Point cloud reference | Attaches a point cloud to the model for brownfield work and snaps model objects to scanned surfaces. | T3 | 5 |

## Drawing generation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TPLANT-042 | Orthographic generation | Generates plan, section and elevation views from selected areas of the 3D model, associative to the model, with annotation and dimensions that update on refresh. | T1 | 4 |
| DB-TPLANT-043 | Orthographic annotation | Adds associative dimensions, tags and elevation marks to generated views, kept through regeneration and flagged if their object is removed. | T1 | 4 |
| DB-TPLANT-044 | Isometric generation | Generates isometrics from selected lines or line groups with styles for annotation, spool names, weld numbers, dimensions and a bill of materials, and writes the same data in a neutral exchange file. | T1 | 4 |
| DB-TPLANT-045 | Quick and production isometrics | Provides a quick isometric that is a view of the model for checking, and a production isometric with spools and sheet splitting for fabrication. | T2 | 4 |
| DB-TPLANT-046 | Spool and weld tagging | Divides piping into spools by rule (shop and field welds, maximum length), numbers spools and welds, and updates the isometric and the reports. | T2 | 4 |
| DB-TPLANT-047 | Isometric styles | Defines the sheet size, symbols, annotation and table layout of isometrics as styles used by all isometrics in a project. | T2 | 4 |

## Reports and take-off

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TPLANT-048 | Report creator | Builds reports from data manager tables with chosen columns, filters, sort, grouping and totals, saved as templates and exported to CSV, XLSX, XML, PDF and HTML. | T1 | 4 |
| DB-TPLANT-049 | Line and equipment lists | Provides ready reports for equipment, lines, valves and instruments from the P&ID, and the same from the 3D model. | T1 | 4 |
| DB-TPLANT-050 | Material take-off | Takes off pipe length, fittings, valves, flanges, gaskets and bolts from the 3D model by line, area or spool, and compares it with the P&ID list. | T1 | 4 |
| DB-TPLANT-051 | Isometric bill of materials | Lists parts per isometric with quantities and cut lengths, and totals across all isometrics in a project. | T2 | 4 |

## Consistency and collaboration

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TPLANT-052 | P&ID to 3D consistency check | Compares P&ID and 3D data by tag and line number, reporting items present in only one, size, spec and line group differences, and equipment nozzle mismatches. | T1 | 4 |
| DB-TPLANT-053 | Consistency resolution | Resolves a difference by copying the value from either side through a command, with a log of each resolution. | T2 | 4 |
| DB-TPLANT-054 | Shared project work | Several users work on different drawings of one project through a shared project database with drawing-level check-out via Arc Hub. | T2 | 5 |
| DB-TPLANT-055 | Project comparison | Compares two revisions of the project data and drawings and lists added, removed and changed objects by tag. | T3 | 5 |
| DB-TPLANT-056 | Plain-entity and exchange output | Exploded output keeps symbols as blocks with properties and piping as solids with data on layers; IFC export carries the 3D piping and equipment with line numbers. | T2 | 4 |

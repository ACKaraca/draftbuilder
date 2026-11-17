# Electrical controls toolset

The electrical controls toolset adds control and power schematics to DraftBuilder: a project that spans many drawings, schematic symbol libraries by standard, ladder and point-to-point wiring, wire numbering, component tagging, live cross-references, parent and child components, PLC input and output drawing generation, a circuit builder, panel layouts with footprints, a terminal strip editor, connectors and cables, a catalog database, reports, error auditing and title block updates. It builds on the framework in [20-toolset-framework.md](20-toolset-framework.md).

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Controls project | Name, drawing list with section and sheet, settings (tag format, wire format, cross-reference style, standard), project database | Contains drawings; owns the catalog link and reports |
| Controls drawing | Sheet, section, area, location, descriptions | Belongs to a project; contains components and wires |
| Component | Symbol, tag, catalog entry, description, rating, installation and location codes | Parent or child of other components; placed on a drawing or panel layout |
| Parent and child link | Parent component, child components, contact status | Drives cross-references and reports |
| Wire | Path, wire type, layer, gauge, color, number | Connects component pins; carries a wire number |
| Wire number | Value, format, fixed flag | Attached to wires and signal arrows |
| Ladder, rung | Rung spacing, numbering, line reference | Places components in rungs; supplies line references |
| Signal arrow | Source or destination, code | Links wires across sheets |
| Terminal, terminal strip | Terminal number, level, strip name, jumpers | Connects wires; appears on the panel layout |
| Jumper | Terminals, kind | Joins terminals in a strip |
| PLC module | Rack, slot, I/O points with address and description | Generates I/O drawings; reported |
| Panel footprint | Outline, mounting data, link to a schematic component | Placed on panel layout drawings |
| Connector, cable | Pins, cable cores, markers | Connects components across drawings |
| Catalog entry | Manufacturer, part number, description, ratings, linked symbol and footprint, multi-part pin lists | Referenced by components |
| Circuit definition | Parametric symbols, wires, tags, option set | Stored in a circuit library; inserted by the circuit builder |
| Report | Kind, columns, source, output | Generated from the project database |
| Audit finding | Rule, object, severity | Produced by the project audit |

## Project and drawings

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TELEC-001 | Controls project manager | Shows the project drawings grouped by area, location or section, with open, add, reorder, copy and rename actions, and drawing properties editable in place. | T1 | 4 |
| DB-TELEC-002 | Project settings | Holds tag format, wire format, wire layers, cross-reference style, standards and naming rules at project level, with a per-drawing override that is marked as an override. | T1 | 4 |
| DB-TELEC-003 | Drawing properties and descriptions | Sheet number, section, area, location and description lines per drawing, feeding the title block and the drawing index. | T1 | 4 |
| DB-TELEC-004 | Project-wide update | Updates tags, wire numbers, cross-references, title block fields and reports across all drawings in one run, in the application or headless, with a per-drawing report and cancel. | T1 | 4 |
| DB-TELEC-005 | Project copy and archive | Copies a project or packs it with its drawings, database, catalog links and settings into an archive that restores with paths intact. | T2 | 4 |
| DB-TELEC-006 | Project templates | A project template holds drawing templates, settings and library paths, and creating a project from it creates the first drawings. | T1 | 4 |
| DB-TELEC-007 | Drawing index report | Generates an index of drawings with sheet number, description and revision as a table in the drawing. | T1 | 4 |
| DB-TELEC-008 | Open project database | Stores project data (components, wires, terminals, cables, cross-references) in an open, documented database file, readable by tools outside the application. | T1 | 4 |

## Symbols, circuits and components

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TELEC-009 | Symbol libraries by standard | Provides symbol sets for IEC, IEEE and NFPA, JIC, GB and JIS practice; a project selects one standard, and a standard change swaps the symbols of existing components through a mapping table. | T1 | 4 |
| DB-TELEC-010 | Symbol insertion | Inserts a symbol from a menu or palette by category, with a catalog lookup at insertion and the component's tag, description and rating attributes filled from the chosen entry. | T1 | 4 |
| DB-TELEC-011 | Parent and child components | A parent component (such as a coil) links to child components (its contacts) wherever they are drawn, within or across sheets, and the links update when tags change. | T1 | 4 |
| DB-TELEC-012 | Multi-section components | A multi-section component such as a relay or a PLC module draws each section on different sheets with one tag and shared catalog data. | T2 | 4 |
| DB-TELEC-013 | Symbol builder | Builds a custom symbol with attributes, pin positions, orientation variants and standard category, and adds it to a user library. | T2 | 4 |
| DB-TELEC-014 | Ladder insertion | Inserts ladders with a rung spacing and power rails, and numbers rungs; each rung supplies the line reference used in tags and cross-references. | T1 | 4 |
| DB-TELEC-015 | Point-to-point wiring | Draws wires between component pins directly (without ladders), with automatic connection when a wire ends on a pin and wire breaking where wires cross components. | T1 | 4 |
| DB-TELEC-016 | Wire types | Defines wire types with layer, color, gauge and name, and draws or changes wires by type; the type feeds the wire list. | T1 | 4 |
| DB-TELEC-017 | Circuit builder | Inserts a parametric circuit (motor starter, feeder, heater) from a library after choosing options in a wizard, placing components, wires and tags in one command. | T2 | 4 |
| DB-TELEC-018 | Custom circuits | Saves selected components and wires as a library circuit with its parameters and option choices, and inserts it again with new tags. | T2 | 4 |
| DB-TELEC-019 | Copy circuit | Copies a circuit to a new location and assigns new tags and wire numbers by the project's rules, keeping the catalog data. | T1 | 4 |
| DB-TELEC-020 | Part and symbol swap | Replaces a component's catalog entry or symbol while keeping its tag, wires and links, with a check that the pin count matches. | T2 | 4 |
| DB-TELEC-021 | Source and destination arrows | Places arrows that link a wire across sheets by a code; selecting one lists its counterpart, and wire numbers propagate across the link. | T1 | 4 |
| DB-TELEC-022 | Cable markers | Marks wires that belong to a cable with the cable name and core color, and updates the cable summary report. | T2 | 4 |

## Wire numbering and tagging

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TELEC-023 | Wire numbering | Numbers wire networks by rung, sequentially, by source and destination or by a custom format, with fixed numbers preserved and a preview of changes before applying. | T1 | 4 |
| DB-TELEC-024 | Wire network rules | Treats wires joined by components that do not break a network (terminals, connectors, splices) as one number per the rule, and breaks at components that do. | T1 | 4 |
| DB-TELEC-025 | Wire number display | Places wire number text on wires with rotation and position rules, hides numbers on request without deleting them, and moves them with the wire. | T1 | 4 |
| DB-TELEC-026 | Component tagging | Generates tags from a format of line reference, installation and location codes and a sequence, in automatic, manual or from-list modes, and retags selected or all components. | T1 | 4 |
| DB-TELEC-027 | Tag uniqueness | Rejects duplicate tags unless the components are sections of the same component, and offers renumbering with a preview. | T1 | 4 |
| DB-TELEC-028 | Retag and renumber | Retags after a copy, a move to another section or a format change, preserving fixed tags and updating cross-references and reports. | T1 | 4 |

## Cross-referencing

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TELEC-029 | Real-time cross-references | Shows beside every parent the sheet and line of each child, and beside every child the location of its parent; changing a tag, moving a symbol or adding a child updates the references without a manual run. | T1 | 4 |
| DB-TELEC-030 | Cross-reference formats | Presents references as text, as graphical contact symbols, as a table or as a combination, chosen per project and overridable per component. | T1 | 4 |
| DB-TELEC-031 | Contact status | Shows each child contact as normally open, normally closed or spare, and counts spare contacts per parent for reporting. | T1 | 4 |
| DB-TELEC-032 | Signal and terminal references | Cross-references signal arrows, terminal numbers and jumpers between sheets, with a table form for terminals. | T2 | 4 |

## PLC and I/O

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TELEC-033 | PLC module database | Holds rack and slot modules with their I/O points, address ranges and descriptions in an editable database, with user-defined modules. | T1 | 4 |
| DB-TELEC-034 | PLC insertion | Inserts a module as a parametric symbol or as a full unit with all points drawn, tagged with its rack, slot and addresses. | T1 | 4 |
| DB-TELEC-035 | I/O drawing generation from a spreadsheet | Reads a CSV or XLSX list of I/O points with address and description and generates the PLC drawings with the module symbols, rungs and descriptions, reporting rows it cannot place. | T1 | 4 |
| DB-TELEC-036 | I/O address utilities | Edits addresses and descriptions in a table and applies them to the drawing, checks for duplicate and out-of-range addresses, and reassigns a block of addresses. | T2 | 4 |
| DB-TELEC-037 | PLC module report | Lists each module, its points, addresses, descriptions and the connected components. | T1 | 4 |

## Terminals and panel layouts

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TELEC-038 | Terminal symbols | Places single and multi-level terminals with strip name and numbers and attaches jumpers; the project database tracks every terminal. | T1 | 4 |
| DB-TELEC-039 | Terminal strip editor | Arranges terminals within a strip by dragging, renumbers them, edits properties and adds spares, jumpers and accessories, writing changes back to the drawings. | T1 | 4 |
| DB-TELEC-040 | Terminal reports | Produces terminal plan, strip and jumper reports with the wires attached to each terminal and the destination of each. | T1 | 4 |
| DB-TELEC-041 | Panel layout | Places footprints from the catalog on a panel drawing, each linked to its schematic component; a footprint can be created for a component by lookup in the footprint database. | T1 | 4 |
| DB-TELEC-042 | Footprint database | Holds footprints with outline, mounting holes, keep-out zone and 3D body, linked to catalog entries and editable by the user. | T2 | 4 |
| DB-TELEC-043 | Panel rails and ducts | Places rails and wire ducts with sizes, cuts them to length, and lists them in the panel bill of materials. | T2 | 4 |
| DB-TELEC-044 | Nameplates and panel balloons | Generates nameplate text from component descriptions and places balloons that show the item number of the panel bill of materials. | T2 | 4 |
| DB-TELEC-045 | Panel check | Warns about footprint overlaps, components outside the panel and footprints without a schematic component. | T2 | 4 |
| DB-TELEC-046 | Schematic and panel synchronization | Reports components present only in the schematic or only in the panel layout and updates the matching one on request. | T3 | 4 |

## Connectors and cables

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TELEC-047 | Connectors | Places connectors with pin lists and connects plug and receptacle halves across sheets, tracking pin assignment in the project database. | T2 | 4 |
| DB-TELEC-048 | Cables | Defines cables with cores, colors and length, assigns wires to cores, and produces the cable summary with cores used and spare. | T2 | 4 |
| DB-TELEC-049 | Harness from-to data | Lists the connection end to end across connectors and cables for harness manufacturing. | T3 | 5 |

## Catalog and standards

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TELEC-050 | Catalog database | Holds manufacturer entries with part number, description, ratings and links to symbols, footprints and multi-part pin lists, in an open database format. | T1 | 4 |
| DB-TELEC-051 | Catalog browser and import | Searches and assigns entries, imports entry lists from CSV and XLSX with a column mapping, and links an external spreadsheet as a read-only source. | T1 | 4 |
| DB-TELEC-052 | Missing catalog check | Lists components without a catalog entry or with an entry that no longer exists, blocking the bill of materials until resolved when the project requires it. | T1 | 4 |
| DB-TELEC-053 | Standard switching | Converts a project between IEC, NFPA, JIC, GB and JIS conventions for symbols, wire layers and tag formats, reporting symbols with no equivalent in the target set. | T2 | 4 |

## Reports and audits

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TELEC-054 | Bill of materials report | Generates the bill of materials from catalog entries, with quantities merged by part number, for the project, a section or a panel. | T1 | 4 |
| DB-TELEC-055 | Component, from-to and wire list | Generates component, from-to and wire reports with source, destination, wire number, type and length. | T1 | 4 |
| DB-TELEC-056 | Report output | Inserts reports as tables in the drawing or exports them to CSV, XLSX or the database, using user-defined formats with chosen columns, sort and totals. | T1 | 4 |
| DB-TELEC-057 | Project audit | Checks duplicate tags, missing parents, orphan children, unnumbered wires, missing catalog data, duplicate terminal numbers and unconnected pins, with a navigate-to-object action per finding. | T1 | 4 |
| DB-TELEC-058 | Title block update | Fills title block fields from project and drawing properties and descriptions, and updates all sheets at the next project update. | T1 | 4 |
| DB-TELEC-059 | Drawing revision tracking | Records revision data per drawing and tracks which drawings changed since the last release, from the drawing journal. | T3 | 5 |

## Interoperability

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TELEC-060 | Plain-entity output | Exploded output keeps symbols as blocks with attributes for tag, description and catalog data, wires as polylines and tables as plain tables. | T1 | 4 |
| DB-TELEC-061 | Netlist export | Exports components and connections as a netlist in an open text format for other electrical tools. | T3 | 5 |

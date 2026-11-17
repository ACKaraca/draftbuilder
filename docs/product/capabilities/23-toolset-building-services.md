# Building services toolset

The building services toolset adds duct, pipe, cable tray, conduit and plumbing objects to DraftBuilder, with equipment and parts catalogs, system definitions, automatic routing and sizing, schematic design, single-line to double-line conversion, electrical circuits and panels within drawings, interference detection, schedules and display by system. It builds on the framework in [20-toolset-framework.md](20-toolset-framework.md) and can use architecture drawings as references.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| System definition | Name, discipline, color, linetype, size rules, routing preference, flow medium | Applied to connectors; objects inherit its display and rules |
| Routing preference | Fitting policy (mitre or radius, branch type), size transitions, slope rules | Belongs to a system definition |
| Duct | Shape (rectangular, round, oval), width, height or diameter, elevation, slope, justification, insulation and lining | Connects through connectors; belongs to a system |
| Duct fitting | Kind (elbow, tee, wye, transition, offset), geometry, loss data | Generated between ducts; regenerates on change |
| Flex duct | Path, diameter, length limit | Connects a terminal to a duct |
| Pipe | Size, elevation, slope, material, insulation | Connects through connectors; belongs to a system |
| Pipe fitting, valve | Catalog part, size, ends | Generated or placed; carries flow data |
| Cable tray, conduit | Width, depth, size, fittings, fill | Carries circuits; belongs to a system |
| Plumbing fixture | Catalog part, fixture units, connectors | Connects to pipes |
| Equipment | Catalog part, capacity, connectors, clearance zones | Source or sink in systems |
| Electrical device | Symbol, load, circuit assignment | Belongs to a circuit; placed on a level |
| Panel | Name, voltage, phases, positions | Holds circuits; scheduled |
| Circuit | Number, panel, devices, load, wire size | Connects devices to a panel |
| Connector | Position, direction, shape, size, flow, system | Links objects; snapping and propagation endpoint |
| Schematic symbol | Symbol, system, size data, link to model object | Schematic line linkage |
| Part catalog item | Parametric geometry, size table, connectors | Defines placeable parts |
| Interference result | Object pair, overlap volume, status | Produced by the checker |

## Duct and air side

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMEP-001 | Duct creation | Draws rectangular, round and oval ducts from a system by picking points, with size, elevation, slope and justification; fittings generate automatically where ducts meet. | T1 | 4 |
| DB-TMEP-002 | Duct fitting rules | Chooses elbow, tee, wye, transition and offset fittings from the routing preference, with mitre or radius bends and branch type, and regenerates them when a duct moves or resizes. | T1 | 4 |
| DB-TMEP-003 | Routing preferences | Defines the fitting and size-transition policy of a system in one named record that applies to every route in that system. | T1 | 4 |
| DB-TMEP-004 | Duct sizing | Sizes ducts by equal friction, velocity reduction or static regain from the airflows on terminals, and writes the sizes through the route in one undoable command with a result table. | T2 | 4 |
| DB-TMEP-005 | Flexible duct | Draws flexible duct from a terminal to a duct tap with a length limit check, and offers splitting at a chosen length. | T2 | 4 |
| DB-TMEP-006 | Air terminals and equipment | Places terminals and air handling equipment with connectors, airflow data and clearance zones, and flags clearance conflicts. | T1 | 4 |
| DB-TMEP-007 | Insulation and lining | Adds insulation and lining as extra display contours with thickness, included in the size reported for interference and in the quantity take-off. | T2 | 4 |
| DB-TMEP-008 | Convert lines to ducts | Converts lines and polylines to duct runs with a system and size, creating fittings at junctions and reporting segments that cannot convert. | T1 | 4 |
| DB-TMEP-009 | Duct edit | Moves, resizes, splits, merges and changes elevation or slope of runs, with connected fittings and branches adjusting. | T1 | 4 |

## Piping, plumbing and fire protection

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMEP-010 | Pipe creation | Draws pipes with a system, size, elevation and slope; fittings generate automatically from the routing preference and the part catalog. | T1 | 4 |
| DB-TMEP-011 | Fittings and valves | Provides a parametric catalog of fittings and valves by size, ends and material, placed on a pipe with the pipe splitting around them. | T1 | 4 |
| DB-TMEP-012 | Pipe sizing | Sizes pipes from fixture units or flow with a friction or velocity limit, and writes sizes to the run; sizing results include the method and limits used. | T2 | 4 |
| DB-TMEP-013 | Slope and invert data | Applies slope to gravity runs and reports invert elevations at each end and fitting, flagging reverse slope. | T1 | 4 |
| DB-TMEP-014 | Plumbing fixtures | Places fixtures with connectors and fixture unit values and connects them with routes that follow the system's rules. | T1 | 4 |
| DB-TMEP-015 | Pipe insulation | Adds insulation with thickness by system and size and includes it in take-off and clash volumes. | T2 | 4 |
| DB-TMEP-016 | Route around obstructions | Offers an offset route around selected obstacles (beams, other services) with the elevation change and fittings generated. | T3 | 4 |
| DB-TMEP-017 | Riser diagrams | Generates a riser diagram from the connected piping across levels with symbols, sizes and flow data, linked to the model so changes update it. | T3 | 5 |
| DB-TMEP-018 | Sprinkler and fire protection layout | Places sprinkler heads by coverage rule on a ceiling or space and routes branch pipes, listing head counts per zone. | T3 | 5 |

## Electrical in drawings

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMEP-019 | Devices and panels | Places devices (outlets, switches, luminaires, fire alarm devices) and panels from symbol libraries by standard, each with load and circuit data. | T1 | 4 |
| DB-TMEP-020 | Cable tray and conduit | Draws cable tray and conduit with fittings generated at bends, tees and risers, with size from the system. | T1 | 4 |
| DB-TMEP-021 | Fill checks | Computes the fill ratio of tray and conduit from the assigned cables, and flags runs that exceed the limit set in the system. | T2 | 4 |
| DB-TMEP-022 | Wiring and circuits | Connects devices to a panel with a home-run wire symbol, assigns circuit numbers and shows the circuit membership in the properties. | T1 | 4 |
| DB-TMEP-023 | Panel schedules | Generates a panel schedule from the circuits with load per phase, balance and totals, refreshed when circuits change. | T1 | 4 |
| DB-TMEP-024 | Load calculation | Sums connected and demand load per circuit, panel and system, with demand factors from a table the user edits. | T2 | 4 |
| DB-TMEP-025 | Lighting layout | Lays out luminaires on a grid or a ceiling pattern from room size and a spacing rule, with a count and mounting height. | T2 | 4 |
| DB-TMEP-026 | Symbol libraries by standard | Provides device symbols for several electrical drawing standards, switchable by project setting. | T1 | 4 |

## Systems, connectivity and schematics

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMEP-027 | System definitions | Defines systems (supply, return, exhaust, cold, hot, sanitary, vent, fire, chilled, steam, power) with color, linetype, size rules and routing preference; objects inherit through their connectors. | T1 | 4 |
| DB-TMEP-028 | Connectors and snapping | Objects expose connectors with position, direction, shape and size; connecting snaps to compatible connectors, rejects incompatible system or size, and moves connected objects together on request. | T1 | 4 |
| DB-TMEP-029 | Connectivity graph | Builds the graph of connected objects per system, and offers a trace upstream and downstream, a disconnected-object report and a selection of the whole connected run. | T1 | 4 |
| DB-TMEP-030 | Schematic design | Draws diagrams from schematic symbols with sizes and system data; lines between symbols form schematic connectivity. | T2 | 4 |
| DB-TMEP-031 | Schematic to model link | A schematic symbol links to a model object; selecting one highlights the other, and size, flow and system data synchronize in the chosen direction. | T3 | 5 |
| DB-TMEP-032 | Schematic to model conversion | Creates model routes from a schematic by choosing the elevation, producing connected ducts, pipes and equipment placeholders with the schematic data. | T3 | 5 |
| DB-TMEP-033 | Single-line to double-line conversion | Converts single-line routes to double-line objects with sizes from the system and fittings generated, and the reverse conversion for schematic displays; the choice of representation is a display setting for routes drawn as objects. | T1 | 4 |

## Catalogs and parts authoring

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMEP-034 | Parts catalog | Provides catalogs of duct, pipe and electrical parts with size tables and connectors, in the open catalog format, searchable by system and size. | T1 | 4 |
| DB-TMEP-035 | Parts browser | Searches and places catalog parts with a preview and a connector view, remembering recent parts. | T1 | 4 |
| DB-TMEP-036 | Part authoring | Builds a parametric part with named dimensions, connector definitions and a size table, and publishes it to a catalog as a revision. | T2 | 4 |
| DB-TMEP-037 | Equipment authoring | Builds equipment from a 3D body, a plan symbol, connectors and clearance zones, with capacity data in a property set. | T2 | 4 |

## Display and documentation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMEP-038 | Display by system | Colors and styles objects by system through the framework display rules, so one drawing shows a coordination view, a single system view or a single-line view by configuration. | T1 | 4 |
| DB-TMEP-039 | Above and below cut plane | Draws objects below the cut plane dashed and objects above hidden or dashed per configuration, so plans show only what belongs to the level. | T1 | 4 |
| DB-TMEP-040 | Tags | Tags show size, flow, elevation, system and part data, placed automatically along runs at an interval or at terminals. | T1 | 4 |
| DB-TMEP-041 | Services schedules | Provides schedules for ducts, pipes, fixtures, equipment, devices and fittings with sizes, lengths, counts and system totals. | T1 | 4 |
| DB-TMEP-042 | Sections and elevations | Generates sections and elevations of services using the framework generated-view process, with insulation and fittings shown in cut. | T2 | 4 |
| DB-TMEP-043 | Architectural backgrounds | References architecture drawings by level, and shows wall, slab and space data as background with the services drawn on top; moving the background reports services that now collide. | T1 | 4 |
| DB-TMEP-044 | Quantity take-off | Extracts duct area, pipe length, fitting counts, insulation area and cable tray length by system and size for costing. | T2 | 4 |
| DB-TMEP-045 | Layer rules for services | Rule tables per layering standard assign objects and system contours to layers, with an audit of exceptions. | T2 | 4 |

## Interference detection

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMEP-046 | Interference check | Tests selected sets (a system, a level, all services) against each other and against architecture and structure objects, producing a list of pairs with overlap volume and location. | T2 | 4 |
| DB-TMEP-047 | Clearance rules | Adds required clearance distance per class pair (insulation, service access, equipment front), reporting violations as interferences with a distinct status. | T3 | 4 |
| DB-TMEP-048 | Interference review | Lists results with status (new, approved, resolved, ignored), zooms to each, isolates the pair and keeps the status across reruns by object IDs. | T2 | 4 |
| DB-TMEP-049 | Interference report | Exports the result list with images and object data as a report and in the BCF format for issue tracking. | T3 | 5 |

## Interoperability

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMEP-050 | IFC export of services | Exports ducts, pipes, fittings, equipment and devices as IFC distribution elements with systems and ports, and a loss report for generic proxies. | T2 | 4 |
| DB-TMEP-051 | Plain-entity output | Exploded output keeps runs as paired lines with fittings as polylines and blocks, with system names on layers. | T1 | 4 |

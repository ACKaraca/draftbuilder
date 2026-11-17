# Mechanical toolset

The mechanical toolset adds 2D mechanical drafting automation to DraftBuilder: a standard parts and features library, mechanical layer management, a structure of assemblies and components, power dimensioning with fits, bills of materials with parts lists and balloons, machine element generators, section and inertia calculators, a 2D finite-element check, hidden-line situations, drafting standards, and title blocks with revision handling. It builds on the framework in [20-toolset-framework.md](20-toolset-framework.md).

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Structure node | Kind (assembly, subassembly, part, standard part), name, reference ID, properties | Parent and children form the drawing structure; owns component geometry |
| Component geometry | Entities, view membership | Attached to one structure node; recognized by views and the BOM |
| Standard part | Catalog item, standard, size, length, material, mass | A structure node and a block with representations (side, front, top, section) |
| Power block | Parametric block, attribute lookup table | Placed like a part; can feed the BOM |
| Hole feature | Kind (drilled, tapped, counterbored, countersunk), diameter, depth, through or blind, pattern | Belongs to a part; listed in hole charts and notes |
| Hole chart | Hole ID letters, rows, origin | Tracks hole features; updates when holes change |
| Fit | System (hole basis, shaft basis), class pair, deviations | Referenced by dimensions and shaft features |
| Mechanical dimension | Kind, tolerance mode, text, fit reference | Associative to geometry |
| Bill of materials | Rows (item, quantity, standard, material, mass), numbering rule | Built from structure nodes; one or more per drawing |
| Parts list | Table, filter, indent levels, overrides | Presents a BOM; linked to balloons |
| Balloon | Item number, style, leader, target | Bound to a BOM row and a component |
| Hide situation | Overlapping components, visible segments | Stored on a view; recomputed when geometry changes |
| Generated element | Kind (shaft, spring, cam, chain, belt, gear), parameters, result checks | Produces geometry, views and BOM rows; reproducible from parameters |
| Section profile | Closed contours, material | Input to the calculators |
| Title block | Fields, revision table | Fed by drawing and project properties |
| Drafting standard | Name, layer set, text and dimension rules, symbol set | Applied per drawing |

## Structure and layers

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMECH-001 | Structure browser | Shows the drawing as a tree of assemblies, parts, standard parts and views. Dragging a node reassigns it to another parent, and a properties pane edits the node's attributes. | T1 | 4 |
| DB-TMECH-002 | Component attachment | Converts selected geometry into a part or assembly node so the BOM and views recognize it. Geometry added later is attached by selecting it and picking the target node. | T1 | 4 |
| DB-TMECH-003 | Mechanical layer management | Provides layers by function (visible, hidden, center, dimension, hatch, annotation) from the drafting standard. New entities pick up the layer of their function, and a standard change recolors them. | T1 | 4 |
| DB-TMECH-004 | Associative views | Detail and assembly views stay linked to their components; editing a part updates every view of it. | T2 | 4 |
| DB-TMECH-005 | Part reference IDs | Assigns each part a stable reference ID separate from the display item number, preserved when numbering changes. | T1 | 4 |
| DB-TMECH-006 | Structure conversion | Converts existing blocks and linework into structure nodes with BOM data, mapping block attributes to part properties through a table and reporting what stays plain. | T2 | 4 |
| DB-TMECH-007 | Layer filters | Saves named filters for the mechanical layer sets so a drawing can show only dimensions, only hidden lines or only a selected assembly's components. | T3 | 4 |

## Standard parts and features

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMECH-008 | Standard parts library | Catalog of fasteners, pins, bearings, seals, keys, retaining rings, springs, profiles, shapes and o-rings organized by standard and size, placed in side, front, top or section representation. | T1 | 4 |
| DB-TMECH-009 | Part properties | Each standard part carries size, material, mass and standard designation computed from catalog data and written to the BOM. | T1 | 4 |
| DB-TMECH-010 | Screw connection assembly | Selects plates and a hole to build a screw, washer and nut stack with the length chosen from the clamped thickness and the next standard length; the connection regenerates when a plate thickness changes. | T1 | 4 |
| DB-TMECH-011 | Library browser | Searches the library by text, standard, category and size, keeps favorites and recent items, and shows a preview before placement. | T1 | 4 |
| DB-TMECH-012 | Parametric power blocks | Defines a custom parametric block with an attribute lookup table of sizes and properties and adds it to the user's library. | T2 | 4 |
| DB-TMECH-013 | Hole features | Creates drilled, tapped, counterbored and countersunk holes, through or blind, singly or in rectangular, polar and linear patterns, in side, end and top views from one definition. | T1 | 4 |
| DB-TMECH-014 | Hole charts | Builds a table of holes with ID letters grouped by type and size; the table and the hole labels update when holes are added, moved or changed. | T1 | 4 |
| DB-TMECH-015 | Hole notes | Places callouts with diameter, depth, thread and count, associative to the hole, with a count update when the pattern changes. | T1 | 4 |
| DB-TMECH-016 | Fits and tolerances | Looks up limit deviations for hole-basis and shaft-basis fits (for example H7 with g6) from the standard tables and shows them in dimensions and in the fit report. | T1 | 4 |
| DB-TMECH-017 | Steel shapes | Places profiles and sections with length and end trim and generates views, with cut length written to the BOM. | T2 | 4 |
| DB-TMECH-018 | Part swap | Replaces a standard part by another standard, size or representation, keeping position and connection; length suggestions follow the clamped thickness. | T2 | 4 |
| DB-TMECH-019 | Thread representation | Draws thread symbols in side and end views per the drafting standard, with thread designation in the notes and BOM. | T1 | 4 |

## Dimensioning and annotation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMECH-020 | Power dimensioning | One dimension command that picks linear, radial, diameter, angular or ordinate from the entity under the cursor and places the dimension in a single click and drag. | T1 | 4 |
| DB-TMECH-021 | Tolerance modes | Dimensions show tolerances as bilateral, limit, basic, or fit, with values from the fit table or typed deviations; changing mode updates the text without losing the value. | T1 | 4 |
| DB-TMECH-022 | Chain, baseline and stacked dimensions | Creates and edits continuous and baseline dimension sets, aligns them, breaks extension lines at intersections and stacks overlapping dimensions with a standard offset. | T1 | 4 |
| DB-TMECH-023 | Geometric tolerancing | Places datum symbols and feature control frames to ISO 1101 or ASME Y14.5 conventions, with a modifier and datum-reference editor and leader association. | T1 | 4 |
| DB-TMECH-024 | Surface texture and weld symbols | Places surface texture and weld symbols from the standard symbol set with attributes for roughness values and weld parameters. | T1 | 4 |
| DB-TMECH-025 | Taper, chamfer and slope notes | Creates notes for tapers, chamfers and slopes from selected edges with the calculated value in the style of the drafting standard. | T2 | 4 |
| DB-TMECH-026 | Centerlines and center marks | Creates center marks and centerlines for circles, holes and shafts, extends them past the outline by the standard amount and updates them when the geometry moves. | T1 | 4 |
| DB-TMECH-027 | Section and detail symbols | Places cutting-plane lines with arrows, section labels, detail circles with scale labels and break lines, with labels tied to the generated view. | T1 | 4 |

## Bill of materials, parts lists and balloons

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMECH-028 | Bill of materials | Builds a BOM from the structure with quantity, standard, material and mass per row; a drawing can hold several BOMs, such as one per assembly level. | T1 | 4 |
| DB-TMECH-029 | Parts list tables | Presents a BOM as a table with automatic numbering, sort, group filter and indent levels, refreshed when the structure changes. | T1 | 4 |
| DB-TMECH-030 | Balloons | Places balloons with styles that snap to components and show the BOM item number; several balloons per item and clusters on one leader are supported, and renumbering updates all balloons. | T1 | 4 |
| DB-TMECH-031 | Item numbering modes | Numbers items by structure order, by sort order or by a manual override, with a lock on numbers already released. | T1 | 4 |
| DB-TMECH-032 | BOM export | Exports a BOM to CSV or XLSX and imports edited columns back by reference ID, reporting unmatched rows. | T2 | 4 |
| DB-TMECH-033 | Row overrides and merge | Merges and splits parts list rows and overrides cell text with the override marked, kept when the list refreshes. | T2 | 4 |
| DB-TMECH-034 | BOM in title block | Writes totals (total mass, part count) and the main assembly's data into title block fields. | T2 | 4 |
| DB-TMECH-035 | Cross-drawing BOM | Assembles one BOM from the structures of several drawings in a project, resolving quantities across assemblies. | T3 | 4 |

## Generators and calculators

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMECH-036 | Shaft generator | Builds a shaft from stepped features (cylinders, cones, hexagons, threads, keyways, grooves, undercuts) with fits, produces the side view and section views, and writes the stock length and diameters to the BOM. | T1 | 4 |
| DB-TMECH-037 | Shaft checks | Computes deflection, bending stress and critical speed for a shaft with given supports and loads, and reports each result with the formula inputs used. | T2 | 4 |
| DB-TMECH-038 | Spring generator | Generates compression, extension, torsion, leaf and disc springs from wire diameter, coil diameter, free length and end style, checks the parameters and draws detail views. | T2 | 4 |
| DB-TMECH-039 | Cam generator | Generates disc and linear cams from a motion law (uniform, harmonic, cycloidal, polynomial) with lift, base radius and follower type, and outputs the profile and a displacement table. | T2 | 4 |
| DB-TMECH-040 | Chain and belt drive generators | Lays out a chain or belt drive from sprocket or pulley sizes and center distance, computes length and link or belt count, and draws the drive with a parts list entry. | T2 | 4 |
| DB-TMECH-041 | Gear generator | Generates spur and helical gears from module, tooth count, pressure angle and profile shift, draws the tooth profile or its simplified representation, and lists data in a gear table. | T2 | 4 |
| DB-TMECH-042 | Gear pair layout | Places a gear pair at the computed center distance and reports ratio and contact data. | T3 | 4 |
| DB-TMECH-043 | Section and inertia calculator | Takes closed contours as a section and computes area, centroid, second moments about the axes, polar moment, principal axes and section modulus, and can place the results as a table. | T1 | 4 |
| DB-TMECH-044 | Beam and shaft deflection | Computes reactions, shear, moment and deflection for beams with supports and loads, drawing the diagrams in the drawing. | T2 | 4 |
| DB-TMECH-045 | Two-dimensional finite-element check | Runs a linear static analysis of a beam or frame model drawn in the drawing, with supports, point and distributed loads, and shows deformed shape and force diagrams; results include the input listing. | T3 | 4 |
| DB-TMECH-046 | Bearing and fit calculators | Calculates bearing life and press-fit pressure from catalog data and the entered loads and fits, and reports the formulas applied. | T3 | 5 |
| DB-TMECH-047 | Calculation reports | Collects the inputs and results of any generator or calculator into a report that can be inserted as a table or exported as a document. | T2 | 4 |
| DB-TMECH-048 | Generator re-edit | A generated element keeps its parameters, so reopening it in the generator edits and regenerates the geometry, views and BOM rows. | T1 | 4 |

## Hidden-line situations

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMECH-049 | Hide situations | Stores visibility rules for overlapping components in a view (which is in front), and recomputes visible and hidden segments when geometry changes. | T1 | 4 |
| DB-TMECH-050 | Hide edit | Adds or removes visible segments by picking, with manual edits flagged and retained at recomputation unless the geometry under them changes. | T2 | 4 |
| DB-TMECH-051 | Hidden line display | Draws hidden segments in the hidden linetype or suppresses them by view setting, and applies a different treatment for hole interiors. | T1 | 4 |

## Drafting standards, title blocks and revisions

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMECH-052 | Drafting standards | Provides standard sets for ISO, ANSI, DIN, JIS, GB, BSI and other national practice covering layers, text, dimension, hatch, symbol and line weight rules; a drawing uses one set. | T1 | 4 |
| DB-TMECH-053 | Standard change | Switches a drawing from one drafting standard to another and updates styles, layers and symbols, reporting what was converted and what has no mapping. | T2 | 4 |
| DB-TMECH-054 | Title blocks | Title blocks with fields for drawing, project and BOM data, sized by sheet format, and updated when properties change. | T1 | 4 |
| DB-TMECH-055 | Revision handling | Maintains a revision table with letter or number, date, description and author; adding a revision updates the title block, marks the revision cloud and can lock released items. | T2 | 4 |
| DB-TMECH-056 | Sheet format library | Sheet sizes, borders, zone grids and fold marks per standard, selectable from a template. | T1 | 4 |
| DB-TMECH-057 | Mechanical template | A template sets units, standard, layers, dimension and text styles, title block and BOM style, used when creating a new mechanical drawing. | T1 | 4 |
| DB-TMECH-058 | Plain-entity output | Exploded output keeps hole charts, parts lists and balloons as tables, text and leaders, and keeps the structure as block names and property data. | T1 | 4 |

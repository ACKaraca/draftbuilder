# Mapping toolset

The mapping toolset brings geographic data into DraftBuilder drawings: connections to GIS feature sources and rasters, coordinate reference systems and transformation, feature styling and thematic maps, object data and attribute tables, queries across drawings and sources, topologies with network and polygon analysis, buffer and overlay, drawing clean-up, import and export of GIS vector formats, map books and plotting, editing features with check-out, surveys and point data, and industry data models. It builds on the framework in [20-toolset-framework.md](20-toolset-framework.md).

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Map drawing | Coordinate reference system, extent, layer list, display scale | Contains layers and drawing objects |
| Data connection | Provider, location or service URL, credentials reference, schema list, live or snapshot mode | Supplies feature classes and rasters to layers |
| Feature class | Name, geometry type, attribute schema, key, CRS | Belongs to a source; rendered by a layer |
| Feature layer | Source feature class, style rules, scale ranges, query filter, label rule | Appears in the map; checked out for editing |
| Raster layer | Source image or tiled service, bands, styling, georeference | Drawn beneath features |
| Surface layer | Elevation raster or TIN, shading rule | Drawn as relief or contours |
| Coordinate reference system | Authority code or custom definition, datum, transformation | Assigned to drawings and sources; used for on-the-fly projection |
| Style rule | Geometry type, condition, symbol, scale range | Belongs to a layer or theme |
| Theme | Classification kind (ranges, unique values), field, color ramp, legend | Produces style rules |
| Object data table | Name, typed fields | Attached to drawing entities; linked to external tables |
| Link template | Key field, external table, field mapping | Associates entities with external records |
| Query | Condition on attributes and location, scope, name | Selects objects; saved in a query library |
| Topology | Kind (node, network, polygon), tolerance, source layers | Built on layers; supports tracing and analysis |
| Map book | Index layer, template layout, scale rule | Produces sheets, one per index feature |
| Check-out set | Source features, lock state, edit session | Links edited drawing objects to source features |
| Survey point | Coordinates, code, description, elevation | Grouped into figures; styled by code |
| Industry data model | Feature classes, domains, defaults, checks | Constrains feature editing in a layer |

## Data connection

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMAP-001 | Feature source connections | Connects to Shapefile, GeoPackage, SQLite, PostGIS and other spatial databases, GeoJSON, KML and OGC web services (WFS, WMS, WMTS), listing feature classes with geometry type and count before adding layers. | T1 | 4 |
| DB-TMAP-002 | Raster and tiled sources | Connects to GeoTIFF and tiled imagery, shows overviews while zoomed out and reads only visible tiles. | T1 | 4 |
| DB-TMAP-003 | Live and snapshot modes | A connection is live (refreshed on a schedule or on demand) or a snapshot copied into the drawing; the mode is visible per layer and switchable. | T1 | 4 |
| DB-TMAP-004 | Connection management | Saves connections by name with credential references held in the system credential store, never in the drawing, and tests a connection and reports the error cause. | T1 | 4 |
| DB-TMAP-005 | Map explorer | Shows a tree of sources, layers, styles and queries with visibility toggles, ordering by drag, and context actions per node. | T1 | 4 |
| DB-TMAP-006 | Schema editing | Creates and edits feature classes in a writable source (GeoPackage, SQLite): fields, types, domains and keys. | T2 | 4 |
| DB-TMAP-007 | Surface and relief layers | Adds elevation rasters and TIN surfaces with hill-shade, color ramp and contour display, with a spot elevation query. | T2 | 4 |
| DB-TMAP-008 | Online base maps | Adds tiled base maps from configured service addresses; the application ships no default service and caches tiles on disk only on request. | T2 | 4 |
| DB-TMAP-009 | Enterprise service connection | Reads layers from GIS server services that expose OGC interfaces, with authentication through the credential store. | T3 | 5 |

## Coordinate reference systems

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMAP-010 | Drawing coordinate system | Assigns a coordinate reference system to the drawing from an authority registry or a custom definition, stored with the drawing and shown in the status bar. | T1 | 4 |
| DB-TMAP-011 | On-the-fly reprojection | Draws every layer in the drawing coordinate system regardless of its source system, using the transformation chosen for each pair. | T1 | 4 |
| DB-TMAP-012 | Coordinate system library | Searches systems by name, code or area of use, lists datum transformations with accuracy, and holds user-defined systems and grid shift files. | T1 | 4 |
| DB-TMAP-013 | Reproject drawing objects | Transforms the geometry of drawing objects from one system to another and reports objects whose accuracy is reduced by the transformation. | T2 | 4 |
| DB-TMAP-014 | Local grids and scale factor | Defines a local grid with an origin, rotation and combined scale factor for survey drawings, applied at the drawing level. | T2 | 4 |
| DB-TMAP-015 | Coordinate display and input | Shows the cursor position in a chosen system and accepts typed coordinates in a different system than the drawing's. | T1 | 4 |

## Styling and thematic maps

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMAP-016 | Style rules | Styles points, lines and areas by rules with conditions on attributes, per scale range; rules are ordered and the first matching rule applies unless set to accumulate. | T1 | 4 |
| DB-TMAP-017 | Thematic classification | Classifies a field by ranges (equal interval, quantile, natural breaks) or by unique value with a color ramp, producing style rules and a legend. | T1 | 4 |
| DB-TMAP-018 | Labels from attributes | Places labels from field values with position, halo, rotation along lines and collision avoidance, per scale range. | T1 | 4 |
| DB-TMAP-019 | Symbols and symbol library | Defines point symbols, line patterns and area fills as reusable library items, shared by styles. | T2 | 4 |
| DB-TMAP-020 | Legends and scale bars | Generates legends from the current styles and scale bars and north arrows tied to the map scale, updating with the map. | T1 | 4 |
| DB-TMAP-021 | Style import and export | Reads and writes layer styles in a documented format and in the SLD standard, reporting rules that cannot be represented. | T3 | 5 |

## Object data and queries

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMAP-022 | Object data tables | Attaches typed fields to drawing entities in named tables, edited in a grid, with the same property machinery as the framework so values appear in schedules. | T1 | 4 |
| DB-TMAP-023 | Link templates | Links entities to records of an external table by a key field, shows linked data and refreshes it on request. | T2 | 4 |
| DB-TMAP-024 | Annotation from data | Places text, labels and tags from field values, which update when the data changes. | T1 | 4 |
| DB-TMAP-025 | Attribute and location queries | Selects objects by attribute conditions and by location (inside, crossing, within distance of) across drawings, layers and connected sources, with a result count and highlight. | T1 | 4 |
| DB-TMAP-026 | Query library | Saves queries by name with parameters; running one creates a selection, a filtered layer or a report. | T2 | 4 |
| DB-TMAP-027 | Attribute table | Shows layer data in a grid with sort, filter, select and calculate columns, with edits written back through commands for editable sources. | T1 | 4 |
| DB-TMAP-028 | Field calculator | Computes field values from expressions over fields, geometry (length, area, centroid) and functions, with preview before apply. | T2 | 4 |

## Topology and analysis

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMAP-029 | Node topology | Builds topology of points and their connections with a tolerance and reports clusters and orphans. | T2 | 4 |
| DB-TMAP-030 | Network topology | Builds a network from line layers, with nodes at ends and intersections, and traces shortest path, connected set, upstream and downstream, and flood with barriers. | T2 | 4 |
| DB-TMAP-031 | Polygon topology | Builds polygons from closed line sets, with centroids and attribute carry-over, and reports gaps, overlaps and dangling lines. | T2 | 4 |
| DB-TMAP-032 | Buffer | Creates buffers with fixed or attribute-driven distance, with dissolve and end-cap options, as new objects or a new layer. | T1 | 4 |
| DB-TMAP-033 | Overlay | Combines two polygon layers by intersect, union, identity or difference, with attribute handling for each source and area-weighted values where requested. | T2 | 4 |
| DB-TMAP-034 | Spatial join and summary | Joins attributes by location and summarizes by group (count, sum, mean, min, max) into a table. | T3 | 4 |
| DB-TMAP-035 | Topology error flagging | Marks topology errors as flagged objects with a list, and offers to fix those with a defined correction. | T2 | 4 |

## Drawing clean-up

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMAP-036 | Clean-up actions | Deletes duplicates, extends undershoots, snaps clustered nodes, dissolves pseudo-nodes, removes short objects, breaks crossing objects and simplifies, each with a tolerance. | T1 | 4 |
| DB-TMAP-037 | Clean-up profiles | Saves an ordered set of actions and tolerances as a profile applied to selected layers, with a preview that marks what would change. | T2 | 4 |
| DB-TMAP-038 | Clean-up report | Reports counts per action and lists objects changed, and the whole run is one undoable command. | T1 | 4 |

## Import and export

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMAP-039 | Vector import | Imports Shapefile, GeoPackage, GeoJSON, GML, KML, MapInfo interchange, DGN and DXF-based GIS data as drawing objects with object data, with a field mapping step and a saved profile. | T1 | 4 |
| DB-TMAP-040 | Vector export | Exports selected objects or layers to Shapefile, GeoPackage, GeoJSON, GML, KML and SQLite with class and attribute mapping, saved as a profile, and a report of geometry that cannot be represented. | T1 | 4 |
| DB-TMAP-041 | Import options | Controls CRS handling on import (assign, reproject, ask), layer mapping, merging of features into polylines and polygons, and handling of invalid geometry. | T1 | 4 |
| DB-TMAP-042 | Elevation data import | Imports digital elevation models as surfaces. | T3 | 5 |
| DB-TMAP-043 | Batch conversion | Runs import and export profiles over many files headless, with a log per file. | T2 | 4 |

## Map books and plotting

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMAP-044 | Map layouts | Builds map layouts with a map viewport at a scale, legend, scale bar, north arrow, title and inset map. | T1 | 4 |
| DB-TMAP-045 | Map books | Generates one sheet per feature of an index layer from a template layout with scale rule, page numbering and adjacent sheet labels. | T2 | 4 |
| DB-TMAP-046 | Publishing | Plots or publishes layouts and map books to PDF and image files, in the application or headless. | T1 | 4 |
| DB-TMAP-047 | Map viewport synchronization | Keeps the map viewport extent and scale synchronized with the index feature when the index layer changes. | T3 | 4 |

## Editing features with check-out

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMAP-048 | Feature check-out | Checks out selected features from a source into the drawing as editable objects with a lock on the source records; the objects keep their source keys. | T2 | 4 |
| DB-TMAP-049 | Check-in with conflict handling | Writes edited, added and deleted objects back to the source, detects records changed by others, and presents conflicts with both versions without discarding either. | T2 | 4 |
| DB-TMAP-050 | Direct editing | Edits features directly in an editable source within an edit session with undo, without bringing them into the drawing. | T2 | 4 |
| DB-TMAP-051 | Edit validation | Applies domain constraints and industry data model checks at edit time and reports violations before check-in. | T2 | 4 |

## Surveys and point data

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMAP-052 | Point data import | Imports delimited point files with column mapping for number, northing, easting, elevation and code, and places points as styled objects. | T1 | 4 |
| DB-TMAP-053 | Point codes and styles | Maps point codes to symbols, layers and labels through a code table, and connects coded points into figures (lines, polygons) by rule. | T2 | 4 |
| DB-TMAP-054 | Point groups and labels | Groups points by query, labels them with number, elevation and description, and creates a surface from a group. | T2 | 4 |

## Industry data models

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMAP-055 | Industry data models | Provides feature class schemas for water, wastewater, stormwater, electric, gas and telecom, with defaults, domains and checks; a model is applied to a layer or a new source. | T2 | 4 |
| DB-TMAP-056 | Data model customization | Edits classes, fields, domains and defaults in a model and saves it as a documented file shared across projects. | T3 | 5 |
| DB-TMAP-057 | Parcel and cadastral basics | Creates parcels from boundary lines with area and bearing-distance labels and a table, and reports closure errors. | T3 | 5 |

## Interoperability

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TMAP-058 | Plain-entity output | Exported drawings keep map features as polylines, points and hatches with object data as extended data, and layer styling as explicit entity color and layer. | T1 | 4 |

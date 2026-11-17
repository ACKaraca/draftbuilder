# Point clouds and geolocation

Point clouds bring scanned reality into the drawing as a snappable, croppable reference, and
geolocation ties the drawing to a coordinate reference system on the Earth. Advanced GIS work and
raster editing belong to the mapping and raster toolsets.

## Domain objects

- **Point cloud reference** — an attached scan with insertion transform, color mode, crop boundaries and display settings; the scan file stays outside the drawing.
- **Point cloud index** — a level-of-detail index built from the source scan and cached next to it.
- **Crop boundary** — a rectangular, polygonal or circular region, inside or outside, that limits displayed points.
- **Geographic location** — the drawing's coordinate reference system, a reference point in drawing and geodetic coordinates, and north direction.
- **Base map** — tiled imagery from a user-configured tile service shown beneath plan views.

## Point clouds

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-REAL-001 | Attach point clouds (`POINTCLOUDATTACH`) | Attaches LAS, LAZ and E57 scans with insertion point, scale and rotation or at their stored coordinates; the reference appears in the references panel with path and status. Structured E57 files keep their individual scans. | T2 | 3 |
| DB-REAL-002 | Background indexing | Unindexed scans are indexed on worker threads into a level-of-detail cache with progress shown; the drawing stays editable and the cloud appears as indexing completes. Indexing can also run from the command-line application. | T2 | 3 |
| DB-REAL-003 | Level-of-detail streaming | The viewport streams points by screen-space density within a configurable point budget, so clouds of billions of points display without loading them fully. | T2 | 3 |
| DB-REAL-004 | Display settings | Point size, density and budget are settings per drawing and per cloud; points can render as round or square with optional lighting by normal. | T2 | 3 |
| DB-REAL-005 | Color modes | Clouds color by scanned RGB, intensity, elevation with a ramp and range, classification with a per-class table, normal direction, or a single object color. | T2 | 3 |
| DB-REAL-006 | Crop (`POINTCLOUDCROP`) | Crops by rectangle, polygon or circle in the current view, keeping inside or outside; crops combine, can be inverted, removed singly or all, and saved as named crop states. | T2 | 3 |
| DB-REAL-007 | Visibility by scan and region | Individual scans and named regions of a cloud can be shown or hidden. | T2 | 3 |
| DB-REAL-008 | Sections and slices | A section plane or a slab of given thickness limits the displayed points, and section linework can be generated as polylines fitted to the points within the slab. | T2 | 3 |
| DB-REAL-009 | Extraction assistance | Picking on a cloud proposes lines, polylines and centerlines fitted to planar edges and cylindrical features, which the user accepts or adjusts; all results are ordinary drawing objects. | T3 | 3 |
| DB-REAL-010 | Point cloud snaps | Object snaps to the nearest point (node), to fitted edges, corners, cylinder centerlines and the intersection of fitted planes work on clouds alongside ordinary snaps. | T2 | 3 |
| DB-REAL-011 | UCS from a fitted plane | The UCS can be aligned to a plane fitted to points picked on a cloud, such as a floor or wall. | T2 | 3 |

## Geolocation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-REAL-012 | Geographic location (`GEOGRAPHICLOCATION`) | Assigns a coordinate reference system from the EPSG registry (through `arc-io-geo`), a reference point given in drawing and in latitude/longitude or projected coordinates, and a north direction; drawing coordinates are not moved. The location is stored in the DWG geographic data object and works offline. | T2 | 3 |
| DB-REAL-013 | Location from file | A location is taken from a KML or KMZ placemark or from a point picked on the base map. | T2 | 3 |
| DB-REAL-014 | Geographic marker | A marker object shows the reference point with its coordinates and can be placed and edited. | T3 | 3 |
| DB-REAL-015 | Coordinate readout | The status bar shows the cursor position in drawing coordinates and, when a location is set, in geodetic or projected coordinates. | T2 | 3 |
| DB-REAL-016 | Base map tiles | Plan views can show road, aerial or other imagery from tile services the user configures, with resolution, brightness and fade settings; tiles are cached for offline use and no account is required. Base maps never plot unless captured. | T3 | 3 |
| DB-REAL-017 | Map capture | A region of the base map is captured as a georeferenced image object that stays in the drawing and plots. | T3 | 3 |
| DB-REAL-018 | Georeferenced rasters | GeoTIFF images and images with world files are placed at their georeferenced position and scale in the drawing's coordinate reference system. | T2 | 3 |
| DB-REAL-019 | Coordinate transformation | Selected content, references or the whole drawing can be transformed from one coordinate reference system to another, with the transformation used and its accuracy reported. | T3 | 3 |
| DB-REAL-020 | Outlier check for survey coordinates | A check reports objects lying far from the bulk of the content, a common cause of zoom-extents and precision problems in drawings at projected survey coordinates, and zooms to each. | T2 | 3 |
| DB-REAL-021 | Location for sun studies | The geographic location, with date and time, drives the sun direction used by lights, shadows and rendering. | T2 | 4 |

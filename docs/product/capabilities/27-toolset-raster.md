# Raster toolset

The raster toolset works with scanned and photographic images inside drawings: insertion and correlation, clean-up (despeckle, deskew, bias correction, invert), manipulation of raster entities, raster-to-vector conversion, text recognition, image transformations and rubber-sheeting, multispectral and elevation raster display, and export. It builds on the framework in [20-toolset-framework.md](20-toolset-framework.md) and shares georeferencing with the mapping toolset.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Raster image | File reference, size, color depth (bitonal, grayscale, color, indexed), resolution, palette, transparency, clip | Inserted in a drawing; correlated; edited in place |
| Correlation | Control point pairs, transformation kind, residuals | Defines the placement of a raster image; shares a coordinate reference system |
| Clip | Rectangular or polygonal boundary, inversion flag | Limits the visible area of a raster image |
| Raster entity | Detected kind (line, arc, circle, polyline, region), pixel extent | Derived from a raster image; moved, copied or erased in place |
| Raster region | Boundary, mask flag | Limits an operation to an area |
| Vectorization profile | Mode, color rules, smoothing, minimum length, gap fill | Controls conversion to drawing entities |
| Recognition result | Text, confidence, position, language | Produced by text recognition; converted to text entities |
| Mosaic | Source images, blend rule, output grid | Combines raster images into one |
| Multiband raster | Bands, band-to-channel mapping, value stretch | Displayed as composite or analysis image |
| Elevation raster | Cell values, vertical unit | Displayed as relief, color ramp or contours; converted to a surface |

## Insertion and correlation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TRAST-001 | Raster insertion | Inserts TIFF (including Group 4 bitonal), PNG, JPEG, BMP and GeoTIFF images by reference or embedded, with the insertion point, scale and rotation entered or picked; unsupported variants are reported with the reason. | T1 | 4 |
| DB-TRAST-002 | Large image handling | Builds overview pyramids and reads tiles on demand, so scans larger than memory open and pan at interactive speed, with the cache stored beside the image on request. | T1 | 4 |
| DB-TRAST-003 | Raster display properties | Sets brightness, contrast, fade and transparency, shows bitonal images in a chosen foreground and background color, and keeps these as display settings that never change the source file. | T1 | 4 |
| DB-TRAST-004 | Correlation from two points | Correlates an image to the drawing from two or more point pairs, with scale and rotation solved and residuals shown. | T1 | 4 |
| DB-TRAST-005 | Georeferencing from files | Reads GeoTIFF tags and world files, and assigns the image a coordinate reference system, reprojecting it to the drawing system when the drawing has one. | T1 | 4 |
| DB-TRAST-006 | Clipping | Clips an image with a rectangle or a closed polyline, retaining the clip after correlation and allowing inversion. | T1 | 4 |
| DB-TRAST-007 | Image properties | Shows and changes color depth, palette, resolution, size and transparency color, and lists images used by the drawing with path status. | T1 | 4 |
| DB-TRAST-008 | Mosaic and merge | Combines several correlated images into one image or a virtual mosaic with a blend rule at overlaps, and writes the result as a new file. | T2 | 4 |
| DB-TRAST-009 | Reference path repair | Relinks missing image files by file identity and search folders, with a batch repair for a folder move. | T2 | 4 |

## Clean-up

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TRAST-010 | Despeckle | Removes isolated pixel groups below a size threshold, with a preview over the visible area and a separate threshold for holes inside objects. | T1 | 4 |
| DB-TRAST-011 | Deskew | Detects the skew of a scan from its dominant lines, or takes the angle from two picked points on a line that should be horizontal or vertical, and rotates the image. | T1 | 4 |
| DB-TRAST-012 | Bias and background correction | Removes an uneven background or scanner bias by estimating the background and normalizing it, so thresholding works across the whole sheet. | T2 | 4 |
| DB-TRAST-013 | Invert | Inverts bitonal, grayscale and color images, including inverting only a selected region. | T1 | 4 |
| DB-TRAST-014 | Thresholding | Converts grayscale or color images to bitonal by a fixed, adaptive or histogram-derived threshold with a live preview. | T1 | 4 |
| DB-TRAST-015 | Smooth, sharpen and noise removal | Applies smoothing, sharpening and noise filters with adjustable strength and a before-and-after preview. | T2 | 4 |
| DB-TRAST-016 | Histogram and tone adjustment | Shows the histogram and adjusts levels, gamma and contrast, producing a new image or a display setting. | T2 | 4 |
| DB-TRAST-017 | Crop, mirror, rotate and resample | Crops, mirrors, rotates by a free angle and resamples an image with a chosen interpolation, writing a new file. | T1 | 4 |
| DB-TRAST-018 | Region masks | Defines regions (rectangle, polygon, color range) that limit any clean-up or conversion operation to or away from an area. | T2 | 4 |
| DB-TRAST-019 | Non-destructive history | Clean-up operations are recorded as a recipe applied to the source image, so the original is untouched and the recipe can be edited, reordered or re-applied to another scan. | T2 | 4 |

## Raster entity manipulation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TRAST-020 | Entity detection | Detects lines, arcs, circles and polylines in a bitonal image from a picked sample or by a window, and highlights them as selectable raster entities. | T2 | 4 |
| DB-TRAST-021 | Entity move, copy and erase | Moves, copies, rotates, scales and erases raster entities in place, filling erased pixels with the background. | T2 | 4 |
| DB-TRAST-022 | Raster fill and region operations | Fills a region with a color or removes everything inside or outside a boundary, with the operation applied as part of the image recipe. | T2 | 4 |
| DB-TRAST-023 | Raster snap | Snaps the cursor to lines, ends and intersections of the raster image as if they were vector geometry. | T2 | 4 |
| DB-TRAST-024 | Hybrid clean-up | Removes the raster pixels underneath chosen vector entities, so a retraced vector replaces its raster counterpart. | T2 | 4 |
| DB-TRAST-025 | Raster edit by pixel | Draws and erases pixels, lines and shapes on a raster image with brush size and color, recorded in the recipe. | T3 | 4 |

## Raster to vector conversion

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TRAST-026 | Manual tracing | Traces lines and polylines on a raster with snapping to the centerline of a stroke, producing drawing entities on the chosen layer. | T1 | 4 |
| DB-TRAST-027 | Automatic vectorization | Converts selected regions or the whole image into lines, polylines and arcs by color or darkness, with smoothing, minimum length, gap fill and corner detection controlled by a profile. | T1 | 4 |
| DB-TRAST-028 | Vectorization profiles | Saves a set of conversion settings as a named profile (line drawing, contour map, cadastral map, schematic) reusable across images and headless. | T2 | 4 |
| DB-TRAST-029 | Hatch and area conversion | Converts filled areas and hatch patterns into closed polylines and hatch entities, with pattern detection for regular hatches. | T3 | 4 |
| DB-TRAST-030 | Contour and elevation labels | Converts scanned contour lines to polylines and reads elevation labels from text recognition to assign elevations to the contours. | T3 | 5 |
| DB-TRAST-031 | Conversion review | Highlights segments added by conversion for review, rejects or merges them with an undoable command, and compares the result with the raster by overlay. | T2 | 4 |

## Text recognition

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TRAST-032 | Text recognition | Recognizes printed text in a selected region of a raster image in a chosen language, producing a recognition result with confidence per word. | T2 | 4 |
| DB-TRAST-033 | Text to entities | Converts recognition results to text entities with the font and height estimated from the image, preserving rotation and position, with low-confidence words flagged. | T2 | 4 |
| DB-TRAST-034 | Text correction review | Lists low-confidence words with their pixel crops for correction, and learns corrections within the drawing's session. | T3 | 5 |
| DB-TRAST-035 | Dimension and symbol text separation | Separates text from lines and symbols before recognition so linework is not read as characters, and sends the remaining linework to conversion. | T3 | 5 |

## Transformation and rubber-sheeting

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TRAST-036 | Rubber-sheeting | Corrects non-linear distortion of a scan from control point pairs with a polynomial or piecewise transformation, shows residuals per point and writes a corrected image or applies the correction as a display transform. | T2 | 4 |
| DB-TRAST-037 | Control point management | Adds, edits, removes and imports control points from a file or from drawing geometry, with residual lists and removal of the worst point in one step. | T2 | 4 |
| DB-TRAST-038 | Reprojection of rasters | Reprojects a georeferenced raster from its coordinate system to another and writes a new raster with the chosen resolution. | T2 | 4 |
| DB-TRAST-039 | Correlation to drawing geometry | Correlates an image by picking pairs between points in the image and existing drawing entities and snaps to the entities' endpoints. | T1 | 4 |

## Multispectral and elevation rasters

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TRAST-040 | Multiband display | Maps bands of a multispectral raster to display channels, with value stretch and band indices (such as a normalized difference of two bands) shown as a computed layer. | T3 | 5 |
| DB-TRAST-041 | Elevation raster display | Shows elevation rasters as color ramp and hill-shade with a legend and a point query for the cell value. | T2 | 4 |
| DB-TRAST-042 | Elevation raster contours | Generates contours at an interval from an elevation raster as polylines with elevation values. | T3 | 5 |
| DB-TRAST-043 | Elevation raster to surface | Converts an elevation raster into a surface for use with the mapping and civil workflows, with a decimation tolerance. | T3 | 5 |
| DB-TRAST-044 | Classification display | Classifies raster values into classes with colors and a legend, and reports class areas. | T3 | 5 |

## Export, batch and interoperability

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TRAST-045 | Export edited raster | Writes an edited image (with its recipe applied) to TIFF, PNG or JPEG, keeping georeferencing tags in GeoTIFF where present, with compression and resolution set by the user. | T1 | 4 |
| DB-TRAST-046 | Export of visible region | Exports the visible or selected area of the drawing's raster, with or without drawing entities, at a chosen resolution. | T2 | 4 |
| DB-TRAST-047 | Batch operations | Applies a recipe, a vectorization profile or an export to many images headless, with a log per image. | T2 | 4 |
| DB-TRAST-048 | Plain-entity output | Saved for other applications, raster images stay as image references with clip and correlation, and recipe results are baked to a new file when the recipe is not supported by the target. | T1 | 4 |

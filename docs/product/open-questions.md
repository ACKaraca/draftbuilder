# DraftBuilder open questions

Behaviors that need verification or a design decision before they are implemented. Each entry names
the capabilities it affects and what closes it; a closed question is recorded in the affected rows or
in an ADR and keeps its number here with the resolution.

## Formats and fidelity

### OQ-01 Drawings older than R13

Should DWG files from R12 and earlier be read directly, or only through DXF? Affects
[DB-INTEROP-001](capabilities/19-interoperability-standards.md). Closed by measuring how often such
files appear in the corpus and whether the public specification covers them sufficiently.

### OQ-02 Starting point for the DWG library

Should `arc-io-dwg` start from an existing permissively licensed pure-Rust DWG library or be written
from the specification without it? Affects [DB-INTEROP-001](capabilities/19-interoperability-standards.md),
[DB-INTEROP-002](capabilities/19-interoperability-standards.md) and
[ADR-0004](../adr/0004-pure-rust-dwg-from-public-specification.md). Closed by a provenance and license
review of candidate libraries and a comparison on the validation corpus.

### OQ-03 Password-protected and signed drawings

How should drawings with password protection or digital signatures be handled: refuse, open
read-only, or open with the user's password, and should signatures be verified or preserved?
Affects [DB-INTEROP-008](capabilities/19-interoperability-standards.md). Closed by confirming what the
public specification documents about these features and deciding the user-facing behavior.

### OQ-04 DGN exchange and underlays

Is a published specification available that covers the DGN versions users exchange? Affects
[DB-INTEROP-016](capabilities/19-interoperability-standards.md) and
[DB-XREF-028](capabilities/12-references-underlays.md). DGN V7 is read from its published specification
([DB-INTEROP-033](capabilities/19-interoperability-standards.md)); the question remains open for later
versions. Closed by a specification review; without one, those versions are exchanged through a
connector, DWG, DXF or PDF.

### OQ-05 DWF and DWFx

Is the published DWF documentation sufficient and permissively usable for reading underlays and
writing sheets? Affects [DB-PLOT-038](capabilities/14-layouts-plotting.md) and
[DB-XREF-028](capabilities/12-references-underlays.md). Closed by a specification and license review
and by demand evidence; PDF remains the primary sheet format either way.

### OQ-06 Sheet set file format

Existing sheet set files use a format without a public specification. Should DraftBuilder define its
own documented sheet set format, and can existing sets be rebuilt from the drawings and layouts they
reference? Affects [DB-PLOT-039 to 045](capabilities/14-layouts-plotting.md). Closed by a format
decision recorded as an ADR.

### OQ-07 3D solid data inside drawings and SAT exchange

3D solids and surfaces in DWG carry modeling data in an embedded format. Which versions of that data
can be read and written from published documentation, and how is it mapped to `arc-brep`? Affects
[DB-M3D-001 to 029](capabilities/15-3d-modeling-visualization.md) and
[DB-INTEROP-013](capabilities/19-interoperability-standards.md). Closed by a specification review;
until closed, such data is preserved byte for byte and displayed through its stored graphics.

### OQ-08 UI customization files of other applications

Should menus, toolbars and shortcuts from other applications' customization files be importable, and
is their format documented? Affects [DB-AUTO-001 to 003](capabilities/18-customization-automation.md).
Closed by a format review; aliases, scripts and Lisp files are covered regardless.

### OQ-26 Plotter configuration import

Which settings of existing plotter configuration files can be imported, given that their format is
not publicly documented? Affects [DB-PLOT-023](capabilities/14-layouts-plotting.md). Closed by a
format review; otherwise devices are configured in DraftBuilder only.

### OQ-28 Field expression compatibility

Which field expression codes found in existing drawings must evaluate, and how are unsupported codes
shown? Affects [DB-TEXT-022 to 025](capabilities/08-text-tables-fields.md). Closed by surveying field
codes in the corpus and documenting the supported set.

### OQ-29 Replacement shape fonts

Which openly licensed shape fonts are shipped as replacements, and which common font names do they
cover with matching metrics? Affects [DB-INTEROP-021](capabilities/19-interoperability-standards.md)
and [DB-TEXT-021](capabilities/08-text-tables-fields.md). Closed by a font license review and a
metric comparison on corpus drawings.

## Customization and automation

### OQ-09 Lisp object-model and event coverage

Which object-model function families and event callbacks of the Lisp dialect are supported, and how
do they map onto the DraftBuilder object model? Affects
[DB-AUTO-022](capabilities/18-customization-automation.md) and
[DB-AUTO-023](capabilities/18-customization-automation.md). Closed by a survey of routines contributed
by users and a published coverage table.

### OQ-10 Out-of-process Lisp performance

Can batched protocol calls give common batch routines acceptable speed out of process? Affects
[DB-AUTO-015](capabilities/18-customization-automation.md) and
[ADR-0005](../adr/0005-lisp-interpreter-out-of-process.md). Closed by the Lisp performance suite
against the target in [overview.md](../architecture/overview.md).

### OQ-13 Setting-name compatibility table

Which widely used setting names must be accepted, and what are their value encodings and scopes?
Affects [DB-CMD-042](capabilities/02-command-interface.md) and
[DB-CMD-043](capabilities/02-command-interface.md). Closed by collecting the names used in
contributed scripts and routines and verifying each value's behavior by observation.

### OQ-19 Assistant scope

Which assistant interactions are offered first, and how are previews and explicit apply presented
for multi-command plans? Affects [DB-AUTO-037](capabilities/18-customization-automation.md). Closed by
a design review with the suite assistant decision.

## Drafting behavior

### OQ-11 Constraint storage in drawings

How are constraint networks stored so other applications preserve them and DraftBuilder reads those
written by others? Affects [DB-CNSTR-023](capabilities/13-parametric-constraints.md) and
[DB-BLOCK-027](capabilities/11-blocks-attributes.md). Closed by documenting the storage layout from
the format reference and corpus observation, and by round-trip tests.

### OQ-12 Dynamic block evaluation fidelity

Do evaluated parameters and actions reproduce the representations stored by other applications for
every parameter and action type, including chained actions and lookup tables? Affects
[DB-BLOCK-021 to 031](capabilities/11-blocks-attributes.md) and
[DB-SEL-029](capabilities/04-selection-grips.md). Closed by comparing evaluated representations with
stored ones across the corpus.

### OQ-17 Repeated-geometry detection thresholds

Which matching tolerances and minimum group sizes make detection of repeated geometry useful without
false matches, and how are near matches presented? Affects
[DB-BLOCK-033](capabilities/11-blocks-attributes.md) and
[DB-BLOCK-034](capabilities/11-blocks-attributes.md). Closed by evaluation on corpus drawings.

### OQ-18 Geometry cleanup categories

Which defect categories (gaps, overlaps, near-orthogonal segments, disjoint chains) and default
tolerances does geometry cleanup check? Affects [DB-MOD-033](capabilities/06-modify-2d.md). Closed by
a review with drafters on representative legacy drawings.

## 3D and reality data

### OQ-20 3D modeling scope

Which surface, mesh and primitive-history operations are needed in the first 3D release, given that
exact modeling is shared with SolidBuilder through `arc-brep`? Affects
[DB-M3D-001 to 029](capabilities/15-3d-modeling-visualization.md). Closed by observing the 3D
workflows users perform in drawings and agreeing the order with the `arc-brep` roadmap.

### OQ-21 Associative drawing views in drawings

How are drawing views generated from 3D stored so they stay associative in DraftBuilder and remain
readable elsewhere? Affects [DB-M3D-033](capabilities/15-3d-modeling-visualization.md) and
[DB-M3D-034](capabilities/15-3d-modeling-visualization.md). Closed by a storage design recorded as an
ADR.

### OQ-22 Visual styles and materials mapping

How do visual styles and materials stored in drawings map to `arc-gpu` shading and to the Arc Scene
material model, and what is lost when writing back? Affects
[DB-M3D-035](capabilities/15-3d-modeling-visualization.md) and
[DB-M3D-046](capabilities/15-3d-modeling-visualization.md). Closed by a mapping table verified on
corpus drawings.

### OQ-23 3D print and solid exchange expectations

Which output checks (closed shells, build volume, units) and which STEP and IGES schemas are
expected for exported solids? Affects [DB-M3D-051](capabilities/15-3d-modeling-visualization.md) and
[DB-INTEROP-012](capabilities/19-interoperability-standards.md). Closed by user interviews and
independent-reader validation.

### OQ-24 Point cloud formats and index

Which point cloud formats are read directly and what is the format of the local index cache? Affects
[DB-REAL-001 to 003](capabilities/16-point-clouds-geolocation.md). Closed by a decision shared with
`arc-io-pointcloud` and Arc Review.

### OQ-25 Base map tile sources

Which tile service protocols are supported, and how are attribution and license terms of
user-configured sources shown? Affects [DB-REAL-016](capabilities/16-point-clouds-geolocation.md) and
[DB-REAL-017](capabilities/16-point-clouds-geolocation.md). Closed by a review of open tile
protocols and attribution requirements.

## Collaboration and history

### OQ-14 Multi-user editing of a file-native drawing

How do borrowed objects, change sets and conflict copies persist when the authoritative file is a
DWG rather than a Hub-native document? Affects
[DB-COLLAB-026 to 029](capabilities/17-compare-history-collaboration.md). Closed by a design shared
with Arc Hub stage 2, recorded as an ADR.

### OQ-15 Compare matching and tolerances

How are objects matched when stable IDs differ (geometric matching), and which tolerances decide
"modified" versus "unchanged"? Affects
[DB-COLLAB-001 to 008](capabilities/17-compare-history-collaboration.md). Closed by evaluation on
pairs of corpus drawings with known edits.

### OQ-16 Markup import scope

Which PDF annotation types are imported as markup overlays, and how are scanned markups aligned?
Affects [DB-COLLAB-013 to 015](capabilities/17-compare-history-collaboration.md). Closed by a review
of PDF annotation types and alignment methods.

### OQ-30 Activity history without Hub identity

What identity is recorded in activity history when no Hub account is connected, and how is it
reconciled later? Affects [DB-COLLAB-011](capabilities/17-compare-history-collaboration.md). Closed
by a privacy review and a decision on local user identity.

## Web, mobile and platform integration

### OQ-31 Storage and memory limits on browser and mobile targets

How large can an offline replica and a loaded drawing be on each target, and what reduced mode is
acceptable beyond the budget? Affects [DB-WEB-007](capabilities/28-web-and-mobile.md) and
[DB-WEB-027](capabilities/28-web-and-mobile.md). Closed by measuring corpus drawings on reference
browsers and devices and recording the per-target budgets.

### OQ-32 Command availability by stage

Which commands belong to stage 2 light editing, and which stage-3 commands need input methods that
touch cannot provide precisely? Affects [DB-WEB-004](capabilities/28-web-and-mobile.md),
[DB-WEB-023](capabilities/28-web-and-mobile.md) and [DB-WEB-024](capabilities/28-web-and-mobile.md).
Closed by usability trials on tablets and phones and a published availability table.

### OQ-33 Automation object-model coverage

Which members of the conventional automation object model must map for the scripts in use, and which
cannot map onto the command model? Affects [DB-AUTO-039](capabilities/18-customization-automation.md)
and [DB-AUTO-040](capabilities/18-customization-automation.md). Closed by a survey of the member
usage in existing scripts and a published mapping table with its coverage.

## Performance

### OQ-27 Performance budgets

Are the targets in [overview.md](../architecture/overview.md) achievable and sufficient on the
reference workstation for the drawings users actually have? Affects
[DB-DBASE-044](capabilities/01-drawing-database.md) and
[DB-CMD-041](capabilities/02-command-interface.md). Closed by benchmarking corpus drawings once the
2D core runs, and by adjusting budgets with recorded rationale.

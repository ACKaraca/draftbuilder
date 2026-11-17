# Web and mobile

DraftBuilder is delivered to browsers, tablets and phones as additional targets of the same crates
and the same UI kit, not as separate applications. Users open, view, measure, mark up and edit
drawings with touch, pen, keyboard or mouse, keep drawings available offline, and synchronize or work
together through Arc Hub. The delivery decision is [ADR-0007](../../adr/0007-web-and-mobile-targets.md);
the suite decision is
[ADR-0018](https://github.com/ACKaraca/arc-suite/blob/main/docs/adr/0018-web-and-mobile-targets.md).

## Domain objects

- **Target** — a build of DraftBuilder for one platform family: desktop, browser, tablet or phone.
- **Stage** — a level of editing offered on a non-desktop target. Stage 1 is viewing, measuring and markup; stage 2 is light editing; stage 3 is full 2D editing. Every row below names its stage.
- **Offline replica** — a drawing, its journal and its pending change sets stored on the device so the drawing opens and edits with no network.
- **Pending change set** — the journal entries of transactions made offline that have not yet reached Arc Hub.
- **Target availability** — a flag on each registered command stating the stages and targets on which it is offered.

## Targets and stages

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-WEB-001 | Browser build | The same crates, compiled to WebAssembly and rendered with the same UI kit, run in a current desktop or mobile browser without installation. The build is a target of the desktop code base; no separate JavaScript user interface exists. | T3 | 5 |
| DB-WEB-002 | Tablet and phone builds | The same crates run as native applications on iOS and Android tablets and phones, with the same document model, command registry and file behavior as the desktop application. | T3 | 5 |
| DB-WEB-003 | Installable and account free | The browser build installs as an application and starts offline once installed; no account, sign-in or network is needed to open a local drawing. Arc Hub features appear when a Hub is configured. | T3 | 5 |
| DB-WEB-004 | Command availability by target | Each registered command declares its stages and targets. A command unavailable on the current target is listed as such in command search with the reason, never silently missing, and its non-interactive form answers with a structured "not available on this target" error. | T3 | 5 |
| DB-WEB-005 | Staged delivery | Stage 1 (view, measure, markup) is complete before stage 2 (light editing) is offered, and stage 2 before stage 3 (full 2D editing). The stage a build reaches is published with its release notes and shown in an about page. | T3 | 5 |

## Opening, saving and viewing (stage 1)

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-WEB-006 | Open and save drawings | Drawings open from the device, from Arc Hub and from a share link, and save back to the same place or as a download or export; DWG and DXF are read and written by the same code as on the desktop, and the fidelity report is shown. | T3 | 5 |
| DB-WEB-007 | Large drawings | Drawings load progressively and display from tiled display lists, so a large drawing becomes navigable before regeneration completes; a memory budget per target is declared and a drawing that exceeds it opens in a reduced mode with the reason stated. | T3 | 5 |
| DB-WEB-008 | Navigation and layers | Pan, zoom, rotate view, named views, layout tabs, viewport display, layer on and off, freeze and isolate, and object properties on selection are available at stage 1. | T3 | 5 |
| DB-WEB-009 | Measurement | Distance, angle, area, coordinates and object lengths are measured with the same snapping and units as the desktop, with results copyable and savable as markup. | T3 | 5 |
| DB-WEB-010 | Markup and issues | Redlines, text notes, clouds, stamps and photographs are drawn as markup overlay items outside the geometry; issues carry BCF-compatible viewpoints; both synchronize with Arc Hub and open in the desktop application ([DB-COLLAB-013 to 015](17-compare-history-collaboration.md), [DB-COLLAB-021](17-compare-history-collaboration.md)). | T3 | 5 |
| DB-WEB-011 | Plot to PDF and image | A layout plots to PDF or an image on the device from the shared sheet pipeline; plot styles, page setups and fonts are the drawing's own, and the result matches the desktop output within the plot comparison tolerance. | T3 | 5 |
| DB-WEB-012 | Fonts and resources | Shape fonts, openly licensed replacement fonts and plot style tables needed by a drawing load on demand and are cached; a missing font is substituted and reported as on the desktop. | T3 | 5 |
| DB-WEB-013 | Heavy operations as Hub jobs | Operations too large for a device, such as publishing a sheet set, auditing, standards checking and batch conversion, are submitted to Arc Hub as headless jobs; progress and the loss report return to the device. | T3 | 5 |

## Touch and pen input

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-WEB-014 | Touch gestures | One finger pans or selects, two fingers pan, zoom and rotate the view, a long press opens the context menu, and a two-finger tap or an on-screen control undoes; gesture meanings are fixed and documented, and mouse and keyboard remain fully usable on the same device. | T3 | 5 |
| DB-WEB-015 | Precise point entry on touch | A point is placed in two steps: the pointer is held and moved while a magnifying loupe shows the snap target and the offset from the finger, then released to accept or dragged off to cancel. Object snaps, polar tracking and numeric entry work as on the desktop. | T3 | 5 |
| DB-WEB-016 | Pen input | A stylus hovers to preview snaps and selection, draws with the tip, and is told apart from touch so a resting palm does not draw; the barrel button maps to a configurable action; pen strokes can be converted to polylines, lines or revision clouds. | T3 | 5 |
| DB-WEB-017 | Touch grips and handles | Grips and selection handles are enlarged on touch targets, can be nudged by fixed steps, and can be dragged with the loupe of DB-WEB-015. | T3 | 5 |
| DB-WEB-018 | Hardware keyboard and mouse | When a keyboard or mouse is present, the full command line, aliases, shortcuts and cursor input of the desktop work on browser and tablet targets. | T3 | 5 |

## Commands and layout on small screens

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-WEB-019 | Command search and on-screen command line | A command search and an on-screen command line with option buttons reach every available command by name or alias without a keyboard; prompts show their keyword options as tappable choices. | T3 | 5 |
| DB-WEB-020 | Context command bar | A bar of commands relevant to the current selection and the active command appears near the work area, with recent and favorite commands; its contents come from the customization files. | T3 | 5 |
| DB-WEB-021 | Adaptive layout | Panels (layers, properties, references, markup) dock as sheets or drawers on small screens, collapse by orientation and width, and keep the drawing visible; interface scale and high-contrast themes follow the system setting. | T3 | 5 |
| DB-WEB-022 | Accessibility and localization | Every control has an accessible name, screen readers and switch access work on all targets, input methods for non-Latin text are supported, and strings come from the same catalogs as the desktop. | T3 | 5 |

## Light and full editing (stages 2 and 3)

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-WEB-023 | Light editing | Stage 2 offers selection, move, copy, rotate, scale, mirror, erase, grips, property and layer changes, text and attribute editing, and drawing of lines, polylines, circles, arcs, rectangles, text and basic dimensions, each as a validated, undoable command. | T3 | 5 |
| DB-WEB-024 | Full 2D editing | Stage 3 offers every 2D drawing, modify, annotation, hatch, block, reference and layout command of files [05](05-draw-2d.md) to [11](11-blocks-attributes.md) and [13](13-parametric-constraints.md) with their prompt machines, so a drawing edited on a device is indistinguishable from one edited on the desktop. | T3 | 5 |
| DB-WEB-025 | Same command model | Commands on every target run the same registered commands inside transactions with the same journal, undo and redo; no editing path bypasses them, and the automation protocol is available to the Hub runner and to scripts the target supports. | T3 | 5 |
| DB-WEB-026 | Scripts and routines | Script files run on every target that offers the commands they use; Lisp routines run through the out-of-process interpreter hosted by an Arc Hub runner rather than on the device, and the target reports when a routine cannot run locally. | T3 | 5 |

## Offline use, synchronization and shared sessions

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-WEB-027 | Offline drawings | A drawing, its references, fonts and plot resources are made available offline by an explicit action or a project setting; an offline replica opens, displays, measures, marks up and, at the stage the target offers, edits with no network. Stored data can be listed and removed by the user. | T3 | 5 |
| DB-WEB-028 | Offline edits | Edits made offline are recorded as pending change sets in the journal, survive closing the application and a crash, and are shown as pending until they reach Arc Hub. | T3 | 5 |
| DB-WEB-029 | Synchronization through Arc Hub | Pending change sets, markup and issues are exchanged with Arc Hub when a connection exists; non-conflicting changes merge, and conflicting changes keep both states for the user to resolve as in [DB-COLLAB-029](17-compare-history-collaboration.md); no change is discarded. | T3 | 5 |
| DB-WEB-030 | Shared sessions | A browser or device joins a multi-user editing session on a drawing, borrows objects or a region, and receives others' change sets, with the rules of [DB-COLLAB-026 to 028](17-compare-history-collaboration.md); participants, their borrowed objects and their view positions are shown. | T3 | 5 |
| DB-WEB-031 | Device capabilities | The camera attaches photographs as markup or images, device location sets or checks the geographic location, and stored files are reachable through the system file picker; each use needs the user's permission and no drawing content leaves the device without a user action. | T3 | 5 |

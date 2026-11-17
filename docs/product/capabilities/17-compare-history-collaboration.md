# Comparison, history, markup and collaboration

DraftBuilder treats change as first-class data: drawings and versions are compared object by
object, every object has a history from the operation journal, markups live beside the drawing
without altering it, and teams share drawings through an optional Arc Hub up to element-level
multi-user editing.

## Domain objects

- **Comparison** — the result of comparing two drawing states: added, removed and modified objects with property differences.
- **Journal entry** — a recorded transaction with its object operations, author and time, kept in the `.draftb` sidecar package.
- **Markup overlay** — a non-destructive layer of comments, sketches and imported markups aligned to the drawing and stored outside its geometry.
- **Lock file** — a file next to an open drawing that records who has it open.
- **Hub version** — a stored version of a drawing in Arc Hub with lifecycle state, reviews and issues.
- **Borrow** — an exclusive editing right on objects or a region of a shared drawing, held by one user.
- **Change set** — a group of journal transactions sent between users in a multi-user session.

## Comparison

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-COLLAB-001 | Compare drawings (`COMPARE`) | Compares the current drawing with another drawing or with an earlier saved version, matching objects by stable ID first and by geometry second. Results open in a comparison view without modifying either drawing. | T2 | 2 |
| DB-COLLAB-002 | Difference classes | Each difference is classified as added, removed or modified, and modified objects list every changed property with old and new values, including geometry, layer, style, attribute values and block definitions. | T2 | 2 |
| DB-COLLAB-003 | Compare settings | Geometric tolerance, object types, layers and properties to ignore, and whether to include content outside the current view are settings saved with the comparison. | T2 | 2 |
| DB-COLLAB-004 | Difference display and navigation | Differences display in distinct colors with optional revision clouds around groups; next/previous navigation zooms to each difference and the list filters by class, layer and type. | T2 | 2 |
| DB-COLLAB-005 | Export differences | Differences export as revision clouds and labels on a chosen layer of the current drawing, as markup overlay items, and as a machine-readable and a printable report. | T2 | 2 |
| DB-COLLAB-006 | Reference compare | When an attached reference changes on reload, its new state is compared with the previous load and the differences are shown in the host drawing. | T2 | 2 |
| DB-COLLAB-007 | Accept or reject differences | In a comparison between two versions, individual differences can be applied to the current drawing or skipped, each applied difference becoming an undoable command. | T3 | 2 |
| DB-COLLAB-008 | Headless compare | The command-line application compares two drawings and writes the report with an exit code that signals whether they differ, for use in continuous integration of drawing sets. | T3 | 2 |

## History

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-COLLAB-009 | Object timeline view | Selecting an object shows its journal history as a timeline: creation, each modification with the property changes, and the command that made it, with author and time when an identity is configured. Stepping along the timeline previews the object's earlier states in the viewport without changing the drawing. | T2 | 2 |
| DB-COLLAB-010 | Recover deleted objects by region | A window or polygon lists objects deleted inside it in earlier sessions, with the command and time of deletion; chosen objects are recreated with their stable IDs as one undoable command. | T2 | 2 |
| DB-COLLAB-011 | Activity history panel | A panel lists drawing activity grouped by session, author and command type, filters by category, layer and time, and zooms to the affected objects. | T2 | 2 |
| DB-COLLAB-012 | History without the sidecar | When a drawing is opened without its sidecar package (for example after another application saved it), history starts from that open and the user is told that earlier history is unavailable. | T1 | 2 |

## Markup

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-COLLAB-013 | Markup overlays | Comments, sketches, clouds and text are placed on markup overlays that display over the drawing and can be shown, hidden, filtered by author and plotted optionally, without changing drawing objects. Overlays are stored in the sidecar package or on Hub. | T2 | 2 |
| DB-COLLAB-014 | Markup import | PDF and image markups are imported and aligned to the drawing by matching sheet extents or two point pairs; each markup becomes an overlay item that can be marked done, rejected or answered. | T2 | 2 |
| DB-COLLAB-015 | Markup to action | A markup item can be linked to the objects it concerns and to the command that resolved it, so the resolution appears in both the markup and the object history. | T3 | 2 |
| DB-COLLAB-016 | Revision clouds with notes | Revision clouds can carry a note and a revision identifier that appear in the markup list and in fields. | T2 | 1 |

## File sharing and Arc Hub

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-COLLAB-017 | Notify when free | When a drawing is open read-only because another user holds its lock file or check-out, DraftBuilder notifies the user when it becomes available and offers to reopen it for editing, keeping unsaved view state. | T2 | 2 |
| DB-COLLAB-018 | Open and save through Hub | Drawings, their references and sidecar packages open from and save to an Arc Hub project; a local copy is kept so work continues offline and syncs on reconnect. | T2 | 2 |
| DB-COLLAB-019 | Check-out locks | A drawing checked out from Hub is editable by one user while others open it read-only and see who holds it; check-in creates a new version. | T2 | 2 |
| DB-COLLAB-020 | Versions and lifecycle | Hub versions list with author, time and comment; any version opens read-only or is compared with the current one; lifecycle states (such as work in progress, shared, published) are shown and changed according to project permissions. | T2 | 2 |
| DB-COLLAB-021 | Reviews and issues | Reviews and approvals on Hub versions appear in a panel; issues are created on objects or locations with BCF-compatible viewpoints and synchronize with Hub. | T2 | 2 |
| DB-COLLAB-022 | Browser viewing and share links | A Hub share link opens the drawing in the Arc Review browser viewer for viewing and markup without DraftBuilder; markup made there returns as overlay items. | T2 | 2 |
| DB-COLLAB-023 | Hub-aware references | References stored in the same Hub project resolve by project identity rather than by file path, so moving files within the project does not break references. | T2 | 2 |
| DB-COLLAB-024 | Branch and merge | A drawing version can be branched; merging two branches applies non-conflicting object changes automatically and presents conflicting changes with both states kept for the user to choose. No change is discarded without a recorded decision. | T3 | 3 |
| DB-COLLAB-025 | Offline first | Every capability that does not inherently need a server works without network or account; Hub features become available when a Hub is configured. | T1 | 1 |

## Multi-user editing

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-COLLAB-026 | Element-level multi-user editing | Several users edit one drawing at the same time through Hub: each user borrows the objects or a region they edit, and transactions travel to the others as change sets from the journal. Borrowed objects are marked with the holder's name. | T3 | 5 |
| DB-COLLAB-027 | Borrowing | Starting an edit on unborrowed objects borrows them automatically; editing objects borrowed by someone else is refused with the holder named and a request option; borrows are released on synchronize or on request. | T3 | 5 |
| DB-COLLAB-028 | Live updates | Change sets from other users apply between the local user's commands, never during one, and display without regenerating unaffected content; symbol-table changes such as new layers synchronize in the same way. | T3 | 5 |
| DB-COLLAB-029 | Conflict handling | When two change sets cannot both apply, both object states are kept and shown side by side for resolution; the outcome is recorded in the journal. | T3 | 5 |

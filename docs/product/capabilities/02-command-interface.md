# Command interface

The command interface is the keyboard-first way every DraftBuilder command is started, prompted
and completed, plus the workspace, navigation and settings that surround it. The command line,
menus, panels, scripts and the automation protocol all drive the same command registry.

## Domain objects

- **Command** — registry entry with a stable id, a de-facto name, parameters, a non-interactive run and an optional interactive prompt machine.
- **Prompt** — one step of a prompt machine: the expected input, its options with keyboard abbreviations, and a default.
- **Alias** — a short name mapped to a command or to an external program.
- **Setting** — a named value scoped to the drawing, the profile or the session, with a DraftBuilder name and optional compatibility names.
- **Profile** — a named, exportable collection of user settings.
- **Named view** — a saved view of a space with optional camera, UCS and layer snapshot.
- **Workspace** — a saved arrangement of panels, command panels and status bar items.

## Command line

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-CMD-001 | Command line window | A dockable or floating command line shows the active command, the current prompt with its options and default, and a scrollable multi-line history. Typing anywhere in the drawing area when no field has focus sends the keystrokes to the command line. | T1 | 1 |
| DB-CMD-002 | Prompt grammar | Each prompt shows the expected input, its options in brackets with the abbreviation letters marked, and a default in angle brackets. Typing an abbreviation in any case, clicking an option in the command line, or choosing it from the cursor menu selects it; Enter or Space accepts the default. | T1 | 1 |
| DB-CMD-003 | Nested prompts | An option can switch the prompt machine to a sub-state with its own option set (for example arc segments inside a polyline) until the user returns. Undo inside a command steps back one prompt-level action without leaving the command. | T1 | 1 |
| DB-CMD-004 | Remembered defaults | Values such as the last offset distance, fillet radius, circle radius, array counts and text height are remembered per drawing or session and offered as the default next time. | T1 | 1 |
| DB-CMD-005 | Enter, Space and Escape | Enter and Space both confirm input, and on an empty command line repeat the last command; inside text entry Space is a character. Escape cancels the current prompt, a second Escape cancels the command, and a cancelled command leaves no change. | T1 | 1 |
| DB-CMD-006 | Input history | Up and Down arrows recall earlier input lines, including partial commands and coordinates; the history can be copied, searched and cleared and its length is configurable. | T1 | 1 |
| DB-CMD-007 | Autocomplete and suggestions | Typing shows a ranked list of commands, aliases, settings and synonyms that start with or contain the text, with recently used entries first and an icon per entry. Arrow keys or the pointer choose, Tab or Enter accepts; autocorrect suggests the closest command for a misspelled name; each behavior can be turned off. | T1 | 1 |
| DB-CMD-008 | Command search | A searchable command palette lists every command with its label, alias, shortcut and description and starts the chosen one; it also finds commands defined by scripts and the Lisp-dialect interpreter. | T1 | 1 |
| DB-CMD-009 | Aliases | Short aliases map to commands (the conventional one- to four-letter aliases ship as defaults) and to external programs with arguments; aliases are case-insensitive and can target script- or Lisp-defined commands. Edits apply immediately without restarting. | T1 | 1 |
| DB-CMD-010 | Alias file import and export | Alias files in the widely used plain-text alias format import into and export from the alias table, with collisions reported. The DraftBuilder alias file is a documented text format. | T1 | 1 |
| DB-CMD-011 | Repeat and multiple | Enter, Space or the context menu repeats the last command; `MULTIPLE` repeats the following command until cancelled. A recent-commands list offers the last several commands. | T1 | 1 |
| DB-CMD-012 | Transparent commands | A command prefixed with an apostrophe (zoom, pan, view, layer, settings, inquiry, calculator and others marked transparent) runs inside the active command and returns to the interrupted prompt with its state intact. Commands not marked transparent are refused at that point with a message. | T1 | 1 |
| DB-CMD-013 | Non-interactive prompt form | Every command that would open a dialog has a hyphen-prefixed form that asks for each setting at the command line, and every command can also run from its parameter schema with no prompts at all. Scripts, macros, the Lisp-dialect interpreter and the automation protocol never open a dialog. | T1 | 1 |
| DB-CMD-014 | Language-neutral and built-in prefixes | An underscore prefix makes command names and option keywords resolve to their English names in any interface language; a dot prefix runs the built-in command even when a script has redefined or undefined its name. | T1 | 1 |
| DB-CMD-015 | Command redefinition | Scripts and plug-ins can undefine a built-in command name and define a replacement; `REDEFINE` restores the built-in. The dot prefix always reaches the built-in. | T2 | 2 |
| DB-CMD-016 | Command echo control | Whether prompts and input of commands called from scripts and the Lisp-dialect interpreter are echoed to the history is a setting; errors are always shown. | T2 | 1 |
| DB-CMD-017 | Expressions at prompts | Wherever a number, distance, angle or point is expected, an arithmetic expression with units, a calculator call or a Lisp-dialect expression in parentheses is accepted and evaluated before use. | T2 | 1 |
| DB-CMD-018 | Calculator | A calculator panel and a transparent command-line calculator evaluate arithmetic, scientific and vector expressions, convert units, keep named variables and pick points, distances and angles from the drawing; results can be pasted into a prompt or text. | T2 | 1 |

## Pointer, keys and cursor input

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-CMD-019 | Right-click behavior | Right-click opens a context menu or acts as Enter, configurable separately with nothing selected, with objects selected and during a command; a time-sensitive mode treats a quick click as Enter and a held click as the menu. During a command the menu lists Enter, Cancel, recent input, snap overrides, navigation and the current command's options. | T1 | 1 |
| DB-CMD-020 | Context menus by object type | With objects selected, the context menu shows editing commands specific to the selected object types plus clipboard, ordering, grouping and selection commands. | T1 | 1 |
| DB-CMD-021 | Keyboard shortcuts | Shortcuts for file, clipboard, undo, select all, panels and drawing switching are provided and every shortcut can be reassigned or exported. Conflicting assignments are reported when saved. | T1 | 1 |
| DB-CMD-022 | Function-key toggles | Function keys toggle help, text history, object snap, 3D object snap, isometric plane, dynamic UCS, grid, ortho, snap, polar tracking, object snap tracking and cursor input, transparently during any command. | T1 | 1 |
| DB-CMD-023 | Temporary override keys | Holding a configured key temporarily inverts ortho, suspends running snaps or forces a chosen snap for as long as the key is held. | T2 | 1 |
| DB-CMD-024 | Mouse buttons | The left button picks, the wheel zooms about the cursor, holding the middle button pans, double-clicking it zooms to extents, and modifier combinations orbit in 3D. Button assignments are configurable. | T1 | 1 |
| DB-CMD-025 | Double-click actions | Double-clicking an object starts the edit command for its type (text edit, hatch edit, attribute edit, block or reference edit, polyline edit, properties); the mapping is configurable. | T1 | 1 |
| DB-CMD-026 | Cursor input | Optional input fields beside the cursor accept coordinates for the first point, length and angle for later points, and grip distances, with the current prompt and its options shown at the cursor. Tab moves between fields, a typed field locks, the coordinate format (polar or cartesian, relative or absolute) is a setting, and the command line receives the same input. | T1 | 1 |
| DB-CMD-027 | Command preview | Move, copy, rotate, scale, stretch, fillet, chamfer, trim, extend, offset and property changes show their result live before the final pick; the preview never changes the drawing. | T1 | 1 |

## Workspace

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-CMD-028 | Docking workspace | Panels (properties, layers, references, blocks, tool palettes, command line, history, calculator) dock, float, tab, auto-hide and remember their positions per workspace; the layout can be locked. | T1 | 1 |
| DB-CMD-029 | Command panels and toolbars | Commands are grouped in tabbed command panels and toolbars, with contextual tabs that appear for the active editing context (text, hatch, array, block authoring, point clouds) and keyboard access to every button. | T1 | 1 |
| DB-CMD-030 | Workspaces | Named workspaces store which panels, command panels and status bar items are shown; switching applies immediately and workspaces export as documented files. | T2 | 2 |
| DB-CMD-031 | Status bar | The status bar shows coordinates and toggles for grid, snap, ortho, polar, isometric drafting, object snap, tracking, cursor input, lineweight and transparency display, selection cycling, annotation scale and visibility, units and isolation; each item has a settings menu and can be hidden. | T1 | 1 |
| DB-CMD-032 | Drawing tabs and windows | Open drawings appear as tabs with thumbnail previews of their model space and layouts; tabs can be reordered, closed and split side by side, and Ctrl+Tab cycles drawings. | T1 | 1 |
| DB-CMD-033 | Start page | A start page lists recent drawings with pinning and previews, templates and a new-drawing action, without network access or sign-in. | T2 | 1 |
| DB-CMD-034 | Themes and display preferences | Light, dark and high-contrast themes, drawing-area background colors per space, crosshair size, cursor badges and UCS icon style are user settings. | T1 | 1 |
| DB-CMD-035 | Clean screen | A toggle hides all panels and toolbars except the command line to maximize the drawing area and restores them on the second press. | T3 | 1 |
| DB-CMD-036 | Identical features on three platforms | The same executable feature set, commands, settings and file behavior are available on Windows, macOS and Linux; platform differences are limited to native file dialogs and keyboard modifier names. | T1 | 1 |

## Views and navigation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-CMD-037 | Zoom | `ZOOM` provides real-time, window, previous, extents, all, center, object, scale (absolute, relative to the current view and relative to paper space) and in/out forms, and runs transparently. Zoom extents ignores unloaded and frozen content. | T1 | 1 |
| DB-CMD-038 | Pan | `PAN` provides real-time panning and displacement entry, and runs transparently; middle-button panning works inside any command. | T1 | 1 |
| DB-CMD-039 | Named views | Views of model space or a layout are saved by name with optional UCS, layer state snapshot, visual style, camera and category, restored by command, and listed for layouts and plotting. Preset orthographic and isometric views are provided. | T1 | 1 |
| DB-CMD-040 | Tiled model viewports | Model space can be split into tiled viewports, each with its own view, UCS, grid and snap; configurations are saved by name and restored. | T2 | 1 |
| DB-CMD-041 | GPU display of very large drawings | 2D display renders drawings with millions of entities, dense hatches and many layouts through the GPU, keeping pan and zoom interactive while detail streams in, with drafting-quality lines and lineweights. | T3 | 1 |

## Settings, help and access

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-CMD-042 | Settings model | Every setting has a documented DraftBuilder name, type, range, scope (drawing, profile or session) and default; `SETVAR` reads and writes any setting at the command line and lists them with a filter. Drawing-scoped settings save with the drawing. | T1 | 1 |
| DB-CMD-043 | Setting name compatibility | Widely used setting names found in existing scripts, Lisp routines and drawings are accepted as aliases of the corresponding DraftBuilder settings, including bit-flag value encodings. Names with no counterpart are reported rather than silently ignored. | T1 | 1 |
| DB-CMD-044 | Profiles | Named profiles hold user settings (paths, display, selection, drafting aids, file behavior) and export to and import from a documented text file; a profile can be chosen at launch. | T2 | 1 |
| DB-CMD-045 | Launch options | The application accepts launch arguments for a startup script, profile, template, workspace and configuration directory. | T2 | 1 |
| DB-CMD-046 | Help | Help for the current command or prompt opens from F1 and from autocomplete entries, and works offline from bundled documentation. | T1 | 1 |
| DB-CMD-047 | Keyboard-complete operation | Every command, option, panel and dialog is reachable and operable from the keyboard, with visible focus and no pointer-only actions apart from free-hand picking, which always has a typed equivalent. | T1 | 1 |
| DB-CMD-048 | Screen-reader names and scalable interface | Every control exposes an accessible name and role, prompts and command results are announced, and the interface scales and supports high-contrast themes without loss of function. | T1 | 1 |
| DB-CMD-049 | Localization | Interface strings come from message catalogs with English as source and Turkish as the first complete translation, right-to-left layout is supported, and localized command names coexist with the underscore-prefixed English names. | T1 | 1 |

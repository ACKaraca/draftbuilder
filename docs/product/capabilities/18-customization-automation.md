# Customization and automation

Users and organizations adapt DraftBuilder through open, documented customization files and
automate it through scripts, recorded macros, the Lisp-dialect interpreter, the Arc Automation
Protocol, a headless command-line application, node graphs and sandboxed plug-ins. Every
mechanism runs the same registered commands, so automation never opens a dialog and every change
is undoable.

## Domain objects

- **UI customization file** — a documented text file defining command panels, toolbars, menus, context menus, shortcuts, double-click actions, mouse buttons and workspaces; files layer as main, partial and enterprise.
- **Command macro** — a string bound to a UI element that issues command input with cancel, pause and repeat markers.
- **Tool palette** — a panel of tools (blocks, hatches, commands) each with preset properties.
- **Template** — a DWT drawing used as the starting state of new drawings.
- **Script** — a SCR text file of command-line input.
- **Recorded macro** — a documented text file of recorded commands, inputs, pauses and messages.
- **Lisp routine** — LSP source loaded into the out-of-process Lisp-dialect interpreter, optionally with DCL dialog definitions.
- **Automation bridge** — a Windows-only separate process that exposes the DraftBuilder object model to component-automation clients and maps their calls onto registered commands.
- **Plug-in package** — a WebAssembly module with a manifest of requested capabilities and host-assigned trust.
- **Trusted location** — a folder from which executable content may load without confirmation.

## User interface customization

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-AUTO-001 | UI customization editor | An editor creates and edits command panels and their groups, toolbars, menus, context menus per object type and editing mode, keyboard shortcuts, temporary override keys, double-click actions and mouse-button assignments, with search over all commands and a live preview. Changes apply without restarting. | T2 | 2 |
| DB-AUTO-002 | Open customization file format | All customization is stored in documented, diffable text files with a published schema, so organizations version and review them with ordinary tools. Unknown entries written by newer versions are preserved. | T2 | 2 |
| DB-AUTO-003 | Layered customization | A main file, any number of partial files and a read-only enterprise file combine in a defined precedence; the editor shows which file an element comes from and transfers elements between files. | T2 | 2 |
| DB-AUTO-004 | Custom commands | User-defined commands combine a name, label, description, icon, help text and a macro or script action, appear in autocomplete and command search, and can be bound to any UI element or shortcut. | T2 | 2 |
| DB-AUTO-005 | Command macros | Macro strings support cancel-current-command, Enter, pause for user input, transparent calls, language-neutral and built-in prefixes, and repeat-until-cancelled markers in the conventional notation, so existing button macros run unchanged where the commands exist. | T2 | 2 |
| DB-AUTO-006 | Workspaces in customization layers | Workspace definitions (DB-CMD-030) live in the layered customization file, so an enterprise layer can supply locked workspaces with panel positions and visibility, users can reset to them, and the last workspace restores on start. | T2 | 2 |
| DB-AUTO-007 | Tool palettes | Palettes hold tools for blocks, hatches, commands and scripts, each with preset layer, color, scale, rotation and other properties; tools are added by drag and drop from the drawing or content browser. Palettes and palette groups export and import as documented files and can be shared from a folder. | T2 | 2 |

## Templates and startup

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-AUTO-008 | Template authoring | Any drawing saves as a DWT template with a description and measurement system; templates carry layers, styles, units, layouts with title blocks, page setups and settings, and are listed in the new-drawing chooser (DB-DBASE-025). | T1 | 1 |
| DB-AUTO-009 | Startup loading | Scripts, Lisp routines and plug-ins load at application start or per drawing open from a configured list and conventional startup file names, in a documented order. | T2 | 2 |
| DB-AUTO-010 | Trusted locations and secure loading | Executable content (scripts, Lisp routines, macros, plug-ins) loads without confirmation only from trusted locations; a security level setting chooses between loading only from trusted locations, asking for others, or allowing all, and every load is logged. | T1 | 2 |

## Scripts and macro recording

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-AUTO-011 | Scripts (`SCRIPT`) | Runs a SCR file whose lines and spaces are command-line input, using the non-interactive command forms; a running script can pause for a delay, be repeated from the start, stopped with Escape and resumed. An error stops the script and reports the line. | T1 | 1 |
| DB-AUTO-012 | Script compatibility | Scripts written for other DWG applications run when they use conventional command names, option keywords and setting names; unknown commands and settings are reported with the line number instead of being silently skipped. | T1 | 1 |
| DB-AUTO-013 | Macro recording | Records a sequence of commands and inputs, with markers for pausing for user input, showing a message and requesting a new base point; recordings play back from a panel or a UI element. | T2 | 2 |
| DB-AUTO-014 | Edit recorded macros | Recorded values, prompts and messages are editable in a panel, and recordings are stored as documented text files that can be shared and run headless. | T2 | 2 |

## Lisp-dialect interpreter

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-AUTO-015 | Lisp-dialect interpreter | Loads and runs LSP routines written in the Lisp dialect long used for DWG-application customization, implemented in Rust and running in a separate process that talks to DraftBuilder through the automation protocol. A failing or runaway routine cannot crash the application and its pending changes roll back. | T2 | 2 |
| DB-AUTO-016 | Command definition and calls | Functions named with the command prefix become commands with autocomplete and alias support; routines call built-in commands with arguments in either the deferred or the immediate calling form, and commands called from a routine never open dialogs. | T2 | 2 |
| DB-AUTO-017 | Entity access by group codes | Routines read, create, modify and delete objects as association lists of DXF group codes, with entity names valid for the session and handles for persistence; changes made by a routine form one undo step. | T2 | 2 |
| DB-AUTO-018 | Selection sets | Selection set functions support interactive selection with all prompt-level methods, filter lists with logical grouping and wildcards, and adding, removing, counting and iterating members. | T2 | 2 |
| DB-AUTO-019 | User input functions | Point, distance, angle, corner, integer, real, string and keyword input functions with input control flags are routed to the active prompt machine, so they use snaps, tracking, cursor input and the command line like built-in commands. | T2 | 2 |
| DB-AUTO-020 | Settings, tables and dictionaries | Routines get and set settings through the compatibility setting names, search and iterate symbol tables, read and edit dictionaries and xrecords, register application names and read and write extended data. | T2 | 2 |
| DB-AUTO-021 | Language library | Arithmetic, list, string, file, environment, error-handling and higher-order functions of the dialect are provided, including the commonly used extension functions for strings, lists, files and sorting; object snap queries return snapped points. | T2 | 2 |
| DB-AUTO-022 | Object-model functions | The object-property and method functions used by existing routines map onto the DraftBuilder object model for documents, spaces, entities, collections, layers, styles, blocks and layouts; functions without a mapping raise a named error. | T2 | 2 |
| DB-AUTO-023 | Event callbacks | A subset of event callbacks (object modified and erased, command started and ended, drawing opened and saved) is available to routines; callbacks cannot modify the document while another command is running. | T3 | 2 |
| DB-AUTO-024 | Dialog definitions | DCL dialog definition files used by routines are rendered by the native UI with their tiles, actions, lists and images; dialogs never open during non-interactive runs, where the routine receives an error instead. | T2 | 2 |
| DB-AUTO-025 | Development support | A language server and debug adapter give editors completion, go-to-definition, breakpoints, stepping and variable inspection for routines; a console evaluates expressions against the open drawing. | T3 | 2 |
| DB-AUTO-026 | Per-drawing namespace | Each open drawing has its own interpreter namespace by default, with an option to share variables across drawings; load order and namespace rules are documented. | T2 | 2 |

## Automation protocol and headless use

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-AUTO-027 | Arc Automation Protocol | A JSON-RPC endpoint over standard input/output or a local socket exposes every registered command with its parameter schema, queries over the drawing database, selection and settings, and events for document changes. Calls run as transactions and return results or structured errors. | T1 | 1 |
| DB-AUTO-028 | Model Context Protocol endpoint | An MCP endpoint derived from the command registry lets compatible assistants list and call commands and queries, subject to the same permissions and previews as other clients. | T2 | 2 |
| DB-AUTO-029 | Python client | An official Python client library wraps the automation protocol with typed command calls, query helpers and event subscriptions; Python runs in its own process. | T2 | 2 |
| DB-AUTO-030 | Headless command-line application | `draftbuilder-cli` opens drawings without a display to convert, validate, audit, plot, publish, run scripts, Lisp routines and recorded macros over a file set, and serve the automation protocol; each run returns an exit code and a machine-readable report. | T1 | 1 |
| DB-AUTO-031 | Batch runs with dry run | Batch jobs over folders or file lists run in parallel per file, can run in a dry-run mode that reports intended changes without saving, and continue past failing files with per-file results. | T2 | 2 |
| DB-AUTO-032 | Deterministic replay | A recorded command sequence replayed on the same drawing produces the same result on every platform, so automation outputs can be verified in continuous integration. | T3 | 2 |

## Windows automation bridge

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-AUTO-038 | COM automation bridge | On Windows, a separate process registers an automation server that exposes the application, documents, spaces, collections, entities, layers, styles, blocks, layouts and selection sets to component-automation clients such as script hosts, spreadsheets and managed code. It is a client of the Arc Automation Protocol and never loads the drawing database ([ADR-0008](../../adr/0008-windows-integration-crate.md)). | T3 | 5 |
| DB-AUTO-039 | Mapping onto the command model | Every property write and method call of the object model maps onto registered commands or protocol queries and runs as one transaction; a call with no mapping fails with a named error, and the mapping table is published and diffable. | T3 | 5 |
| DB-AUTO-040 | Existing automation scripts | Scripts and macros written against the conventional application object model run through the bridge when they use mapped members; a coverage report lists unmapped members of a script before it runs, and the command model, not the script, decides each result. | T3 | 5 |
| DB-AUTO-041 | Bridge events | Document and command events (opened, saved, object modified or erased, command started and ended) reach bridge clients with the limits of [DB-AUTO-023](18-customization-automation.md); handlers cannot modify the drawing from inside an event. | T3 | 5 |
| DB-AUTO-042 | Bridge security | The server accepts only local callers of the same user, asks once per client before the first connection and records trusted clients, honors trusted locations and the permissions of the protocol, and never opens a dialog during a call. | T3 | 5 |
| DB-AUTO-043 | Headless bridge | The command-line application can host the bridge without a display so batch scripts and server-side jobs on Windows use the same object model; each run returns an exit code and a machine-readable report. | T3 | 5 |

## Extension and assistance

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-AUTO-033 | WebAssembly plug-ins | Plug-ins are WebAssembly modules that register commands, panels and custom object types through the command model; they run sandboxed with no ambient file, network or process access beyond what their manifest requests and the user grants, with memory and time limits. A failing plug-in command rolls back. | T2 | 3 |
| DB-AUTO-034 | Plug-in custom objects | Custom object types defined by plug-ins store their data in the drawing with display graphics and a plain-entity representation, so the drawing opens with correct appearance in other applications and in DraftBuilder without the plug-in. | T3 | 3 |
| DB-AUTO-035 | Plug-in packages | Plug-ins, scripts, routines, palettes and customization files are distributed as packages with a manifest (name, version, requested capabilities, signatures); the host assigns trust and the package manager installs, updates and removes them. | T2 | 3 |
| DB-AUTO-036 | Node graphs | Visual node graphs on `arc-nodes` combine commands, queries and geometry operations to generate or update drawing content; graph results are ordinary objects linked to the graph and regenerate when inputs change. | T3 | 4 |
| DB-AUTO-037 | Assistant panel | An optional assistant panel, connected to a user-configured provider including local models, proposes command sequences, shows their preview and applies them only on explicit confirmation. No drawing content leaves the machine without a user action and no feature depends on an assistant. | T3 | 5 |

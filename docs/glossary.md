# DraftBuilder glossary

Terms used across the DraftBuilder documents, one line each. Suite-wide terms are defined in
[`arc-suite`](https://github.com/ACKaraca/arc-suite).

| Term | Meaning |
|---|---|
| Alias | Short command name, such as `L` for `LINE`, defined in an alias file |
| Annotation scale | The scale at which annotative objects are shown in model space or a viewport |
| Annotative object | Text, dimension, leader, hatch, block or attribute defined at paper size with one representation per annotation scale |
| Associative object | Object whose geometry follows other objects: associative dimension, hatch, array, center mark, constraint |
| Attribute | Named text value carried by a block reference, defined by an attribute definition in the block |
| Automation bridge | Windows-only separate process that maps component-automation calls onto registered commands |
| Block | Named group of entities with a base point, defined once in a block record and placed by block references |
| Block authoring mode | Editing environment for block definitions, their parameters, actions, visibility states and constraints |
| ByBlock, ByLayer | Property values that defer to the containing block reference or to the entity's layer |
| Browser build | DraftBuilder compiled to WebAssembly from the same crates and run in a web browser |
| Change set | The journal entries of one or more transactions sent between collaborators |
| Command preview | Temporary display of a command's result while it waits for input |
| Compatibility name | A widely used setting name accepted from scripts, routines and drawings and mapped to a DraftBuilder setting |
| Connector | Add-in running inside another application through its public API that reads or writes the Arc exchange package |
| Constraint network | Geometric and dimensional constraints, parameters and constrained geometry solved together |
| Cursor input | Input fields, measurements and prompts shown next to the pointer |
| Custom object | Object of a class defined by an application rather than the format itself |
| Dependent symbol | Layer, style or block that belongs to an external reference, shown with the reference name as prefix |
| Display list | Derived drawing primitives built from entities for the viewport |
| Drawing database | The DWG/DXF data model held in memory: header, classes, tables, block records, entities, dictionaries |
| Dynamic block | Block whose references can change shape through parameters, actions and visibility states |
| Element borrowing | Temporary exclusive editing right on objects or a region during multi-user editing |
| Embedded object | Document of another application held in a drawing as an object frame with a cached image |
| Extended data | Typed values attached to an object under a registered application name |
| Extension dictionary | Dictionary owned by an object for application data |
| External reference | Another drawing displayed in the host drawing and loaded from its own file |
| Field | Text placeholder that evaluates to a property, date, file name, formula or other value |
| Fingerprint | GUID that identifies a drawing across saves; with a handle it forms the stable ID |
| Handle | Hexadecimal object identifier unique within a drawing and kept across saves |
| Handle seed | The next handle a drawing will assign; handles are never reused |
| Header setting | Drawing-wide value stored in the file header |
| Layer state | Saved snapshot of chosen layer properties that can be restored |
| Layout | Paper space sheet with a page setup, viewports and annotation |
| Loss report | Structured list of what an open, save, import or export preserved, converted or dropped |
| Markup overlay | Comment layer kept outside drawing geometry and aligned to it |
| Model space | The space where design geometry is drawn at real size |
| Offline replica | Drawing, journal and pending change sets stored on a device for use without a network |
| Non-interactive form | Command execution with complete parameters, from scripts, the protocol or the CLI, that never opens a dialog |
| Object coordinate system | Coordinate system derived from a planar entity's normal in which its geometry is stored |
| Object snap | Exact point on existing geometry picked by the pointer, such as an endpoint or intersection |
| Owner tree | The ownership hierarchy from the drawing root through tables and block records to every object |
| Page setup | Named plot settings: device, paper, area, scale, plot style table and options |
| Paper space | The space of layouts, measured in paper units |
| Plot style table | Table that maps object colors (color-dependent) or named styles to output pen settings |
| Preservation report | Report of unknown classes and data kept, transformed or dropped on open and save |
| Preserved object | Object DraftBuilder does not model, kept with its raw data and proxy graphics |
| Prompt form | Hyphen-prefixed command form that asks for every setting at the command line |
| Prompt machine | State machine of a command that collects its parameters from input tokens |
| Property set | Typed property group attached to toolset objects or plain entities |
| Proxy graphics | Graphics stored with a custom object so applications without its definition can display it |
| Query selection | Selection built from object type, property, operator and value |
| Registered application | Name under which extended data is stored |
| Stage | Level of editing offered on a non-desktop target: viewing and markup, light editing or full 2D editing |
| Sheet set | Ordered collection of layouts across drawings with shared properties, numbering and publishing |
| Sidecar package | `.draftb` file beside a drawing holding history, recovery data, settings and Hub metadata |
| Stable ID | 128-bit object identifier derived from the drawing fingerprint and the handle |
| Symbol table | Table of named records: blocks, layers, linetypes, text styles, dimension styles, UCSs, views, viewports, registered applications |
| Toolset | Optional domain pack of typed objects, styles, commands and content built on the toolset framework |
| Transmittal package | Folder or archive of a drawing with its references, fonts, plot settings and report |
| Transparent command | Command that runs during another command's prompt and returns to it |
| UCS | User coordinate system: a named or temporary origin and axis orientation for input |
| Underlay | Referenced PDF or other sheet content displayed and snapped to but not edited |
| Viewport | Window in a layout showing model space at a scale, with its own layer freezes and overrides |
| Xrecord | Dictionary entry holding an arbitrary typed value list |

# Messages of the errors, commands and progress of draftbuilder-commands. Identifiers are
# registered in crates/draftbuilder-commands/src/error.rs and its command modules.

db-cmd-drawing-not-empty = The document already holds a drawing; open the file into a new document.
db-cmd-no-model-space = The document has no model space.
db-cmd-format-unknown = The file name must end in .dwg or .dxf.
db-cmd-format-mismatch = This command reads DXF files only; use the open command for a DWG file.
db-cmd-format-unsupported = DWG files cannot be written yet; save the drawing as DXF.
db-cmd-target-not-file = The save target exists and is not a regular file; choose another name.
db-cmd-progress-open = Opening the drawing
db-cmd-progress-save = Saving the drawing
db-cmd-open-label = Open DXF drawing
db-cmd-open-description = Reads a DXF file into the document as a drawing with its handles and fingerprint, in one undoable step.
db-cmd-open-drawing-label = Open drawing
db-cmd-open-drawing-description = Reads a DWG or DXF file into the document as a drawing with its handles and fingerprint, in one undoable step.
db-cmd-save-label = Save DXF drawing
db-cmd-save-description = Writes the drawing to a DXF file with the handles and fingerprint it was opened with.
db-cmd-save-drawing-label = Save drawing
db-cmd-save-drawing-description = Writes the drawing to a DXF file, through a temporary file that replaces the target only when it is complete.
db-cmd-new-label = New drawing
db-cmd-new-description = Creates an empty drawing from a unit system or from a template drawing, in one undoable step.
db-cmd-audit-label = Audit drawing
db-cmd-audit-description = Checks the owners, handle bindings and geometry of the drawing and reports each finding; repairs nothing.
db-cmd-purge-label = Purge drawing
db-cmd-purge-description = Removes unused layers, linetypes, blocks, styles and zero-length lines, in one undoable step.

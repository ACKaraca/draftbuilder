# Messages of the errors, commands and progress of draftbuilder-commands. Identifiers are
# registered in crates/draftbuilder-commands/src/error.rs and its command modules.

db-cmd-drawing-not-empty = The document already holds a drawing; open the file into a new document.
db-cmd-no-model-space = The document has no model space.
db-cmd-handles-unaligned = The entities created from the file could not be matched to the handles of the file.
db-cmd-move-unsupported = Objects of the type { $type } cannot be moved yet.
db-cmd-offset-invalid = The offset must be two finite numbers.
db-cmd-progress-open = Opening the DXF drawing
db-cmd-progress-save = Saving the DXF drawing
db-cmd-open-label = Open DXF drawing
db-cmd-open-description = Reads a DXF file into the document as a drawing with its handles and fingerprint, in one undoable step.
db-cmd-save-label = Save DXF drawing
db-cmd-save-description = Writes the drawing to a DXF file with the handles and fingerprint it was opened with.
db-cmd-move-label = Move
db-cmd-move-description = Moves objects by an offset in the plane of the drawing.

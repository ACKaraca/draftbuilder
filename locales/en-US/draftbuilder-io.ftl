# Messages of the loss reasons of draftbuilder-io. Identifiers are registered in
# crates/draftbuilder-io/src/error.rs.

db-io-loss-handle-assigned = { $count } { $kind } objects were written with handles assigned by DraftBuilder, not their source handles.
db-io-loss-fingerprint-assigned = The drawing had no fingerprint; one was assigned and written.
db-io-loss-handle-duplicate = { $count } { $kind } objects shared a handle with an earlier object; they receive assigned handles when the drawing is saved.

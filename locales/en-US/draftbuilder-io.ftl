# Messages of the loss reasons of draftbuilder-io. Identifiers are registered in
# crates/draftbuilder-io/src/error.rs.

db-io-loss-fingerprint-assigned = The drawing had no fingerprint; one was assigned and written.
db-io-loss-layer-zero-created = The drawing had no layer 0; the reserved layer 0 was created.
db-io-loss-layer-name-case-duplicate = Some layer names of the file differ only by case; the product treats each such group as one name, and the count is the number of groups.
db-io-loss-header-value-ignored = A header variable holds a value its setting cannot take; the setting keeps its default.
db-io-loss-header-value-clamped = A header variable holds a number outside its setting's range; it is set to the nearest bound.
db-io-loss-current-layer-fallback = The current layer of the file is frozen or missing; layer 0 is current.
db-io-loss-header-not-written = A header variable of the file is not written back on save; the product keeps only its settings and the coordinate system.

# ADR-0006: Toolset objects are typed objects that degrade to plain entities

Toolset objects are stored as custom objects with proxy graphics and plain-entity fallbacks, so other
applications see and plot correct geometry and DraftBuilder restores the typed object.

- **Status:** accepted
- **Suite context:** [suite decision records](https://github.com/ACKaraca/arc-suite/tree/main/docs/adr)

## Context

Discipline toolsets add walls, pipes, schematic components and other domain objects with parameters,
styles and property sets. Drawings with such objects are shared with partners who do not have the
toolset, who then often see frozen pictures that cannot be selected meaningfully or that are dropped
on export.

## Decision

- Each toolset object is a typed object of a class registered by a toolset module, with a parameter
  schema, style reference, property sets and display representations
  ([20-toolset-framework.md](../product/capabilities/20-toolset-framework.md)).
- In DWG it is written as a custom object of a DraftBuilder class with proxy graphics for the current
  display configuration, plus a documented extension-dictionary record holding plain-entity fallbacks
  (lines, polylines, hatches, text, blocks) on the object's layers.
- An explicit "save for other applications" option writes only the plain entities, with property-set
  values as attributes or extended data according to a documented mapping.
- DraftBuilder without the toolset module treats toolset objects as preserved objects
  ([ADR-0003](0003-lossless-preservation.md)).

## Consequences

- Partners get selectable, plottable geometry even with applications that ignore proxy graphics.
- Saving writes graphics twice for toolset objects; file size grows with fallback detail, bounded by
  the display configuration.
- The fallback layout becomes a public, versioned format that must be documented.

## Alternatives considered

- **Proxy graphics only.** Common practice, but many applications show proxies poorly or drop them.
- **Plain entities only, with data in extended data.** Loses the typed object for DraftBuilder users
  on reopen unless the data is reparsed heuristically.

## Reopen when

File-size measurements on toolset corpus drawings show fallbacks more than doubling typical drawing
size, or partners' applications prove to handle the custom objects directly.

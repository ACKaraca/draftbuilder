//! The objects a selection method tests: the entities of a space that the visibility and layer
//! rules allow, each with the curves of its display list, and a spatial index over their boxes.
//!
//! Rules (DB-LAYER-002, DB-SEL-013, DB-SEL-014): an entity whose `visible` flag is false is never
//! selected; an entity on a layer that is off or frozen is never selected; an entity on a locked
//! layer is selected only when the options allow locked objects. For a block reference the
//! primitives of nested content on an off or frozen layer (in any layer of their chain) are left
//! out of its geometry, and the entity's own layer decides the lock.
//!
//! Geometry: the display list of the entity ([`arc_draft::display::entity_list`]). A stroke gives
//! its whole curve (dashes and clipped pieces do not change the object), a fill, a wipeout mask
//! and every loop of a region give their boundary curves, a point gives the point. Text drawn with
//! a stroke font gives its exact curves. Text drawn with an outline font gives the box of its
//! glyph positions, extended by the em size in text space: the one approximation of this module.
//!
//! The index holds the box of every entity whose geometry is bounded. An entity with an unbounded
//! curve (a line or a ray) is kept apart and always tested exactly.

use std::collections::BTreeMap;

use arc_base::math::Point2;
use arc_base::{Error, ObjectId, Result};
use arc_doc::relationship::ReferenceField;
use arc_doc::{Snapshot, Value};
use arc_draft::display::{DisplayOptions, Primitive, Shape, entity_list};
use arc_draft::types::{ENTITIES, LAYER, entity, layer};
use arc_geom::{Aabb2, Curve2, Polyline2, PolylineVertex, SpatialIndex2};

use crate::error::codes;

/// The most entities one selection call gathers. A larger space fails with
/// `db.cmd.limit_exceeded` rather than selecting a partial set.
pub const MAX_CANDIDATES: usize = 1_000_000;

/// The rules of a selection call.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Options {
    /// Whether objects on locked layers are selected (for editing). Locked objects are
    /// otherwise left out, as DB-SEL-014 requires.
    pub include_locked: bool,
}

/// One piece of geometry of an entity, tested by the selection predicates.
#[derive(Debug, Clone)]
pub(crate) enum Part {
    /// A point.
    Point(Point2),
    /// A curve, bounded or not.
    Curve(Curve2),
}

impl Part {
    /// The tight box of the part; `None` for an unbounded curve.
    fn bounds(&self) -> Option<Aabb2> {
        match self {
            Self::Point(p) => Some(Aabb2::from_corners(*p, *p)),
            Self::Curve(curve) => curve.bounding_box(),
        }
    }
}

/// An entity that a selection method may return, with its geometry.
#[derive(Debug, Clone)]
pub(crate) struct Candidate {
    /// The entity.
    pub(crate) id: ObjectId,
    /// Its draw-order key in its space; larger is drawn later, on top.
    pub(crate) order: i64,
    /// The geometry, never empty.
    pub(crate) parts: Vec<Part>,
    /// The box of the geometry; `None` when a part is unbounded.
    bounds: Option<Aabb2>,
}

/// The candidates of one space, in draw order (ties by identifier), with their spatial index.
pub(crate) struct Candidates {
    /// The candidates, sorted by draw order and then identifier.
    pub(crate) items: Vec<Candidate>,
    /// The position of each candidate in `items`.
    positions: BTreeMap<ObjectId, usize>,
    /// The boxes of the bounded candidates.
    index: SpatialIndex2<ObjectId>,
    /// The candidates with an unbounded part.
    unbounded: Vec<ObjectId>,
}

impl Candidates {
    /// The positions of the candidates whose geometry lies entirely inside `rect`, ascending.
    /// Only bounded candidates can lie inside a finite box.
    pub(crate) fn window(&self, rect: &Aabb2) -> Result<Vec<usize>> {
        let keys = self.index.window(rect)?;
        Ok(self.positions_of(keys))
    }

    /// The positions of the candidates whose geometry may meet `rect`, ascending: every bounded
    /// candidate whose box meets it, and every unbounded one.
    pub(crate) fn crossing(&self, rect: &Aabb2) -> Result<Vec<usize>> {
        let mut keys = self.index.crossing(rect)?;
        keys.extend(self.unbounded.iter().copied());
        Ok(self.positions_of(keys))
    }

    fn positions_of(&self, keys: Vec<ObjectId>) -> Vec<usize> {
        let mut positions: Vec<usize> = keys
            .into_iter()
            .filter_map(|id| self.positions.get(&id).copied())
            .collect();
        positions.sort_unstable();
        positions
    }
}

/// The entities whose owner is `space`, in no particular order.
fn entities_of(snapshot: &Snapshot, space: ObjectId) -> Vec<ObjectId> {
    snapshot
        .referrers(space)
        .filter(|r| r.field == ReferenceField::Owner)
        .filter_map(|r| {
            let handle = snapshot.resolve(r.source).ok()?;
            let key = snapshot.type_key(handle)?;
            ENTITIES.contains(&key.as_str()).then_some(r.source)
        })
        .collect()
}

/// The value of a flag field of `id`, `None` when the object or the field is absent.
pub(crate) fn flag(snapshot: &Snapshot, id: ObjectId, index: usize) -> Option<bool> {
    let handle = snapshot.resolve(id).ok()?;
    match snapshot.get(handle).ok()?.values.get(index) {
        Some(Some(Value::Bool(b))) => Some(*b),
        _ => None,
    }
}

/// The reference of a reference field of `id`.
pub(crate) fn reference(snapshot: &Snapshot, id: ObjectId, index: usize) -> Option<ObjectId> {
    let handle = snapshot.resolve(id).ok()?;
    match snapshot.get(handle).ok()?.values.get(index) {
        Some(Some(Value::Reference(target))) => Some(*target),
        _ => None,
    }
}

/// Whether a layer is off or frozen: its objects are not selected and not drawn.
pub(crate) fn layer_hidden(snapshot: &Snapshot, layer_id: ObjectId) -> bool {
    flag(snapshot, layer_id, layer::ON) == Some(false)
        || flag(snapshot, layer_id, layer::FROZEN) == Some(true)
}

/// The layer named `0`, the layer of an entity that names none.
pub(crate) fn default_layer(snapshot: &Snapshot) -> Option<ObjectId> {
    snapshot
        .iter_type(LAYER)
        .ok()?
        .find(|(_, object)| {
            arc_draft_dxf::fields::name_of(snapshot, Some(object.id)).as_deref() == Some("0")
        })
        .map(|(_, object)| object.id)
}

/// The geometry of one primitive: its parts, with the boundary of a fill or a region and the
/// box of an outline text.
fn parts_of(primitive: &Primitive, out: &mut Vec<Part>) -> Result<()> {
    match &primitive.shape {
        Shape::Point(point) => out.push(Part::Point(*point)),
        Shape::Stroke { curve, .. } => out.push(Part::Curve(curve.clone())),
        Shape::Fill(boundary) | Shape::Mask(boundary) => {
            out.push(Part::Curve(Curve2::Polyline(boundary.clone())));
        }
        Shape::Region(region) => {
            for part in region.parts() {
                let loops = std::iter::once(part.outer()).chain(part.holes());
                for lp in loops {
                    out.extend(lp.curves().iter().cloned().map(Part::Curve));
                }
            }
        }
        Shape::Glyphs {
            size,
            glyphs,
            placement,
            ..
        } => {
            let mut low = None::<(f64, f64)>;
            let mut high = None::<(f64, f64)>;
            for glyph in glyphs {
                let (x, y) = (glyph.position.x, glyph.position.y);
                low = Some(low.map_or((x, y), |(a, b)| (a.min(x), b.min(y))));
                high = Some(high.map_or((x, y), |(a, b)| (a.max(x), b.max(y))));
            }
            if let (Some((x0, y0)), Some((x1, y1))) = (low, high) {
                let corners = [
                    (x0, y0),
                    (x1 + size, y0),
                    (x1 + size, y1 + size),
                    (x0, y1 + size),
                ];
                let vertices = corners
                    .iter()
                    .map(|&(x, y)| PolylineVertex::new(placement * Point2::new(x, y), 0.0))
                    .collect();
                out.push(Part::Curve(Curve2::Polyline(Polyline2::new(
                    vertices, true,
                )?)));
            }
        }
    }
    Ok(())
}

/// Gathers the candidates of `space` under `options`.
///
/// # Errors
///
/// `db.cmd.limit_exceeded` when the space holds more than [`MAX_CANDIDATES`] entities, and the
/// errors of the spatial index and of the polyline of an outline text.
pub(crate) fn gather(snapshot: &Snapshot, space: ObjectId, options: Options) -> Result<Candidates> {
    let ids = entities_of(snapshot, space);
    if ids.len() > MAX_CANDIDATES {
        return Err(
            Error::new(codes::LIMIT_EXCEEDED).with_detail("limit", MAX_CANDIDATES.to_string())
        );
    }
    let default = default_layer(snapshot);
    let mut items = Vec::new();
    for id in ids {
        if flag(snapshot, id, entity::VISIBLE) == Some(false) {
            continue;
        }
        // An entity whose display list fails is left out: it has no geometry to test.
        let Ok(list) = entity_list(snapshot, id, &DisplayOptions::default()) else {
            continue;
        };
        let own = reference(snapshot, id, entity::LAYER).or(default);
        // The entity's own layer decides both rules: an entity that names no layer is on layer 0.
        if own.is_some_and(|l| layer_hidden(snapshot, l)) {
            continue;
        }
        if !options.include_locked
            && own.is_some_and(|l| flag(snapshot, l, layer::LOCKED) == Some(true))
        {
            continue;
        }
        let mut parts = Vec::new();
        for primitive in &list.primitives {
            if primitive.layers.iter().any(|l| layer_hidden(snapshot, *l)) {
                continue;
            }
            parts_of(primitive, &mut parts)?;
        }
        if parts.is_empty() {
            continue;
        }
        let mut bounds: Option<Aabb2> = None;
        for part in &parts {
            match part.bounds() {
                Some(b) => bounds = Some(bounds.map_or(b, |a| a.union(&b))),
                None => {
                    bounds = None;
                    break;
                }
            }
        }
        items.push(Candidate {
            id,
            order: list.order,
            parts,
            bounds,
        });
    }
    items.sort_by(|a, b| a.order.cmp(&b.order).then(a.id.cmp(&b.id)));
    let mut index = SpatialIndex2::new();
    let mut positions = BTreeMap::new();
    let mut unbounded = Vec::new();
    for (position, item) in items.iter().enumerate() {
        positions.insert(item.id, position);
        match item.bounds {
            Some(bounds) => {
                index.insert(item.id, bounds)?;
            }
            None => unbounded.push(item.id),
        }
    }
    Ok(Candidates {
        items,
        positions,
        index,
        unbounded,
    })
}

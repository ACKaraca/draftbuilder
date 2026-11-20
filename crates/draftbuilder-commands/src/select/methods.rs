//! The geometric selection methods as pure functions over a document snapshot: a window
//! (inside), a crossing (any part inside or meeting the boundary), the same two with a polygon, and
//! a fence (a polyline that selects what it crosses).
//!
//! Exact rules. A bounded curve lies inside a closed area when every stretch of it between the
//! points where it meets the boundary lies inside: each stretch is tested at its midpoint. It
//! crosses the area when it meets the boundary or any stretch lies inside. A point lies inside
//! when it is inside or on the boundary. An unbounded curve is never inside a closed area. The
//! boundary contact of a curve with the boundary counts for a crossing, and a curve that only
//! touches the boundary from inside counts as inside. Fills, masks and region loops are tested by
//! their boundaries, and a block reference by all of its primitives (see [`super::candidates`]).
//!
//! Results are in draw order (the candidates' order), which is the order the methods return.

use std::collections::BTreeSet;

use arc_base::math::Point2;
use arc_base::{Error, ObjectId, Result, Tolerance};
use arc_doc::Snapshot;
use arc_geom::{
    Aabb2, Containment, Curve2, Intersection, Loop2, Polyline2, PolylineVertex, Segment2, intersect,
};

use super::candidates::{Options, Part, Pick, gather};
use crate::error::codes;

/// The most vertices of a window or crossing polygon, and of a fence.
pub const MAX_VERTICES: usize = 4_096;

/// A closed area that a window or a crossing tests against.
enum Area {
    /// A rectangle with positive width and height, closed.
    Rect(Aabb2),
    /// A closed loop of straight segments.
    Loop(Loop2),
}

/// A boundary curve with its box, which lets a curve skip the boundary pieces it cannot meet.
type Edge = (Curve2, Option<Aabb2>);

impl Area {
    /// The boundary as curves, each with its box.
    fn edges(&self) -> Result<Vec<Edge>> {
        let curves: Vec<Curve2> = match self {
            Self::Rect(rect) => {
                let (a, b) = (rect.min(), rect.max());
                let corners = [
                    Point2::new(a.x, a.y),
                    Point2::new(b.x, a.y),
                    Point2::new(b.x, b.y),
                    Point2::new(a.x, b.y),
                ];
                (0..corners.len())
                    .map(|i| {
                        let next = corners[(i + 1) % corners.len()];
                        Ok(Curve2::Segment(Segment2::new(corners[i], next)?))
                    })
                    .collect::<Result<_>>()?
            }
            Self::Loop(lp) => lp.curves().to_vec(),
        };
        Ok(curves
            .into_iter()
            .map(|curve| {
                let bounds = curve.bounding_box();
                (curve, bounds)
            })
            .collect())
    }

    /// The box of the area.
    fn bounds(&self) -> Aabb2 {
        match self {
            Self::Rect(rect) => *rect,
            Self::Loop(lp) => lp.bounding_box(),
        }
    }

    /// Whether `point` is inside the area or on its boundary.
    fn contains(&self, point: Point2) -> Result<bool> {
        match self {
            Self::Rect(rect) => Ok(rect.contains_point(&point)),
            Self::Loop(lp) => Ok(lp.classify(point, &Tolerance::DEFAULT)? != Containment::Outside),
        }
    }
}

/// Whether a part lies inside `area`, and whether it meets or enters it.
fn inside_and_touching(part: &Part, area: &Area, edges: &[Edge]) -> Result<(bool, bool)> {
    match part {
        Part::Point(point) => {
            let inside = area.contains(*point)?;
            Ok((inside, inside))
        }
        Part::Curve(curve) => curve_against(curve, area, edges),
    }
}

/// The exact test of a curve against an area: see the module documentation. A boundary piece whose
/// box does not meet the curve's box cannot meet the curve, so it is not intersected.
fn curve_against(curve: &Curve2, area: &Area, edges: &[Edge]) -> Result<(bool, bool)> {
    let tolerance = Tolerance::DEFAULT;
    let own = curve.bounding_box();
    let mut breaks = Vec::new();
    for (edge, edge_box) in edges {
        if matches!((own, edge_box), (Some(a), Some(b)) if !a.intersects(b)) {
            continue;
        }
        for hit in intersect(curve, edge, &tolerance)? {
            match hit {
                Intersection::Point { a, .. } => breaks.push(a),
                Intersection::Overlap { a, .. } => breaks.extend(a),
            }
        }
    }
    let touching = !breaks.is_empty();
    let (low, high) = curve.parameter_range();
    let bounded = low.is_finite() && high.is_finite();
    // An unbounded end is closed at one unit past the outermost break, which lies in the same
    // stretch of the curve.
    let finite: Vec<f64> = breaks.iter().copied().filter(|t| t.is_finite()).collect();
    let fallback = finite.iter().copied().fold(0.0_f64, |acc, t| acc.min(t));
    let top = finite.iter().copied().fold(0.0_f64, |acc, t| acc.max(t));
    let lo = if low.is_finite() { low } else { fallback - 1.0 };
    let hi = if high.is_finite() { high } else { top + 1.0 };
    let mut points = vec![lo];
    let mut inner: Vec<f64> = finite.into_iter().filter(|t| *t > lo && *t < hi).collect();
    inner.sort_by(f64::total_cmp);
    points.extend(inner);
    points.push(hi);
    let mut inside = bounded;
    let mut any = touching;
    let mut sampled = false;
    for pair in points.windows(2) {
        let (u, v) = (pair[0], pair[1]);
        if v <= u {
            continue;
        }
        sampled = true;
        if area.contains(curve.point_at((u + v) / 2.0))? {
            any = true;
        } else {
            inside = false;
        }
    }
    if !sampled {
        let on = area.contains(curve.point_at(lo))?;
        any |= on;
        inside &= on;
    }
    Ok((inside && bounded, any))
}

/// Selects the candidates of `space` whose geometry lies inside `area` (`crossing` false) or
/// meets it (`crossing` true), in draw order.
fn select_in(
    snapshot: &Snapshot,
    space: ObjectId,
    area: &Area,
    crossing: bool,
    options: &Options,
) -> Result<Pick> {
    let candidates = gather(snapshot, space, *options)?;
    let edges = area.edges()?;
    let bounds = area.bounds();
    let positions = if crossing {
        candidates.crossing(&bounds)?
    } else {
        candidates.window(&bounds)?
    };
    let mut ids = Vec::new();
    for position in positions {
        let item = &candidates.items[position];
        let mut all = true;
        let mut any = false;
        for part in &item.parts {
            let (inside, touching) = inside_and_touching(part, area, &edges)?;
            all &= inside;
            any |= touching;
        }
        if (crossing && any) || (!crossing && all) {
            ids.push(item.id);
        }
    }
    Ok(Pick {
        ids,
        skipped: candidates.skipped,
    })
}

/// The rectangle with the corners `first` and `second`. A rectangle with no width or no height has
/// no area to test, so it is refused.
fn rectangle(first: Point2, second: Point2) -> Result<Area> {
    if first.x == second.x || first.y == second.y {
        return Err(invalid("rectangle"));
    }
    Ok(Area::Rect(Aabb2::from_corners(first, second)))
}

/// The closed loop through `points`, in order.
fn polygon(points: &[Point2]) -> Result<Area> {
    if !(3..=MAX_VERTICES).contains(&points.len()) {
        return Err(invalid("polygon"));
    }
    let curves = (0..points.len())
        .map(|i| {
            Segment2::new(points[i], points[(i + 1) % points.len()])
                .map(Curve2::Segment)
                .map_err(|_| invalid("polygon"))
        })
        .collect::<Result<Vec<_>>>()?;
    let lp = Loop2::new(curves, &Tolerance::DEFAULT).map_err(|_| invalid("polygon"))?;
    Ok(Area::Loop(lp))
}

/// The error of a geometry the method cannot use, with its reason.
fn invalid(reason: &str) -> Error {
    Error::new(codes::SELECTION_INVALID).with_detail("reason", reason)
}

/// Selects the objects of `space` that lie entirely inside the rectangle with the corners
/// `first` and `second`.
///
/// # Errors
///
/// `db.cmd.selection_invalid` for a rectangle with no width or height, `db.cmd.limit_exceeded`
/// and the errors of gathering.
pub fn select_window(
    snapshot: &Snapshot,
    space: ObjectId,
    corners: [Point2; 2],
    options: &Options,
) -> Result<Pick> {
    select_in(
        snapshot,
        space,
        &rectangle(corners[0], corners[1])?,
        false,
        options,
    )
}

/// Selects the objects of `space` that have a part inside the rectangle with the corners `first`
/// and `second`, or that cross its boundary.
///
/// # Errors
///
/// As [`select_window`].
pub fn select_crossing(
    snapshot: &Snapshot,
    space: ObjectId,
    corners: [Point2; 2],
    options: &Options,
) -> Result<Pick> {
    select_in(
        snapshot,
        space,
        &rectangle(corners[0], corners[1])?,
        true,
        options,
    )
}

/// Selects the objects of `space` that lie entirely inside the closed polygon through `points`.
///
/// # Errors
///
/// `db.cmd.selection_invalid` for fewer than three or more than [`MAX_VERTICES`] points or a
/// polygon that does not enclose an area, and the errors of [`select_window`].
pub fn select_window_polygon(
    snapshot: &Snapshot,
    space: ObjectId,
    points: &[Point2],
    options: &Options,
) -> Result<Pick> {
    select_in(snapshot, space, &polygon(points)?, false, options)
}

/// Selects the objects of `space` with a part inside the closed polygon through `points`, or that
/// cross its boundary.
///
/// # Errors
///
/// As [`select_window_polygon`].
pub fn select_crossing_polygon(
    snapshot: &Snapshot,
    space: ObjectId,
    points: &[Point2],
    options: &Options,
) -> Result<Pick> {
    select_in(snapshot, space, &polygon(points)?, true, options)
}

/// Selects the objects of `space` that the open polyline through `points` crosses.
///
/// # Errors
///
/// `db.cmd.selection_invalid` for fewer than two or more than [`MAX_VERTICES`] points, and the
/// errors of gathering and of the intersections.
pub fn select_fence(
    snapshot: &Snapshot,
    space: ObjectId,
    points: &[Point2],
    options: &Options,
) -> Result<Pick> {
    if !(2..=MAX_VERTICES).contains(&points.len()) {
        return Err(invalid("fence"));
    }
    let vertices = points
        .iter()
        .map(|p| PolylineVertex::new(*p, 0.0))
        .collect::<Vec<_>>();
    let fence = Curve2::Polyline(Polyline2::new(vertices, false).map_err(|_| invalid("fence"))?);
    let candidates = gather(snapshot, space, *options)?;
    let bounds = Aabb2::from_points(points.iter());
    let mut ids = Vec::new();
    for position in candidates.crossing(&bounds)? {
        let item = &candidates.items[position];
        let mut hit = false;
        for part in &item.parts {
            if let Part::Curve(curve) = part {
                hit |= !intersect(curve, &fence, &Tolerance::DEFAULT)?.is_empty();
            }
        }
        if hit {
            ids.push(item.id);
        }
    }
    Ok(Pick {
        ids,
        skipped: candidates.skipped,
    })
}

/// Selects every object of `space` that the options allow.
///
/// # Errors
///
/// As [`select_window`] without geometry.
pub fn select_all(snapshot: &Snapshot, space: ObjectId, options: &Options) -> Result<Pick> {
    let candidates = gather(snapshot, space, *options)?;
    Ok(Pick {
        ids: candidates.items.iter().map(|item| item.id).collect(),
        skipped: candidates.skipped,
    })
}

/// Selects the topmost object of `space` that the options allow, the one drawn last. Objects are
/// drawn in creation order unless their draw order has changed, so this is the most recently
/// created object, or the topmost one after a draw-order change.
///
/// # Errors
///
/// As [`select_all`].
pub fn select_last(snapshot: &Snapshot, space: ObjectId, options: &Options) -> Result<Pick> {
    let candidates = gather(snapshot, space, *options)?;
    Ok(Pick {
        ids: candidates
            .items
            .last()
            .map(|item| item.id)
            .into_iter()
            .collect(),
        skipped: candidates.skipped,
    })
}

/// The objects of `previous` that the selection rules of `space` allow, in their order and without
/// repetition: an object that is frozen, off, invisible or locked (unless `options` allow locked
/// objects), an identifier that names no entity of `space`, and an identifier that no longer exists
/// are all left out, exactly as the other methods leave them out.
///
/// # Errors
///
/// As [`select_all`].
pub fn select_previous(
    snapshot: &Snapshot,
    space: ObjectId,
    previous: &[ObjectId],
    options: &Options,
) -> Result<Pick> {
    let candidates = gather(snapshot, space, *options)?;
    let allowed: BTreeSet<ObjectId> = candidates.items.iter().map(|item| item.id).collect();
    let mut seen = BTreeSet::new();
    let ids = previous
        .iter()
        .copied()
        .filter(|id| allowed.contains(id) && seen.insert(*id))
        .collect();
    Ok(Pick {
        ids,
        skipped: candidates.skipped,
    })
}

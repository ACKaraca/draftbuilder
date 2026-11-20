//! Pure constructions of the drawing commands, in drawing coordinates: circles and arcs through
//! points, arcs from a center or a chord, the bulge of a polyline arc, regular polygons,
//! ellipses from their axes, rectangles and tangent circles. Nothing here reads the document.
//!
//! Conventions: an angle is in radians and counterclockwise from the positive x axis; a sweep is
//! signed, positive counterclockwise; a bulge is the tangent of a quarter of the sweep, as in
//! `arc-geom`. A chord-based arc is the one whose sweep is the given signed angle, so its center
//! lies to the left of the chord for a positive angle below π.

use std::f64::consts::{PI, TAU};

use arc_base::math::{Point2, Vector2};
use arc_base::{Error, Result, Tolerance};
use arc_geom::{Arc2, Circle2, Ellipse2};

use crate::error::codes;

/// Distances at or below this are zero, in drawing units.
fn linear() -> f64 {
    Tolerance::DEFAULT.linear
}

/// The angle of a vector, in (−π, π].
fn angle_of(v: Vector2) -> f64 {
    v.y.atan2(v.x)
}

/// The counterclockwise angle from `from` to `to`, in [0, 2π).
fn ccw_between(from: f64, to: f64) -> f64 {
    (to - from).rem_euclid(TAU)
}

/// The vector turned a quarter turn counterclockwise.
fn perp(v: Vector2) -> Vector2 {
    Vector2::new(-v.y, v.x)
}

/// The z component of the cross product of two vectors.
fn cross(a: Vector2, b: Vector2) -> f64 {
    a.x * b.y - a.y * b.x
}

/// The unit vector at `angle`.
fn unit_at(angle: f64) -> Vector2 {
    Vector2::new(angle.cos(), angle.sin())
}

/// Whether two points are within the linear tolerance of each other.
#[must_use]
pub(crate) fn coincident(a: Point2, b: Point2) -> bool {
    (b - a).norm() <= linear()
}

/// Fails when two points are closer than the linear tolerance.
fn distinct(a: Point2, b: Point2) -> Result<()> {
    if (b - a).norm() <= linear() {
        Err(Error::new(codes::POINTS_COLLINEAR))
    } else {
        Ok(())
    }
}

/// The bulge of a segment that sweeps `sweep` radians: the tangent of a quarter of the sweep.
#[must_use]
pub(crate) fn bulge_of_sweep(sweep: f64) -> f64 {
    (sweep / 4.0).tan()
}

/// The sweep of a segment with `bulge`: four times the arctangent of the bulge.
#[must_use]
pub(crate) fn sweep_of_bulge(bulge: f64) -> f64 {
    4.0 * bulge.atan()
}

/// The circle through three points.
///
/// # Errors
///
/// `db.cmd.points_collinear` when the points lie on one line or two of them coincide.
pub(crate) fn circle_through(a: Point2, b: Point2, c: Point2) -> Result<Circle2> {
    let (u, v) = (b - a, c - a);
    let d = 2.0 * cross(u, v);
    if d.abs() <= linear() * u.norm().max(v.norm()) {
        return Err(Error::new(codes::POINTS_COLLINEAR));
    }
    let (uu, vv) = (u.norm_squared(), v.norm_squared());
    let offset = Vector2::new((v.y * uu - u.y * vv) / d, (u.x * vv - v.x * uu) / d);
    Circle2::new(a + offset, offset.norm())
}

/// The arc from `a` through `b` to `c`.
///
/// # Errors
///
/// As [`circle_through`].
pub(crate) fn arc_through(a: Point2, b: Point2, c: Point2) -> Result<Arc2> {
    let circle = circle_through(a, b, c)?;
    let center = circle.center();
    let (ta, tb, tc) = (
        angle_of(a - center),
        angle_of(b - center),
        angle_of(c - center),
    );
    let ccw_ac = ccw_between(ta, tc);
    let sweep = if ccw_between(ta, tb) < ccw_ac {
        ccw_ac
    } else {
        ccw_ac - TAU
    };
    Arc2::new(center, circle.radius(), ta, sweep)
}

/// The arc from `start` counterclockwise about `center` to `end`; the radius is the distance
/// from the center to `start`.
///
/// # Errors
///
/// `db.cmd.no_solution` when `start` is at the center or `end` is at the same angle as `start`.
pub(crate) fn arc_center_end(start: Point2, center: Point2, end: Point2) -> Result<Arc2> {
    let radius = (start - center).norm();
    if radius <= linear() {
        return Err(Error::new(codes::NO_SOLUTION));
    }
    let (ts, te) = (angle_of(start - center), angle_of(end - center));
    let sweep = ccw_between(ts, te);
    if sweep <= 0.0 {
        return Err(Error::new(codes::NO_SOLUTION));
    }
    Arc2::new(center, radius, ts, sweep)
}

/// The arc about `center` from `start` through the signed `angle`.
///
/// # Errors
///
/// `db.cmd.no_solution` when `start` is at the center; the arc's own errors for a zero angle or
/// one larger than a full turn.
pub(crate) fn arc_center_angle(start: Point2, center: Point2, angle: f64) -> Result<Arc2> {
    let radius = (start - center).norm();
    if radius <= linear() {
        return Err(Error::new(codes::NO_SOLUTION));
    }
    Arc2::new(center, radius, angle_of(start - center), angle)
}

/// The center and the radius of the arc from `start` to `end` with the signed included `angle`.
fn chord_center(start: Point2, end: Point2, angle: f64) -> Result<(Point2, f64)> {
    let d = end - start;
    let chord = d.norm();
    let half = angle / 2.0;
    let sin = half.sin();
    if chord <= linear() || !angle.is_finite() || sin.abs() <= 1e-12 {
        return Err(Error::new(codes::NO_SOLUTION));
    }
    let left = perp(d) / chord;
    let offset = (chord / 2.0) * (half.cos() / sin);
    Ok((start + d * 0.5 + left * offset, (chord / 2.0) / sin.abs()))
}

/// The arc from `start` to `end` with the signed included `angle` (positive counterclockwise).
///
/// # Errors
///
/// `db.cmd.no_solution` when the points coincide or the angle is a full turn or zero.
pub(crate) fn arc_start_end_angle(start: Point2, end: Point2, angle: f64) -> Result<Arc2> {
    let (center, radius) = chord_center(start, end, angle)?;
    Arc2::new(center, radius, angle_of(start - center), angle)
}

/// The signed included angle of the minor arc of `chord` with `radius`: positive for a
/// counterclockwise arc from start to end, negative for a clockwise one.
///
/// # Errors
///
/// `db.cmd.no_solution` when the radius is zero or not finite, or the chord is longer than the
/// diameter.
pub(crate) fn sweep_of_radius(chord: f64, radius: f64) -> Result<f64> {
    if !radius.is_finite() || radius == 0.0 {
        return Err(Error::new(codes::NO_SOLUTION));
    }
    let ratio = chord / (2.0 * radius.abs());
    if ratio > 1.0 + 1e-12 {
        return Err(Error::new(codes::NO_SOLUTION));
    }
    Ok((2.0 * ratio.min(1.0).asin()).copysign(radius))
}

/// The arc from `start` to `end` with the signed `radius`, the minor arc, counterclockwise for a
/// positive radius.
///
/// # Errors
///
/// As [`sweep_of_radius`], and `db.cmd.points_collinear` when the points coincide.
pub(crate) fn arc_start_end_radius(start: Point2, end: Point2, radius: f64) -> Result<Arc2> {
    distinct(start, end)?;
    let sweep = sweep_of_radius((end - start).norm(), radius)?;
    arc_start_end_angle(start, end, sweep)
}

/// The bulge of the arc through `a`, `b` and `c`.
///
/// # Errors
///
/// As [`arc_through`].
pub(crate) fn bulge_through(a: Point2, b: Point2, c: Point2) -> Result<f64> {
    Ok(bulge_of_sweep(arc_through(a, b, c)?.sweep()))
}

/// The bulge of the arc from `start` counterclockwise about `center` to `end`.
///
/// # Errors
///
/// As [`arc_center_end`].
pub(crate) fn bulge_center(start: Point2, center: Point2, end: Point2) -> Result<f64> {
    Ok(bulge_of_sweep(arc_center_end(start, center, end)?.sweep()))
}

/// The bulge of the arc from `start` to `end` with the signed included `angle`.
///
/// # Errors
///
/// `db.cmd.no_solution` when the angle is a full turn or more, or the points coincide.
pub(crate) fn bulge_angle(start: Point2, end: Point2, angle: f64) -> Result<f64> {
    distinct(start, end)?;
    if angle.abs() >= TAU {
        return Err(Error::new(codes::NO_SOLUTION));
    }
    Ok(bulge_of_sweep(angle))
}

/// The bulge of the arc from `start` to `end` with the signed `radius`.
///
/// # Errors
///
/// As [`arc_start_end_radius`].
pub(crate) fn bulge_radius(start: Point2, end: Point2, radius: f64) -> Result<f64> {
    distinct(start, end)?;
    let sweep = sweep_of_radius((end - start).norm(), radius)?;
    Ok(bulge_of_sweep(sweep))
}

/// The bulge of the arc that leaves `start` along `tangent` and ends at `end`: 0 when the end
/// lies ahead on the tangent line, else the bulge of the circle tangent to `tangent` at `start`
/// that passes through `end`.
///
/// # Errors
///
/// `db.cmd.no_solution` when `end` lies behind `start` on the tangent line or the tangent is
/// zero; `db.cmd.points_collinear` when the points coincide.
pub(crate) fn bulge_direction(start: Point2, tangent: Vector2, end: Point2) -> Result<f64> {
    distinct(start, end)?;
    if tangent.norm() <= 0.0 || !tangent.norm().is_finite() {
        return Err(Error::new(codes::NO_SOLUTION));
    }
    let t = tangent.normalize();
    let d = end - start;
    let n = perp(t);
    let normal = n.dot(&d);
    if normal.abs() <= linear() {
        return if t.dot(&d) > 0.0 {
            Ok(0.0)
        } else {
            Err(Error::new(codes::NO_SOLUTION))
        };
    }
    let center = start + n * (d.norm_squared() / (2.0 * normal));
    let counterclockwise = t.dot(&perp(start - center)) > 0.0;
    let (ts, te) = (angle_of(start - center), angle_of(end - center));
    let sweep = if counterclockwise {
        ccw_between(ts, te)
    } else {
        -ccw_between(te, ts)
    };
    Ok(bulge_of_sweep(sweep))
}

/// The direction of travel at the end of a segment from `start` to `end` with `bulge`, as a
/// unit vector.
#[must_use]
pub(crate) fn end_tangent(start: Point2, bulge: f64, end: Point2) -> Vector2 {
    let angle = angle_of(end - start) + sweep_of_bulge(bulge) / 2.0;
    unit_at(angle)
}

/// How a regular polygon is placed about its center.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PolygonMode {
    /// The vertices lie on the circle through the given point.
    Inscribed,
    /// The middle of an edge lies on the circle through the given point.
    Circumscribed,
}

/// The vertices of a regular polygon of `sides` sides about `center`, counterclockwise.
///
/// # Errors
///
/// `db.cmd.no_solution` when the given point is at the center.
pub(crate) fn polygon_about(
    center: Point2,
    point: Point2,
    sides: usize,
    mode: PolygonMode,
) -> Result<Vec<Point2>> {
    let offset = point - center;
    let distance = offset.norm();
    if distance <= linear() {
        return Err(Error::new(codes::NO_SOLUTION));
    }
    let count = sides as f64;
    let step = TAU / count;
    let (radius, start) = match mode {
        PolygonMode::Inscribed => (distance, angle_of(offset)),
        PolygonMode::Circumscribed => {
            (distance / (PI / count).cos(), angle_of(offset) + PI / count)
        }
    };
    Ok((0..sides)
        .map(|k| center + unit_at(start + step * k as f64) * radius)
        .collect())
}

/// The vertices of a regular polygon of `sides` sides with the edge from `first` to `second`,
/// counterclockwise from `first`.
///
/// # Errors
///
/// `db.cmd.points_collinear` when the points coincide.
pub(crate) fn polygon_edge(first: Point2, second: Point2, sides: usize) -> Result<Vec<Point2>> {
    distinct(first, second)?;
    let turn = TAU / sides as f64;
    let (center, radius) = chord_center(first, second, turn)?;
    let start = angle_of(first - center);
    Ok((0..sides)
        .map(|k| center + unit_at(start + turn * k as f64) * radius)
        .collect())
}

/// The ellipse centered at `center` with the semi-major axis `half` (a vector to a vertex) and
/// the semi-minor length `minor`; the longer of the two axes becomes the major one.
///
/// # Errors
///
/// `db.cmd.no_solution` when `half` is zero; the ellipse's own errors for a minor length that is
/// not positive.
pub(crate) fn ellipse_from_half_axis(
    center: Point2,
    half: Vector2,
    minor: f64,
) -> Result<Ellipse2> {
    let major = half.norm();
    if major <= linear() {
        return Err(Error::new(codes::NO_SOLUTION));
    }
    if minor <= major {
        Ellipse2::new(center, half, minor / major)
    } else {
        Ellipse2::new(center, perp(half) * (minor / major), major / minor)
    }
}

/// The ellipse whose axis runs from `first` to `second`, with the semi-minor length `minor`.
///
/// # Errors
///
/// As [`ellipse_from_half_axis`], and `db.cmd.points_collinear` when the points coincide.
pub(crate) fn ellipse_on_axis(first: Point2, second: Point2, minor: f64) -> Result<Ellipse2> {
    distinct(first, second)?;
    ellipse_from_half_axis(
        first + (second - first) * 0.5,
        (second - first) * 0.5,
        minor,
    )
}

/// The ellipse with its center and one axis end given and the other semi-axis `minor`.
///
/// # Errors
///
/// As [`ellipse_from_half_axis`].
pub(crate) fn ellipse_about(center: Point2, end: Point2, minor: f64) -> Result<Ellipse2> {
    ellipse_from_half_axis(center, end - center, minor)
}

/// The ellipse of a circle of radius `|end - center|` turned by `rotation` about its axis: the
/// minor semi-axis is the radius times the cosine of the rotation, in [0, π/2).
///
/// # Errors
///
/// `db.cmd.no_solution` when the rotation is outside [0, π/2).
pub(crate) fn ellipse_rotated(center: Point2, end: Point2, rotation: f64) -> Result<Ellipse2> {
    if !(0.0..PI / 2.0).contains(&rotation) {
        return Err(Error::new(codes::NO_SOLUTION));
    }
    ellipse_about(center, end, (end - center).norm() * rotation.cos())
}

/// The four corners of a rectangle with one corner at `first`, whose sides lie along the
/// direction `angle`, and whose opposite corner is `other`.
///
/// # Errors
///
/// `db.cmd.no_solution` when `other` lies on a line through `first` along one side.
pub(crate) fn rectangle_rotated(first: Point2, angle: f64, other: Point2) -> Result<[Point2; 4]> {
    let u = unit_at(angle);
    let v = perp(u);
    let d = other - first;
    let (length, width) = (d.dot(&u), d.dot(&v));
    if length.abs() <= linear() || width.abs() <= linear() {
        return Err(Error::new(codes::NO_SOLUTION));
    }
    let along = u * length;
    let across = v * width;
    Ok([first, first + along, first + along + across, first + across])
}

/// The four corners of an axis-aligned rectangle from `first` to the opposite corner `other`.
///
/// # Errors
///
/// As [`rectangle_rotated`].
pub(crate) fn rectangle_corners(first: Point2, other: Point2) -> Result<[Point2; 4]> {
    rectangle_rotated(first, 0.0, other)
}

/// The four corners of an axis-aligned rectangle of `length` along x and `width` along y from
/// `first`, toward the quadrant of `side`.
///
/// # Errors
///
/// As [`rectangle_corners`].
pub(crate) fn rectangle_sized(
    first: Point2,
    length: f64,
    width: f64,
    side: Point2,
) -> Result<[Point2; 4]> {
    let sign = |v: f64| if v < 0.0 { -1.0 } else { 1.0 };
    let other = Point2::new(
        first.x + sign(side.x - first.x) * length,
        first.y + sign(side.y - first.y) * width,
    );
    rectangle_corners(first, other)
}

/// A side of a tangent construction: an infinite line or a segment, or a circle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Tangent {
    /// A line through `origin` along the unit vector `unit`; with `span`, the segment from
    /// `origin` along `unit` for that length.
    Line {
        /// A point of the line.
        origin: Point2,
        /// The unit direction.
        unit: Vector2,
        /// The length of the segment, `None` for an infinite line.
        span: Option<f64>,
    },
    /// A circle.
    Circle {
        /// The center.
        center: Point2,
        /// The radius.
        radius: f64,
    },
}

/// The point where a circle of radius `radius` at `center` touches `object`, `None` when it
/// touches no point of it (a segment too short, or a center on the object's center).
fn touch_point(object: Tangent, center: Point2) -> Option<Point2> {
    match object {
        Tangent::Line { origin, unit, span } => {
            let q = (center - origin).dot(&unit);
            if span.is_some_and(|len| q < -linear() || q > len + linear()) {
                return None;
            }
            Some(origin + unit * q)
        }
        Tangent::Circle {
            center: c,
            radius: r,
        } => {
            let d = center - c;
            let n = d.norm();
            (n > linear()).then(|| c + d * (r / n))
        }
    }
}

/// The centers of circles of radius `radius` tangent to a line and a circle (or the reverse).
fn line_circle_centers(line: (Point2, Vector2), circle: (Point2, f64), radius: f64) -> Vec<Point2> {
    let (origin, unit) = line;
    let (center, rc) = circle;
    let normal = perp(unit);
    let mut out = Vec::new();
    for side in [1.0, -1.0] {
        let e = origin + normal * (side * radius) - center;
        let b = unit.dot(&e);
        for rho in [rc + radius, (rc - radius).abs()] {
            let disc = b * b - (e.norm_squared() - rho * rho);
            if disc < -linear() * rho.max(1.0) {
                continue;
            }
            let root = disc.max(0.0).sqrt();
            for t in [-b + root, -b - root] {
                out.push(origin + unit * t + normal * (side * radius));
            }
        }
    }
    out
}

/// The centers of circles of radius `radius` tangent to two circles.
fn circle_circle_centers(a: (Point2, f64), b: (Point2, f64), radius: f64) -> Vec<Point2> {
    let (ca, ra) = a;
    let (cb, rb) = b;
    let axis = cb - ca;
    let d = axis.norm();
    let mut out = Vec::new();
    if d <= linear() {
        return out;
    }
    let e = axis / d;
    for rho_a in [ra + radius, (ra - radius).abs()] {
        for rho_b in [rb + radius, (rb - radius).abs()] {
            let along = (rho_a * rho_a - rho_b * rho_b + d * d) / (2.0 * d);
            let h2 = rho_a * rho_a - along * along;
            if h2 < -linear() * rho_a.max(1.0) {
                continue;
            }
            let h = h2.max(0.0).sqrt();
            let base = ca + e * along;
            out.push(base + perp(e) * h);
            out.push(base - perp(e) * h);
        }
    }
    out
}

/// The centers of circles of radius `radius` tangent to two lines.
fn line_line_centers(a: (Point2, Vector2), b: (Point2, Vector2), radius: f64) -> Vec<Point2> {
    let mut out = Vec::new();
    let (oa, ua) = a;
    let (ob, ub) = b;
    let denom = cross(ua, ub);
    if denom.abs() <= 1e-12 {
        return out;
    }
    for sa in [1.0, -1.0] {
        for sb in [1.0, -1.0] {
            let pa = oa + perp(ua) * (sa * radius);
            let pb = ob + perp(ub) * (sb * radius);
            let t = cross(pb - pa, ub) / denom;
            out.push(pa + ua * t);
        }
    }
    out
}

/// The centers of every circle of `radius` tangent to both objects.
fn candidate_centers(a: Tangent, b: Tangent, radius: f64) -> Vec<Point2> {
    match (a, b) {
        (
            Tangent::Line {
                origin: oa,
                unit: ua,
                ..
            },
            Tangent::Line {
                origin: ob,
                unit: ub,
                ..
            },
        ) => line_line_centers((oa, ua), (ob, ub), radius),
        (Tangent::Line { origin, unit, .. }, Tangent::Circle { center, radius: r })
        | (Tangent::Circle { center, radius: r }, Tangent::Line { origin, unit, .. }) => {
            line_circle_centers((origin, unit), (center, r), radius)
        }
        (
            Tangent::Circle {
                center: ca,
                radius: ra,
            },
            Tangent::Circle {
                center: cb,
                radius: rb,
            },
        ) => circle_circle_centers((ca, ra), (cb, rb), radius),
    }
}

/// The circle of `radius` tangent to `a` and `b` whose tangent points are nearest to the picks
/// `pick_a` (on `a`) and `pick_b` (on `b`): the solution with the least sum of the distances from
/// each pick to its tangent point.
///
/// # Errors
///
/// `db.cmd.no_solution` when the radius is not positive and finite, or no circle of that radius
/// touches both objects at points of their extent.
pub(crate) fn tangent_circle(
    a: Tangent,
    b: Tangent,
    radius: f64,
    pick_a: Point2,
    pick_b: Point2,
) -> Result<Circle2> {
    if !(radius.is_finite() && radius > 0.0) {
        return Err(Error::new(codes::NO_SOLUTION));
    }
    let mut best: Option<(f64, Point2)> = None;
    for center in candidate_centers(a, b, radius) {
        let (Some(ta), Some(tb)) = (touch_point(a, center), touch_point(b, center)) else {
            continue;
        };
        let score = (pick_a - ta).norm() + (pick_b - tb).norm();
        if best.is_none_or(|(s, _)| score < s) {
            best = Some((score, center));
        }
    }
    let (_, center) = best.ok_or_else(|| Error::new(codes::NO_SOLUTION))?;
    Circle2::new(center, radius)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn close_point(a: Point2, b: Point2) -> bool {
        (a - b).norm() < 1e-9
    }

    #[test]
    fn circle_through_three_points_is_exact() {
        let circle = circle_through(p(1.0, 0.0), p(0.0, 1.0), p(-1.0, 0.0)).unwrap();
        assert!(close_point(circle.center(), p(0.0, 0.0)));
        assert!(close(circle.radius(), 1.0));
    }

    #[test]
    fn collinear_points_have_no_circle() {
        let error = circle_through(p(0.0, 0.0), p(1.0, 1.0), p(2.0, 2.0)).unwrap_err();
        assert_eq!(error.code(), codes::POINTS_COLLINEAR.code);
        assert!(circle_through(p(0.0, 0.0), p(0.0, 0.0), p(1.0, 0.0)).is_err());
    }

    #[test]
    fn arc_through_takes_the_side_that_contains_the_middle_point() {
        // From (1,0) through (0,1) to (-1,0): the upper half, counterclockwise from angle 0.
        let arc = arc_through(p(1.0, 0.0), p(0.0, 1.0), p(-1.0, 0.0)).unwrap();
        assert!(close(arc.sweep(), PI));
        assert!(close(arc.start_angle(), 0.0));
        // The same end points with the middle point below: clockwise from angle 0.
        let arc = arc_through(p(1.0, 0.0), p(0.0, -1.0), p(-1.0, 0.0)).unwrap();
        assert!(close(arc.sweep(), -PI));
    }

    #[test]
    fn arc_center_end_sweeps_counterclockwise() {
        let arc = arc_center_end(p(1.0, 0.0), p(0.0, 0.0), p(0.0, -1.0)).unwrap();
        assert!(close(arc.radius(), 1.0));
        assert!(close(arc.sweep(), 1.5 * PI));
        assert!(arc_center_end(p(1.0, 0.0), p(0.0, 0.0), p(2.0, 0.0)).is_err());
    }

    #[test]
    fn arc_from_start_end_and_angle_has_the_chord_on_the_expected_side() {
        // A quarter turn counterclockwise from (1,0) to (0,1): center at the origin.
        let arc = arc_start_end_angle(p(1.0, 0.0), p(0.0, 1.0), PI / 2.0).unwrap();
        assert!(close_point(arc.center(), p(0.0, 0.0)));
        assert!(close(arc.radius(), 1.0));
        // A three-quarter turn clockwise from the same start reaches the same end point, and
        // its center is the same.
        let arc = arc_start_end_angle(p(1.0, 0.0), p(0.0, 1.0), -1.5 * PI).unwrap();
        assert!(close_point(arc.center(), p(0.0, 0.0)));
        assert!(close(arc.sweep(), -1.5 * PI));
    }

    #[test]
    fn arc_from_start_end_and_radius_is_the_minor_arc_with_the_sign_as_the_side() {
        let arc = arc_start_end_radius(p(1.0, 0.0), p(0.0, 1.0), 1.0).unwrap();
        assert!(close_point(arc.center(), p(0.0, 0.0)));
        assert!(close(arc.sweep(), PI / 2.0));
        let arc = arc_start_end_radius(p(1.0, 0.0), p(0.0, 1.0), -1.0).unwrap();
        assert!(close(arc.sweep(), -PI / 2.0));
        assert!(close(arc.radius(), 1.0));
        // A chord longer than the diameter has no solution.
        assert!(arc_start_end_radius(p(0.0, 0.0), p(3.0, 0.0), 1.0).is_err());
    }

    #[test]
    fn bulges_match_the_sweep_of_their_arcs() {
        assert!(close(
            bulge_through(p(1.0, 0.0), p(0.0, 1.0), p(-1.0, 0.0)).unwrap(),
            1.0
        ));
        assert!(close(
            bulge_center(p(1.0, 0.0), p(0.0, 0.0), p(0.0, 1.0)).unwrap(),
            (PI / 8.0).tan()
        ));
        assert!(close(
            bulge_angle(p(0.0, 0.0), p(1.0, 0.0), PI).unwrap(),
            1.0
        ));
        assert!(bulge_angle(p(0.0, 0.0), p(1.0, 0.0), TAU).is_err());
        assert!(close(
            bulge_radius(p(1.0, 0.0), p(0.0, 1.0), -1.0).unwrap(),
            -(PI / 8.0).tan()
        ));
    }

    #[test]
    fn bulge_direction_follows_the_tangent() {
        // Leaving the origin along +x and ending at (0, 2): a half circle centered at (0, 1).
        let bulge = bulge_direction(p(0.0, 0.0), Vector2::new(1.0, 0.0), p(0.0, 2.0)).unwrap();
        assert!(close(bulge, 1.0));
        // Ahead on the tangent line is a straight segment.
        assert!(close(
            bulge_direction(p(0.0, 0.0), Vector2::new(1.0, 0.0), p(3.0, 0.0)).unwrap(),
            0.0
        ));
        // Behind on the tangent line has no solution.
        assert!(bulge_direction(p(0.0, 0.0), Vector2::new(1.0, 0.0), p(-3.0, 0.0)).is_err());
    }

    #[test]
    fn end_tangent_of_a_half_circle_points_back() {
        // A counterclockwise half circle from (0,0) to (2,0) passes below the chord and ends
        // travelling in +y.
        let t = end_tangent(p(0.0, 0.0), 1.0, p(2.0, 0.0));
        assert!(close(t.x, 0.0) && close(t.y, 1.0));
        let t = end_tangent(p(0.0, 0.0), 0.0, p(0.0, 5.0));
        assert!(close(t.x, 0.0) && close(t.y, 1.0));
    }

    #[test]
    fn inscribed_and_circumscribed_squares() {
        let inscribed = polygon_about(p(0.0, 0.0), p(1.0, 0.0), 4, PolygonMode::Inscribed).unwrap();
        assert!(close_point(inscribed[0], p(1.0, 0.0)));
        assert!(close_point(inscribed[1], p(0.0, 1.0)));
        assert!(close_point(inscribed[2], p(-1.0, 0.0)));
        let circumscribed =
            polygon_about(p(0.0, 0.0), p(1.0, 0.0), 4, PolygonMode::Circumscribed).unwrap();
        // The edge middle at (1,0): vertices at (1,1) and (1,-1) and the circumradius sqrt(2).
        assert!(close_point(circumscribed[0], p(1.0, 1.0)));
        assert!(close_point(circumscribed[3], p(1.0, -1.0)));
    }

    #[test]
    fn polygon_on_an_edge_starts_at_the_first_point_counterclockwise() {
        let square = polygon_edge(p(0.0, 0.0), p(1.0, 0.0), 4).unwrap();
        assert!(close_point(square[0], p(0.0, 0.0)));
        assert!(close_point(square[1], p(1.0, 0.0)));
        assert!(close_point(square[2], p(1.0, 1.0)));
        assert!(close_point(square[3], p(0.0, 1.0)));
    }

    #[test]
    fn ellipse_on_axis_swaps_the_axes_when_the_minor_is_longer() {
        let ellipse = ellipse_on_axis(p(-2.0, 0.0), p(2.0, 0.0), 1.0).unwrap();
        assert!(close(ellipse.ratio(), 0.5));
        assert!(close_point(ellipse.center(), p(0.0, 0.0)));
        let swapped = ellipse_on_axis(p(-1.0, 0.0), p(1.0, 0.0), 2.0).unwrap();
        assert!(close(swapped.ratio(), 0.5));
        assert!(close(swapped.major_axis().y.abs(), 2.0));
    }

    #[test]
    fn rotated_ellipse_has_the_cosine_ratio() {
        let ellipse = ellipse_rotated(p(0.0, 0.0), p(2.0, 0.0), PI / 3.0).unwrap();
        assert!(close(ellipse.ratio(), 0.5));
        assert!(ellipse_rotated(p(0.0, 0.0), p(2.0, 0.0), PI / 2.0).is_err());
    }

    #[test]
    fn rectangles_from_corners_rotation_and_size() {
        let corners = rectangle_corners(p(0.0, 0.0), p(4.0, 2.0)).unwrap();
        assert!(close_point(corners[2], p(4.0, 2.0)));
        assert!(close_point(corners[1], p(4.0, 0.0)));
        let sized = rectangle_sized(p(0.0, 0.0), 4.0, 2.0, p(-1.0, -1.0)).unwrap();
        assert!(close_point(sized[2], p(-4.0, -2.0)));
        let turned = rectangle_rotated(p(0.0, 0.0), PI / 2.0, p(1.0, 2.0)).unwrap();
        // Along +y by 2 and along +x by 1 from the first corner: the other corner is (1, 2).
        assert!(close_point(turned[1], p(0.0, 2.0)));
        assert!(close_point(turned[2], p(1.0, 2.0)));
        assert!(rectangle_corners(p(0.0, 0.0), p(4.0, 0.0)).is_err());
    }

    #[test]
    fn circle_tangent_to_two_lines_is_the_nearest_corner_solution() {
        let x_axis = Tangent::Line {
            origin: p(0.0, 0.0),
            unit: Vector2::new(1.0, 0.0),
            span: None,
        };
        let y_axis = Tangent::Line {
            origin: p(0.0, 0.0),
            unit: Vector2::new(0.0, 1.0),
            span: None,
        };
        let circle = tangent_circle(x_axis, y_axis, 1.0, p(2.0, 0.1), p(0.1, 2.0)).unwrap();
        assert!(close_point(circle.center(), p(1.0, 1.0)));
        let circle = tangent_circle(x_axis, y_axis, 1.0, p(2.0, -0.1), p(0.1, -2.0)).unwrap();
        assert!(close_point(circle.center(), p(1.0, -1.0)));
    }

    #[test]
    fn circle_tangent_to_a_line_and_a_circle() {
        // The line y = 0 and the circle of radius 1 at (0, 3), radius 1: the center is at height
        // 1 and at distance 2 from (0, 3), so it is (0, 1), the only solution.
        let line = Tangent::Line {
            origin: p(0.0, 0.0),
            unit: Vector2::new(1.0, 0.0),
            span: None,
        };
        let circle = Tangent::Circle {
            center: p(0.0, 3.0),
            radius: 1.0,
        };
        let found = tangent_circle(line, circle, 1.0, p(3.0, 1.0), p(0.0, 2.0)).unwrap();
        assert!(close_point(found.center(), p(0.0, 1.0)));
    }

    #[test]
    fn circle_tangent_to_two_circles_solves_the_two_offset_circles() {
        let a = Tangent::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        };
        let b = Tangent::Circle {
            center: p(3.0, 0.0),
            radius: 1.0,
        };
        let found = tangent_circle(a, b, 1.0, p(1.5, 2.0), p(1.5, 2.0)).unwrap();
        assert!(close_point(found.center(), p(1.5, 1.75f64.sqrt())));
        assert!(close(found.radius(), 1.0));
    }

    #[test]
    fn tangent_point_outside_a_segment_is_not_a_solution() {
        let short = Tangent::Line {
            origin: p(0.0, 0.0),
            unit: Vector2::new(1.0, 0.0),
            span: Some(0.5),
        };
        let y_axis = Tangent::Line {
            origin: p(0.0, 0.0),
            unit: Vector2::new(0.0, 1.0),
            span: None,
        };
        // Both tangents of radius 1 touch the segment at x = 1, beyond its length of 0.5.
        assert!(tangent_circle(short, y_axis, 1.0, p(1.0, 1.0), p(0.0, 1.0)).is_err());
    }
}

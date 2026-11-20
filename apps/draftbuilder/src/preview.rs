//! The shape a drawing command is taking while the pointer moves: the outline of the rectangle,
//! circle or arc that the points taken so far and the cursor define. Pure geometry in drawing
//! units; the interaction adapter turns it into an overlay.

use std::f64::consts::TAU;

/// The points sampled along a full circle or an arc.
const SAMPLES: u32 = 64;

/// The outline of what `command` makes from the `taken` points and the `cursor`, when it is a
/// shape worth previewing. `radius_next` says the prompt waits for a radius.
pub fn outline(
    command: &str,
    taken: &[[f64; 2]],
    cursor: [f64; 2],
    radius_next: bool,
) -> Option<Vec<[f64; 2]>> {
    match (command, taken) {
        ("draftbuilder.draw.rectangle", [a]) => {
            Some(vec![*a, [cursor[0], a[1]], cursor, [a[0], cursor[1]], *a])
        }
        ("draftbuilder.draw.circle", [center]) if radius_next => {
            let r = (cursor[0] - center[0]).hypot(cursor[1] - center[1]);
            (r > 0.0).then(|| arc_points(*center, r, 0.0, TAU))
        }
        ("draftbuilder.draw.arc", [a, b]) => Some(three_point_arc(*a, *b, cursor)),
        _ => None,
    }
}

fn arc_points(center: [f64; 2], radius: f64, from: f64, sweep: f64) -> Vec<[f64; 2]> {
    (0..=SAMPLES)
        .map(|i| {
            let t = from + sweep * f64::from(i) / f64::from(SAMPLES);
            [center[0] + radius * t.cos(), center[1] + radius * t.sin()]
        })
        .collect()
}

/// The arc from `a` through `b` to `c`; the straight path when the three points are collinear.
fn three_point_arc(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Vec<[f64; 2]> {
    let d = 2.0 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
    if d.abs() < 1e-12 {
        return vec![a, b, c];
    }
    let (sa, sb, sc) = (
        a[0] * a[0] + a[1] * a[1],
        b[0] * b[0] + b[1] * b[1],
        c[0] * c[0] + c[1] * c[1],
    );
    let center = [
        (sa * (b[1] - c[1]) + sb * (c[1] - a[1]) + sc * (a[1] - b[1])) / d,
        (sa * (c[0] - b[0]) + sb * (a[0] - c[0]) + sc * (b[0] - a[0])) / d,
    ];
    let radius = (a[0] - center[0]).hypot(a[1] - center[1]);
    let angle = |p: [f64; 2]| (p[1] - center[1]).atan2(p[0] - center[0]);
    let (start, mid, end) = (angle(a), angle(b), angle(c));
    let ccw_to_end = (end - start).rem_euclid(TAU);
    let ccw_to_mid = (mid - start).rem_euclid(TAU);
    // The arc runs the way that passes through `b`.
    let sweep = if ccw_to_mid <= ccw_to_end {
        ccw_to_end
    } else {
        ccw_to_end - TAU
    };
    arc_points(center, radius, start, sweep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_arc_through_three_points_passes_through_the_middle_one() {
        let path = outline(
            "draftbuilder.draw.arc",
            &[[1.0, 0.0], [0.0, 1.0]],
            [-1.0, 0.0],
            false,
        );
        let path = path.unwrap_or_default();
        assert!(
            path.iter()
                .any(|p| (p[0]).abs() < 1e-6 && (p[1] - 1.0).abs() < 1e-6)
        );
        let last = path.last().copied().unwrap_or_default();
        assert!((last[0] + 1.0).abs() < 1e-9 && last[1].abs() < 1e-9);
    }
}

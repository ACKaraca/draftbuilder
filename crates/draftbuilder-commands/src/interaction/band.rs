//! The selection window of a pointer drag (DB-SEL-002, DB-SEL-010): two clicks make a window when
//! the second lies to the right of the first, else a crossing.

use arc_base::math::Point2;
use arc_base::{Error, ObjectId, Result};
use arc_doc::Snapshot;

use super::BandMode;
use crate::draw::create::from_meters;
use crate::error::codes;
use crate::select::candidates::Options;
use crate::select::methods;

/// The mode of the window that starts at `start` and ends at `end` (meters): a window when the
/// second corner lies to the right of the first, else a crossing.
pub(super) fn mode(start: [f64; 2], end: [f64; 2]) -> BandMode {
    if end[0] >= start[0] {
        BandMode::Window
    } else {
        BandMode::Crossing
    }
}

/// The objects of the model space that the window or crossing from `start` to `end` (meters)
/// selects, in draw order.
///
/// # Errors
///
/// `db.cmd.no_model_space` for a document without one; `db.cmd.selection_invalid` for a rectangle
/// with no width or height.
pub(super) fn select(snapshot: &Snapshot, start: [f64; 2], end: [f64; 2]) -> Result<Vec<ObjectId>> {
    let space = crate::model_space(snapshot).ok_or_else(|| Error::new(codes::NO_MODEL_SPACE))?;
    let corner =
        |p: [f64; 2]| Point2::new(from_meters(snapshot, p[0]), from_meters(snapshot, p[1]));
    let corners = [corner(start), corner(end)];
    let options = Options::default();
    let pick = match mode(start, end) {
        BandMode::Crossing => methods::select_crossing(snapshot, space, corners, &options)?,
        _ => methods::select_window(snapshot, space, corners, &options)?,
    };
    Ok(pick.ids)
}

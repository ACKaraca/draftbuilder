//! The gestures of a selection prompt (DB-SEL-003, 004, 005, 007, 010): the keywords the prompt
//! takes, the window, crossing, polygon or fence that the next gestures define, and the objects
//! gathered in multiple mode.
//!
//! A selection prompt answers at once with the objects of its first gesture (single mode, the
//! default). The keywords `multiple`, `add` and `remove` switch to multiple mode: gestures then
//! collect objects, `add` adds them and `remove` takes them out, `undo` takes back the last
//! gesture, `single` returns to the default, and an empty line answers the prompt with the set.

use arc_base::math::Point2;
use arc_base::{Error, ObjectId, Result};
use arc_doc::Snapshot;

use super::{BandMode, OptionOut};
use crate::draw::create::from_meters;
use crate::error::codes;
use crate::select::candidates::Options;
use crate::select::{MAX_SET, Mode, combine, methods};
use crate::vocabulary::SELECTION_OPTIONS;

/// The gestures multiple mode keeps for `undo`; older ones are dropped.
const UNDO_STEPS: usize = 32;

/// The region a polygon or a fence selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Region {
    /// The objects inside the polygon.
    Window,
    /// The objects inside or touching the polygon.
    Crossing,
    /// The objects the open polyline crosses.
    Fence,
}

/// The shape the next gesture takes, set by a keyword.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Shape {
    /// The default: an object pick, or a window or crossing by the drag direction.
    Default,
    /// A window or a crossing whatever the drag direction: two corners.
    Box(BandMode),
    /// A polygon or a fence, its vertices in meters, closed by an empty line.
    Polygon {
        region: Region,
        points: Vec<[f64; 2]>,
    },
}

/// The objects a selection prompt gathers, and the shape of its next gesture.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Gather {
    /// Multiple mode: gestures collect objects until an empty line answers the prompt.
    pub multiple: bool,
    /// How a gesture combines with the gathered objects in multiple mode.
    pub mode: Mode,
    /// The gathered objects, in order.
    pub ids: Vec<ObjectId>,
    /// The gathered objects before each gesture of multiple mode, for `undo`.
    steps: Vec<Vec<ObjectId>>,
    /// The shape of the next gesture.
    pub shape: Shape,
}

impl Default for Gather {
    fn default() -> Self {
        Self {
            multiple: false,
            mode: Mode::Add,
            ids: Vec::new(),
            steps: Vec::new(),
            shape: Shape::Default,
        }
    }
}

impl Gather {
    /// Whether the prompt holds a gesture or a collection that an Escape clears.
    pub fn pending(&self) -> bool {
        self.multiple || self.shape != Shape::Default
    }

    /// Multiple mode: `ids` combine with the gathered objects by `mode`, one step at a time.
    ///
    /// # Errors
    ///
    /// `db.cmd.limit_exceeded` when the gathered set would pass [`MAX_SET`]; nothing changes.
    pub fn apply(&mut self, ids: &[ObjectId], mode: Mode) -> Result<()> {
        let next = combine(&self.ids, ids, mode);
        if next.len() > MAX_SET {
            return Err(Error::new(codes::LIMIT_EXCEEDED).with_detail("limit", MAX_SET.to_string()));
        }
        // A gesture that changes nothing is no step, so undo never takes back nothing.
        if next == self.ids {
            return Ok(());
        }
        self.steps.push(std::mem::replace(&mut self.ids, next));
        if self.steps.len() > UNDO_STEPS {
            self.steps.remove(0);
        }
        Ok(())
    }

    /// Whether a gesture of multiple mode can be taken back.
    pub fn has_steps(&self) -> bool {
        !self.steps.is_empty()
    }

    /// Takes back the last gesture of multiple mode; nothing when there is none.
    pub fn undo(&mut self) {
        if let Some(before) = self.steps.pop() {
            self.ids = before;
        }
    }

    /// Multiple mode on or off, with the mode of the gestures that follow.
    pub fn set_multiple(&mut self, multiple: bool, mode: Option<Mode>) {
        self.multiple = multiple;
        if let Some(mode) = mode {
            self.mode = mode;
        }
    }

    /// Back to single mode: the gathered objects are dropped.
    pub fn single(&mut self) {
        *self = Self {
            mode: self.mode,
            ..Self::default()
        };
    }
}

/// The keyword of a selection prompt that `text` names, in its identifier form. Case is ignored.
pub(super) fn keyword(text: &str) -> Option<&'static str> {
    let text = text.trim();
    SELECTION_OPTIONS
        .iter()
        .map(|option| option.id)
        .find(|id| id.eq_ignore_ascii_case(text))
}

/// The keyword options a selection prompt offers, in their order.
pub(super) fn options() -> Vec<OptionOut> {
    SELECTION_OPTIONS
        .iter()
        .map(|option| OptionOut {
            id: option.id.to_owned(),
            label: option.message_id.to_owned(),
            shortcut: None,
        })
        .collect()
}

/// The objects of the model space that a polygon or a fence selects. The vertices are in meters.
///
/// # Errors
///
/// `db.cmd.no_model_space` for a document without one, and the errors of the selection method for
/// too few or too many vertices.
pub(super) fn region_ids(
    snapshot: &Snapshot,
    region: Region,
    vertices: &[[f64; 2]],
) -> Result<Vec<ObjectId>> {
    let space = crate::model_space(snapshot).ok_or_else(|| Error::new(codes::NO_MODEL_SPACE))?;
    let points: Vec<Point2> = vertices
        .iter()
        .map(|p| Point2::new(from_meters(snapshot, p[0]), from_meters(snapshot, p[1])))
        .collect();
    let options = Options::default();
    let pick = match region {
        Region::Window => methods::select_window_polygon(snapshot, space, &points, &options)?,
        Region::Crossing => methods::select_crossing_polygon(snapshot, space, &points, &options)?,
        Region::Fence => methods::select_fence(snapshot, space, &points, &options)?,
    };
    Ok(pick.ids)
}

#[cfg(test)]
mod tests {
    use arc_base::id::Identifier;

    use super::*;

    fn id(n: usize) -> ObjectId {
        ObjectId::from_u128(u128::try_from(n).unwrap_or(0))
    }

    #[test]
    fn a_gathered_set_past_the_selection_limit_is_refused_and_nothing_changes() {
        let mut gather = Gather {
            multiple: true,
            ..Gather::default()
        };
        let all: Vec<ObjectId> = (1..=MAX_SET).map(id).collect();
        assert!(gather.apply(&all, Mode::Add).is_ok());
        assert!(gather.apply(&[id(MAX_SET + 1)], Mode::Add).is_err());
        assert_eq!(gather.ids.len(), MAX_SET);
        gather.undo();
        assert!(gather.ids.is_empty());
    }

    #[test]
    fn a_gesture_that_changes_nothing_is_no_undo_step() {
        let mut gather = Gather {
            multiple: true,
            ..Gather::default()
        };
        assert!(gather.apply(&[id(1)], Mode::Add).is_ok());
        assert!(gather.apply(&[id(1)], Mode::Add).is_ok());
        gather.undo();
        assert!(gather.ids.is_empty(), "the second gesture took no step");
        assert!(!gather.has_steps());
    }

    #[test]
    fn undo_keeps_only_the_last_gestures() {
        let mut gather = Gather {
            multiple: true,
            ..Gather::default()
        };
        for n in 1..=UNDO_STEPS + 8 {
            assert!(gather.apply(&[id(n)], Mode::Add).is_ok());
        }
        for _ in 0..UNDO_STEPS {
            gather.undo();
        }
        assert_eq!(
            gather.ids.len(),
            8,
            "the gestures older than the kept ones stay"
        );
        gather.undo();
        assert_eq!(gather.ids.len(), 8);
    }
}

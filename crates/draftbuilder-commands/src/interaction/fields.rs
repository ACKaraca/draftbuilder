//! The dynamic input fields beside the cursor (DB-CMD-026): two fields, a distance and an angle
//! from the last point, or X and Y when no point was given yet. A typed field locks; the other
//! field follows the cursor. Enter or a click then composes the point from the locked values.

use arc_base::{Error, Result};
use arc_doc::Snapshot;

use super::{DynamicMode, DynamicOut, FieldOut};
use crate::draw::create::{from_meters, meters_per_unit};
use crate::error::codes;

/// The two fields and the focused one.
#[derive(Debug, Default)]
pub(super) struct Fields {
    texts: [String; 2],
    focus: usize,
}

/// The distance and angle, or X and Y, of a point relative to the base (or the origin) in drawing
/// units, the angle in degrees.
fn parts(base: Option<[f64; 2]>, cursor: [f64; 2]) -> (DynamicMode, [f64; 2]) {
    match base {
        Some(b) => {
            let (dx, dy) = (cursor[0] - b[0], cursor[1] - b[1]);
            (
                DynamicMode::Relative,
                [dx.hypot(dy), dy.atan2(dx).to_degrees()],
            )
        }
        None => (DynamicMode::Absolute, cursor),
    }
}

impl Fields {
    /// Whether a field is locked by typing.
    pub(super) fn any_locked(&self) -> bool {
        self.texts.iter().any(|t| !t.trim().is_empty())
    }

    /// Clears both fields (the focus stays).
    pub(super) fn clear(&mut self) {
        self.texts = [String::new(), String::new()];
    }

    /// Sets field `index` to `text` and focuses it; an index other than 0 or 1 is ignored.
    pub(super) fn set(&mut self, index: usize, text: String) {
        if let Some(slot) = self.texts.get_mut(index) {
            *slot = text;
            self.focus = index;
        }
    }

    /// Moves the focus to the other field.
    pub(super) fn toggle(&mut self) {
        self.focus = 1 - self.focus;
    }

    /// The value of field `index`: the typed number when the field is locked, else `fallback`.
    fn value(&self, index: usize, fallback: f64, field: &'static str) -> Result<f64> {
        let text = self.texts[index].trim();
        if text.is_empty() {
            return Ok(fallback);
        }
        text.parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(|| Error::new(codes::VALUE_INVALID).with_detail("field", field))
    }

    /// The point the locked fields give, in meters: the locked distance and angle from `base`, or
    /// the locked X and Y; an unlocked field takes the value at `cursor` (meters).
    ///
    /// # Errors
    ///
    /// `db.cmd.value_invalid` (detail `field`) for a locked text that is not a finite number.
    pub(super) fn compose(
        &self,
        snapshot: &Snapshot,
        base: Option<[f64; 2]>,
        cursor: [f64; 2],
    ) -> Result<[f64; 2]> {
        let mpu = meters_per_unit(snapshot);
        let cursor = [
            from_meters(snapshot, cursor[0]),
            from_meters(snapshot, cursor[1]),
        ];
        let base = base.map(|b| [from_meters(snapshot, b[0]), from_meters(snapshot, b[1])]);
        let (mode, here) = parts(base, cursor);
        let (x, y) = match (mode, base) {
            (DynamicMode::Relative, Some(b)) => {
                let distance = self.value(0, here[0], "distance")?;
                let angle = self.value(1, here[1], "angle")?.to_radians();
                (b[0] + distance * angle.cos(), b[1] + distance * angle.sin())
            }
            _ => (self.value(0, here[0], "x")?, self.value(1, here[1], "y")?),
        };
        Ok([x * mpu, y * mpu])
    }

    /// The fields as they are shown: the locked values, else the values at `cursor` (meters).
    pub(super) fn output(
        &self,
        snapshot: &Snapshot,
        base: Option<[f64; 2]>,
        cursor: [f64; 2],
    ) -> DynamicOut {
        let cursor = [
            from_meters(snapshot, cursor[0]),
            from_meters(snapshot, cursor[1]),
        ];
        let base = base.map(|b| [from_meters(snapshot, b[0]), from_meters(snapshot, b[1])]);
        let (mode, here) = parts(base, cursor);
        let field = |index: usize| FieldOut {
            value: self.texts[index]
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .unwrap_or(here[index]),
            locked: !self.texts[index].trim().is_empty(),
            focused: self.focus == index,
        };
        DynamicOut {
            mode,
            fields: [field(0), field(1)],
        }
    }
}

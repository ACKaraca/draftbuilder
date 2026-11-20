//! Remembered defaults (DB-CMD-004): a command stores the values it ran with as session settings,
//! and the prompt that asks the same parameter next offers the value as its default.
//!
//! The foundation's `PromptStep::with_remembered_default` is reachable only from a `PromptSequence`,
//! and these commands build their prompts by hand, so [`offered`] applies the same rule: the value
//! is offered when one of the prompt's accepted kinds admits it. A remembered value is in SI units
//! (meters, radians or a plain number), as a typed answer is. Keys are in the product's namespace
//! and live only in the session, so they are never saved with a drawing.

use arc_base::{Quantity, Result};
use arc_command::{Accept, Context, Prompt, PromptInput, PromptState};

use crate::draw::create::meters_per_unit;

/// The remembered offset distance (meters).
pub const OFFSET_DISTANCE: &str = "draftbuilder.remembered.offset_distance";
/// The remembered fillet radius (meters).
pub const FILLET_RADIUS: &str = "draftbuilder.remembered.fillet_radius";
/// The remembered first chamfer distance (meters).
pub const CHAMFER_FIRST: &str = "draftbuilder.remembered.chamfer_first";
/// The remembered second chamfer distance (meters).
pub const CHAMFER_SECOND: &str = "draftbuilder.remembered.chamfer_second";
/// The remembered polygon side count.
pub const POLYGON_SIDES: &str = "draftbuilder.remembered.polygon_sides";
/// The remembered circle radius (meters).
pub const CIRCLE_RADIUS: &str = "draftbuilder.remembered.circle_radius";
/// The remembered rotation angle (radians).
pub const ROTATE_ANGLE: &str = "draftbuilder.remembered.rotate_angle";
/// The remembered scale factor.
pub const SCALE_FACTOR: &str = "draftbuilder.remembered.scale_factor";

/// Whether `value` lies within `min` and `max`, each bound taken when given.
fn within(value: f64, min: Option<f64>, max: Option<f64>) -> bool {
    min.is_none_or(|m| value >= m) && max.is_none_or(|m| value <= m)
}

/// The prompt `prompt` with the remembered value of `key` as its default, when the session holds a
/// value that one of its accepted kinds admits (within the kind's bounds, whole for an integer
/// number). The value is read each time the prompt is asked, so a "back" can change it. Otherwise
/// the prompt keeps its own default.
#[must_use]
pub(crate) fn offered(state: &PromptState<'_>, key: &str, prompt: Prompt) -> Prompt {
    let Some(value) = state
        .setting(key)
        .and_then(|v| v.as_f64())
        .filter(|v| v.is_finite())
    else {
        return prompt;
    };
    let input = prompt.accepts.iter().find_map(|accept| match accept {
        Accept::Distance { min, max } => {
            within(value, *min, *max).then_some(PromptInput::Distance(value))
        }
        Accept::Angle { min, max } => {
            within(value, *min, *max).then_some(PromptInput::Angle(value))
        }
        Accept::Number {
            dimension,
            min,
            max,
            integer,
        } => (within(value, *min, *max) && (!integer || value.fract() == 0.0))
            .then(|| PromptInput::Number(Quantity::new(value, *dimension))),
        _ => None,
    });
    match input {
        Some(input) => prompt.with_default(input),
        None => prompt,
    }
}

/// Stores `value` as the session setting `key`. A store the session refuses (its settings are full,
/// or the value is too large) is ignored: a remembered value never fails a command.
pub(crate) fn store(ctx: &mut Context<'_>, key: &str, value: f64) {
    if value.is_finite() {
        let _ = ctx.set_setting(key, serde_json::json!(value));
    }
}

/// The run of a command that also remembers the values it ran with. `values` gives the (key, value)
/// pairs from the parameters, with lengths in meters (`meters_per_unit` converts the drawing's
/// units); they are stored only when `run` succeeds, so a failed command stores nothing.
pub(crate) fn remembering<P, R>(
    run: impl Fn(&mut Context<'_>, P) -> Result<R> + Send + Sync + 'static,
    values: impl Fn(&P, f64) -> Vec<(&'static str, f64)> + Send + Sync + 'static,
) -> impl Fn(&mut Context<'_>, P) -> Result<R> + Send + Sync + 'static {
    move |ctx, p| {
        let per_unit = meters_per_unit(ctx.snapshot());
        let remembered = values(&p, per_unit);
        let out = run(ctx, p)?;
        for (key, value) in remembered {
            store(ctx, key, value);
        }
        Ok(out)
    }
}

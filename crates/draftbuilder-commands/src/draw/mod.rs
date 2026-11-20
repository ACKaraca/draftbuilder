//! Drawing commands (area `draw`): the 2D entities of a drawing, each as a product command
//! `draftbuilder.draw.<name>`.
//!
//! Each command has a typed form (its parameters, used by the protocol, scripts and the CLI) and a
//! prompt machine for the interactive form. The prompt machine ends in the same invocation as the
//! typed form, so one command is one undo step. Families: [`line`] (LINE), [`construction`]
//! (XLINE, RAY), [`polyline`] (PLINE), [`rectangle`] (RECTANG), [`polygon`] (POLYGON), [`circle`]
//! (CIRCLE), [`arc`] (ARC), [`ellipse`] (ELLIPSE), [`donut`] (DONUT), [`spline`] (SPLINE) and
//! [`point`] (POINT). Shared support is in [`create`]; the pure constructions are in [`geometry`].
//!
//! The rows of the capability document, with what each command supports, are in
//! `docs/architecture/overview.md`.

mod arc;
mod circle;
mod construction;
mod create;
mod donut;
mod ellipse;
mod geometry;
mod line;
mod point;
mod polygon;
mod polyline;
mod rectangle;
mod spline;

use arc_base::Result;
use arc_command::{CommandSpec, Context, Permission, Registry, UndoBehavior};
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::Message;
use crate::vocabulary::{Area, register_names};

/// A command of the area: undoable, with the `edit` permission class.
pub(crate) fn spec<P, R>(
    id: &'static str,
    label: Message,
    description: Message,
    run: impl Fn(&mut Context<'_>, P) -> Result<R> + Send + Sync + 'static,
) -> CommandSpec
where
    P: DeserializeOwned + JsonSchema,
    R: Serialize + JsonSchema,
{
    CommandSpec::new(id, label.message_id, description.message_id, run)
        .with_undo(UndoBehavior::Undoable)
        .with_permissions([Permission::Edit])
}

/// Registers the commands of the area and applies its names.
///
/// # Errors
///
/// `cmd.duplicate` when an identifier is already registered, and the errors of
/// [`register_names`].
pub fn register(registry: &mut Registry) -> Result<()> {
    let specs = line::specs()
        .into_iter()
        .chain(construction::specs())
        .chain(polyline::specs())
        .chain(rectangle::specs())
        .chain(polygon::specs())
        .chain(circle::specs())
        .chain(arc::specs())
        .chain(ellipse::specs())
        .chain(donut::specs())
        .chain(spline::specs())
        .chain(point::specs());
    for spec in specs {
        registry.register(spec)?;
    }
    register_names(registry, Area::Draw)
}

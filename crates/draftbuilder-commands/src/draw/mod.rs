//! Drawing commands (area `draw`). None are registered yet.

use arc_base::Result;
use arc_command::Registry;

use crate::vocabulary::{Area, register_names};

/// Registers the commands of the area and applies its names.
///
/// # Errors
/// The errors of [`register_names`].
pub fn register(registry: &mut Registry) -> Result<()> {
    register_names(registry, Area::Draw)
}

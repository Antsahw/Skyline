//! Hooks of a server's client without the `java-mods` feature (bodies: `java_entities.rs`,
//! `java_device.rs`): mods' fluids and item models from this game's own JVM, the client half of
//! mods the server lacks. There is no JVM here.
use super::super::Game;

impl Game<'_> {
    pub(in crate::game) fn java_mod_fluids(&mut self) {}
    pub(in crate::game) fn java_mod_item_models(&mut self) {}
    pub(in crate::game) fn tick_device_client(&mut self) {}
    /// The connection to a server ends: this game's own mods' client forgets it.
    pub(in crate::game) fn java_device_left() {}
}

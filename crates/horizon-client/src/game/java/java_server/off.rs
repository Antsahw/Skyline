//! Without the `java-mods` feature: the Java mods' server state holds nothing.

/// The players the Java mods saw and what players did (`ModsState::java`): none.
#[derive(Clone, Default)]
pub(in crate::game) struct JavaServerState;



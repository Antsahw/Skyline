//! Without the `java-mods` feature: the Java mods' server state holds nothing.

/// The players the Java mods saw and what players did (`ModsState::java`): none.
#[derive(Default)]
pub(in crate::game) struct JavaServerState;

impl JavaServerState {
    /// The connection a Java mod's level event leaves out: none.
    pub(in crate::game) fn event_except(&self) -> Option<u32> {
        None
    }
}

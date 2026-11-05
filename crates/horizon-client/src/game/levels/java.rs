//! Java mods' chunk loaders in parked levels.
use super::*;

impl Parked {
    /// The chunks Java mods' chunk loaders keep in this level.
    #[cfg_attr(not(feature = "java-mods"), allow(dead_code))]
    pub(in crate::game) fn set_ticketed(&mut self, chunks: Vec<(i32, i32)>) {
        self.streaming.set_ticketed(chunks);
    }
}

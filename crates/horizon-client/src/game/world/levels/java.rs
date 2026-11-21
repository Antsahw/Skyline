//! Java mods' chunk loaders and items in parked levels.
use super::*;

impl Parked {
    /// The chunks Java mods' chunk loaders keep in this level.
    #[cfg_attr(not(feature = "java-mods"), allow(dead_code))]
    pub(in crate::game) fn set_ticketed(&mut self, chunks: Vec<(i32, i32)>) {
        self.streaming.set_ticketed(chunks);
    }
}

/// What the Java mods' blocks read of parked levels: their item entities (crushing wheels'
/// held items), by dimension.
#[cfg(feature = "java-mods")]
impl Levels {
    /// The item entities of a parked level.
    pub(in crate::game) fn parked_items(&self, dim: Dim) -> Option<&[ItemEntity]> {
        self.parked.get(&dim).map(|level| level.dropped.as_slice())
    }

    /// The item entities of a parked level, to change.
    pub(in crate::game) fn parked_items_mut(&mut self, dim: Dim) -> Option<&mut Vec<ItemEntity>> {
        self.parked.get_mut(&dim).map(|level| &mut *level.dropped)
    }
}

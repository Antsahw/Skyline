//! [`BlockRevision`]: a chunk's blocks' version, for readers that keep what they read (the
//! Java mods' block reads).

/// A number for a chunk's blocks as they are, so a reader can keep what it read: new for
/// each chunk made, each copy of one (a clone may then be changed in any way) and each block
/// set on it ([`super::Chunk::set`]). Java mods' block reads keep whole sections while it stays.
pub struct BlockRevision(u64);
impl BlockRevision {
    pub(super) fn next() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
    pub fn get(&self) -> u64 {
        self.0
    }
}
impl Clone for BlockRevision {
    fn clone(&self) -> Self {
        Self::next()
    }
}

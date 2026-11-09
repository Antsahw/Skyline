//! Java runtimes in processes of their own (`docs/java-bridge-120.md`).
//!
//! JNI allows one JVM per process, and the bridge's JVM runs Fabric for 26.3 (or Forge 1.12,
//! or NeoForge). A second Minecraft version, with its own `net.minecraft` tree, its own Fabric
//! Loader and its own Java version, gets a JVM of its own in a child process, and Horizon
//! talks to it through a socket instead of JNI: [`wire`] (the frames), [`peer`] (synchronous
//! calls that nest both ways, as JNI calls do), [`child`] (starting it, the handshake).
//! [`boot120`] starts Fabric 1.20.1 with unmodified mods there and returns its report;
//! [`content120`] turns their blocks, items and assets into Horizon content mods.

pub mod boot120;
pub mod child;
pub mod content120;
pub mod mirror120;
pub mod peer;
pub mod wire;

pub use peer::{Error, Handler, Peer};

/// The bridge's classes for the 1.20.1 runtime (the transport and `Boot120`), compiled by
/// build.rs; empty when the crate was built without a JDK.
pub const BRIDGE120_JAR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bridge120.jar"));

/// Writes [`BRIDGE120_JAR`] to `path` (when it differs) for a runtime's class path.
pub fn write_bridge120(path: &std::path::Path) -> Result<(), String> {
    if BRIDGE120_JAR.is_empty() {
        return Err("this build has no 1.20.1 bridge classes (it was built without a JDK)".into());
    }
    if std::fs::read(path).ok().as_deref() == Some(BRIDGE120_JAR) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, BRIDGE120_JAR).map_err(|e| format!("{}: {e}", path.display()))
}
pub use wire::Value;

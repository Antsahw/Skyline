//! Java runtimes in processes of their own (`docs/java-bridge-120.md`).
//!
//! JNI allows one JVM per process, and the bridge's JVM runs Fabric for 26.3 (or Forge 1.12,
//! or NeoForge). A second Minecraft version, with its own `net.minecraft` tree, its own Fabric
//! Loader and its own Java version, gets a JVM of its own in a child process, and Horizon
//! talks to it through a socket instead of JNI: [`wire`] (the frames), [`peer`] (synchronous
//! calls that nest both ways, as JNI calls do), [`child`] (starting it, the handshake).
//! [`boot120`] starts Fabric 1.20.1 with unmodified mods there and returns its report.

pub mod boot120;
pub mod child;
pub mod peer;
pub mod wire;

pub use peer::{Error, Handler, Peer};
pub use wire::Value;

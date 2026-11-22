//! What Horizon's files ask the Java mods (docs/java-bridge.md), answered "none" without the
//! `java-mods` feature: each area's plain answers are one file here, so Horizon's own files
//! call the same hooks in both builds and carry no `cfg` of their own. With the feature, the
//! bodies are in the bridge's feature-only files (`java_server/`, `java_entities.rs`,
//! `java_worldgen.rs`, `java_device.rs`).
#![cfg(not(feature = "java-mods"))]

#[path = "java_hooks/blocks.rs"]
mod blocks;
#[path = "java_hooks/entities.rs"]
mod entities;
#[path = "java_hooks/lan.rs"]
mod lan;
#[path = "java_hooks/players.rs"]
mod players;
#[path = "java_hooks/remote.rs"]
mod remote;
#[path = "java_hooks/screens.rs"]
mod screens;
#[path = "java_hooks/worldgen.rs"]
mod worldgen;

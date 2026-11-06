//! What Horizon's files ask the Java mods (docs/java-bridge.md), answered "none" without the
//! `java-mods` feature: each area's plain answers are one file here, so Horizon's own files
//! call the same hooks in both builds and carry no `cfg` of their own. With the feature, the
//! bodies are in the bridge's feature-only files (`java_server/`, `java_entities.rs`,
//! `java_worldgen.rs`, `java_device.rs`).
#![cfg(not(feature = "java-mods"))]

mod blocks;
mod entities;
mod lan;
mod players;
mod remote;
mod screens;
mod worldgen;

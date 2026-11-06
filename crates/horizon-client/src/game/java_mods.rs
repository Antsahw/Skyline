//! Java mods: the Java mod bridge (crate `horizon-jvm`), with the `java-mods` feature. Off
//! unless the player opted in ([`opted_in`]). When they did and
//! `mods-java/` beside `options.txt` holds jars, the game starts the JVM and Fabric Loader on
//! a thread of their own, so the window never waits for them: Fabric loads the mods in
//! `mods-java/` and Fabric API on the player's own game jar, applies their Mixins,
//! bootstraps the game's registries and runs the mods' entrypoints.

/// The Java mods' state in the game (the `java` field of `Game`).
#[derive(Default)]
pub(super) struct JavaGame {
    /// The open window's Java mod screen, as its last frame drew it (game/java_screen.rs).
    pub(super) screen: Option<super::java_screen::JavaScreen>,
    /// The keys the server's Java mods bind (game/java_screen.rs).
    pub(super) binds: super::java_screen::JavaBinds,
    /// What Java mods last drew on the HUD (game/java_screen.rs).
    pub(super) hud: Option<super::java_screen::JavaHud>,
    pub(super) overlay: super::java_screen::JavaOverlay,
    /// Java mods' dynamic textures in the atlas (game/java_screen.rs).
    pub(super) images: super::java_screen::JavaImages,
    /// Java mods' entities the server last sent (game/java_entities.rs).
    #[cfg(feature = "java-mods")]
    pub(super) entities: super::java_entities::JavaEntities,
    /// Whether the spawn search asked the Java mods' generator for its origin (they move it).
    pub(super) spawn_origin_asked: bool,
    /// Players who joined while the Java mods' data-pack load functions hadn't run, with the
    /// ticks they have waited, and the ticks since the functions ran (`hold_join`).
    pub(super) held_joins: Vec<(u32, u32, u32)>,
    /// The launcher's offline skin went to this server's Java mods (`remote/java.rs`).
    pub(super) own_skin_sent: bool,
}

#[cfg(feature = "java-mods")]
mod bridge;
#[cfg(feature = "java-mods")]
pub(super) use bridge::*;

#[cfg(not(feature = "java-mods"))]
mod off;

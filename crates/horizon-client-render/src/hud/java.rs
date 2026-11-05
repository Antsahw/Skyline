//! The Java mod bridge's parts of the HUD atlas: slots for Java mods' dynamic textures
//! (game/java_screen.rs) and the texture folders their screens, HUD and renderers draw from.
use super::{PackedImage, walk};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Atlas slots for Java mods' dynamic textures (images they draw in memory: Chunk Loaders'
/// chunk maps; game/java_screen.rs), each [`JAVA_IMAGE_SIZE`] square; bigger images are shrunk.
pub const JAVA_IMAGE_SLOTS: usize = 128;
pub const JAVA_IMAGE_SIZE: u32 = 64;
/// Bigger slots for Java mods' images that a 64-pixel slot would blur: what they draw with
/// the GPU (Xaero's minimap, a picture in picture at the GUI scale; a full-screen map, sent
/// shrunk to this), a minimap's frame pieces (JourneyMap's rim and mask, 256 px) and the
/// textures of mods' models the atlas hasn't (TACZ's guns, 256-512 px),
/// [`JAVA_BIG_IMAGE_SIZE`] square. Slot `JAVA_IMAGE_SLOTS + i` is `horizon/javabig{i}`.
pub const JAVA_BIG_IMAGE_SLOTS: usize = 16;
pub const JAVA_BIG_IMAGE_SIZE: u32 = 512;

/// A Java image slot's atlas sprite and side (small slots first, then big ones).
pub fn java_image_slot(slot: usize) -> (String, u32) {
    if slot < JAVA_IMAGE_SLOTS {
        (format!("horizon/java{slot}"), JAVA_IMAGE_SIZE)
    } else {
        (
            format!("horizon/javabig{}", slot - JAVA_IMAGE_SLOTS),
            JAVA_BIG_IMAGE_SIZE,
        )
    }
}

/// The Java image slots' empty images, big ones first ([`super::HudAtlas`]'s run-time slots).
pub(super) fn java_slots(images: &mut Vec<PackedImage>) {
    for i in JAVA_IMAGE_SLOTS..JAVA_IMAGE_SLOTS + JAVA_BIG_IMAGE_SLOTS {
        let (name, side) = java_image_slot(i);
        images.push((name, side, side, vec![0; (side * side * 4) as usize]));
    }
    for i in 0..JAVA_IMAGE_SLOTS {
        let side = JAVA_IMAGE_SIZE;
        images.push((
            format!("horizon/java{i}"),
            side,
            side,
            vec![0; (side * side * 4) as usize],
        ));
    }
}

/// A Java mod's texture folder its screens or HUD may draw from: any but those other atlases
/// or no GUI use (items, models, armour, the world's own textures), and those packed apart
/// (block, entity, particle, gui: walked by their own rule).
fn java_gui_folder(name: &str) -> bool {
    !matches!(
        name,
        "item"
            | "items"
            | "block"
            | "blocks"
            | "entity"
            | "particle"
            | "gui"
            | "models"
            | "model"
            | "armor"
            | "trims"
            | "painting"
            | "colormap"
            | "environment"
            | "effect"
            | "mob_effect"
            | "map"
            | "misc"
            | "font"
            | "shaders"
    )
}

/// Java mods (game/java_screen.rs, java_entities.rs): their entity textures,
/// their block textures (models their block entity renderers draw: Chunk
/// Loaders' floating core), their particles, HUD mods' sheets (AppleSkin's
/// `textures/icons.png`, `textures/hud/`) and loose textures, of namespace folder `ns`.
pub(super) fn walk_java_textures(ns: &Path, folders: &[&str], files: &mut Vec<PathBuf>) {
    if !folders.contains(&"entity") {
        walk(&ns.join("textures/entity"), files);
    }
    if !folders.contains(&"particle") {
        walk(&ns.join("textures/particle"), files);
    }
    walk(&ns.join("textures/block"), files);
    for file in fs::read_dir(ns.join("textures"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let path = file.path();
        if path.extension().is_some_and(|e| e == "png") {
            files.push(path);
        } else if path.is_dir() && file.file_name().to_str().is_some_and(java_gui_folder) {
            // Any other folder their screens and HUD may blit from
            // (Simple Voice Chat's `textures/icons/`, `textures/hud/`).
            walk(&path, files);
        }
    }
}

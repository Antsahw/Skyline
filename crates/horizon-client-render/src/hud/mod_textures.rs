//! The Java mod bridge's parts of the HUD atlas: slots for Java mods' dynamic textures
//! (game/java_screen.rs) and the texture folders their screens, HUD and renderers draw from.
use super::{PackedImage, walk};

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

/// The texture folders of namespace `ns` the Java mods' screens, HUD and renderers draw from,
/// beside those the atlas packs for every namespace: their entities' and particles' (when the
/// atlas isn't packing those folders already), their block textures (models their block entity
/// renderers draw: Chunk Loaders' floating core), an empty slot's outline among the items, HUD
/// mods' sheets (AppleSkin's `textures/icons.png`) and loose folders (`textures/hud/`).
pub(super) fn walk_java_textures(
    root: &horizon_assets::resource_source::ResourceSource,
    ns: &str,
    folders: &[&str],
    files: &mut Vec<String>,
) {
    let textures = format!("assets/{ns}/textures");
    if !folders.contains(&"entity") {
        walk(root, &format!("{textures}/entity"), files);
    }
    if !folders.contains(&"particle") {
        walk(root, &format!("{textures}/particle"), files);
    }
    walk(root, &format!("{textures}/block"), files);
    for name in root.list(&format!("{textures}/")) {
        let Some(rest) = name.strip_prefix(&format!("{textures}/")) else {
            continue;
        };
        if !rest.ends_with(".png") {
            continue;
        }
        match rest.split_once('/') {
            // A sheet straight under `textures/` (AppleSkin's `icons.png`).
            None => files.push(name.clone()),
            // An empty slot's outline (a bowl where the bowl goes): in 1.20.1 a slot draws it
            // from the blocks atlas, so it sits among the items, which are otherwise too many.
            Some(("item", file)) if file.starts_with("empty_") && !file.contains('/') => {
                files.push(name.clone())
            }
            // Any other folder their screens and HUD may blit from (Simple Voice Chat's
            // `textures/icons/`, `textures/hud/`).
            Some((folder, _)) if java_gui_folder(folder) => files.push(name.clone()),
            _ => {}
        }
    }
}

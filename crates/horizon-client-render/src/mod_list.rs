//! Installed mod state shared with launcher preparation.
pub use horizon_mod_package::mod_list::*;

/// The side of a Java mod's icon as the Mods screen keeps it: the pack icons' slots' size.
pub const RUNTIME_ICON: u32 = crate::hud::PACK_ICON_SIZE;

/// `rgba`, `w` by `h`, scaled to [`RUNTIME_ICON`] square by nearest pixel (mod icons are mostly
/// pixel art).
pub fn runtime_icon(w: u32, h: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    if w == 0 || h == 0 || rgba.len() < (w * h * 4) as usize {
        return None;
    }
    let n = RUNTIME_ICON;
    let mut out = vec![0; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let (sx, sy) = (x * w / n, y * h / n);
            let from = ((sy * w + sx) * 4) as usize;
            let to = ((y * n + x) * 4) as usize;
            out[to..to + 4].copy_from_slice(&rgba[from..from + 4]);
        }
    }
    Some(out)
}

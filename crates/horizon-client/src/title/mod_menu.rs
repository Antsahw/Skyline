//! What the title screen's Mods button shares with the Mods screen (`game/mod_menu_ui.rs`).
//! From Mod Menu for Horizon by tariksabanuz (MIT), built in.
use std::sync::atomic::{AtomicBool, Ordering};

/// The Mods button, or the key, asked for the Mods screen: the Options screen shows it while
/// this is set.
static OPEN: AtomicBool = AtomicBool::new(false);
/// The screen has just been asked for, so it starts with an empty search box.
static FRESH: AtomicBool = AtomicBool::new(false);

/// A native mod's settings page was opened from the Mods screen, so its Done goes back there.
static FROM_MENU: AtomicBool = AtomicBool::new(false);

pub fn open() {
    OPEN.store(true, Ordering::SeqCst);
    FRESH.store(true, Ordering::SeqCst);
    FROM_MENU.store(false, Ordering::SeqCst);
}

pub fn came_from_menu() -> bool {
    FROM_MENU.load(Ordering::SeqCst)
}

pub fn set_came_from_menu(yes: bool) {
    FROM_MENU.store(yes, Ordering::SeqCst);
}

pub fn close() {
    OPEN.store(false, Ordering::SeqCst);
}

pub fn is_open() -> bool {
    OPEN.load(Ordering::SeqCst)
}

/// True once after [`open`]: the screen starts over.
pub fn take_fresh() -> bool {
    FRESH.swap(false, Ordering::SeqCst)
}

/// The id of the Mods button among the title screen's buttons. Horizon's own buttons use 1 to 15,
/// 30 to 46, 70, 71 and 90 to 95 (8 is Minecraft Realms), so this one sits well clear of them:
/// with a shared id the icon was drawn on the Realms button and not on the Mods button.
pub const BUTTON_ID: u8 = 200;
/// How far the icon row moves left to stay centred with a fourth button in it.
pub const ROW_SHIFT: f32 = 12.0;

/// The button's icon, three bars like a menu (☰), as `(x, y, width, height)` inside the 20 by 20
/// button.
pub const BARS: [(f32, f32, f32, f32); 3] = [
    (4.0, 5.0, 12.0, 2.0),
    (4.0, 9.0, 12.0, 2.0),
    (4.0, 13.0, 12.0, 2.0),
];

/// The block shown for a mod without a picture of its own in the Mods screen: 14 by 14. 1 is the lit top, 2 the left side, 3 the right side.
pub const CUBE: [&str; 14] = [
    "......11......",
    "....111111....",
    "..1111111111..",
    "11111111111111",
    "22111111111133",
    "22221111113333",
    "22222211333333",
    "22222223333333",
    "22222223333333",
    "22222223333333",
    "22222223333333",
    "..2222233333..",
    "....222333....",
    "......23......",
];

/// The icon's runs of one shade, as `(column, row, length, shade)`, so a row is a few rectangles.
pub fn cube_runs() -> Vec<(u8, u8, u8, u8)> {
    let mut runs = Vec::new();
    for (row, line) in CUBE.iter().enumerate() {
        let bytes = line.as_bytes();
        let mut col = 0;
        while col < bytes.len() {
            let shade = bytes[col];
            let mut end = col;
            while end < bytes.len() && bytes[end] == shade {
                end += 1;
            }
            if shade != b'.' {
                runs.push((col as u8, row as u8, (end - col) as u8, shade - b'0'));
            }
            col = end;
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_opens_closes_and_starts_fresh_once() {
        // One test: the flags are shared by the whole process.
        close();
        take_fresh();
        assert!(!is_open() && !take_fresh());
        open();
        assert!(is_open());
        assert!(take_fresh());
        assert!(!take_fresh(), "fresh only once");
        assert!(is_open());
        close();
        assert!(!is_open());
    }

    #[test]
    fn the_button_id_is_not_one_of_horizons() {
        // 8 is Minecraft Realms; none of Horizon's buttons use 96 or more.
        assert!(BUTTON_ID > 95);
    }

    #[test]
    fn the_button_shows_three_bars_that_fit_and_are_centred() {
        for (x, y, w, h) in BARS {
            assert!(x >= 0.0 && y >= 0.0 && x + w <= 20.0 && y + h <= 20.0);
        }
        assert_eq!(BARS[0].2, BARS[2].2);
        assert_eq!(BARS[0].0 * 2.0 + BARS[0].2, 20.0, "centred sideways");
        assert_eq!(
            BARS[0].1 + (BARS[2].1 + BARS[2].3),
            20.0,
            "centred up and down"
        );
        assert_eq!(
            BARS[1].1 - (BARS[0].1 + BARS[0].3),
            BARS[2].1 - (BARS[1].1 + BARS[1].3),
            "even gaps"
        );
    }

    #[test]
    fn the_block_is_a_square_of_three_shades() {
        assert!(CUBE.iter().all(|row| row.len() == 14));
        let runs = cube_runs();
        let mut shades = [0usize; 4];
        for (col, row, len, shade) in &runs {
            assert!(usize::from(*col) + usize::from(*len) <= 14 && *row < 14);
            shades[usize::from(*shade)] += usize::from(*len);
        }
        assert_eq!(shades[0], 0);
        assert!(shades[1] > 30 && shades[2] > 20 && shades[3] > 20);
        // Left and right sides are mirror images: the same number of pixels, give or take the
        // middle column.
        assert!(shades[2].abs_diff(shades[3]) <= 2, "{shades:?}");
        assert_eq!(
            shades.iter().sum::<usize>(),
            CUBE.iter()
                .map(|r| r.bytes().filter(|b| *b != b'.').count())
                .sum::<usize>()
        );
    }
}

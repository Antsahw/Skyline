//! What a crafting grid's take tells the caller's Java mods (their crafted hooks).
use super::*;

/// A crafting result taken from a grid ([`Menu::crafted_takes`]).
#[derive(Clone, Debug)]
pub struct CraftedTake {
    /// The recipe's id.
    pub recipe: String,
    pub taken: ItemStack,
    /// The grid's width (2 or 3).
    pub width: usize,
    /// The grid's cells before the take.
    pub grid: Vec<Option<ItemStack>>,
}
impl CraftedTake {
    /// Recipe `id` (`minecraft:` added) taken as `taken` from a `width`-wide grid of `grid`.
    pub(super) fn of(
        id: &str,
        taken: &ItemStack,
        width: usize,
        grid: &[Option<ItemStack>],
    ) -> Self {
        Self {
            recipe: format!("minecraft:{id}"),
            taken: taken.clone(),
            width,
            grid: grid.to_vec(),
        }
    }
}

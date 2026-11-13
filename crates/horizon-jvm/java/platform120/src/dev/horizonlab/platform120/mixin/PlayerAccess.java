package dev.horizonlab.platform120.mixin;

import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.InventoryMenu;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/**
 * The headless client's player shares the server player's inventory and its menu (Screens120):
 * a screen that reads "the player's inventory" reads the one the menu moves items in.
 */
@Mixin(Player.class)
public interface PlayerAccess {
    @Mutable
    @Accessor("inventory")
    void horizon$setInventory(Inventory inventory);

    @Mutable
    @Accessor("inventoryMenu")
    void horizon$setInventoryMenu(InventoryMenu menu);

    /** The HUD's client player reads its food (AppleSkin): the server player's. */
    @Accessor("foodData")
    void horizon$setFoodData(net.minecraft.world.food.FoodData food);

    @Mutable
    @Accessor("abilities")
    void horizon$setAbilities(net.minecraft.world.entity.player.Abilities abilities);
}

package dev.horizonlab.platform;

import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.inventory.InventoryMenu;

/**
 * The screen under Horizon's inventory in the headless client: a container screen of the
 * player's inventory menu that draws nothing of its own, for mods' overlays on container screens
 * (a recipe viewer's item list and search box) to lay out beside and draw over. Vanilla's InventoryScreen
 * would draw the player's model and the recipe book, which the headless client has no parts for.
 */
final class HorizonOverlayScreen extends AbstractContainerScreen<InventoryMenu> {
    HorizonOverlayScreen(InventoryMenu menu, Inventory inventory) {
        super(menu, inventory, Component.translatable("container.crafting"));
    }

    @Override
    public void extractBackground(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float partialTick) {}

    @Override
    protected void extractLabels(GuiGraphicsExtractor graphics, int mouseX, int mouseY) {}

    @Override
    public void containerTick() {}

    /**
     * The menu's slot {@code index} is the one under the mouse (Horizon's inventory has the
     * mouse over it), as vanilla's frame makes it: keys mods take over a slot read it.
     */
    void hover(int index) {
        hoveredSlot = index >= 0 && index < menu.slots.size() ? menu.slots.get(index) : null;
        if (DEBUG) dev.horizonlab.bridge.Natives.log("overlay: slot " + index + " hovered: " + (hoveredSlot == null ? "none" : hoveredSlot.getItem()));
    }

    private static final boolean DEBUG = String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("screens");
}

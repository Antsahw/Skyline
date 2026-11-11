package dev.horizonlab.platform120;

import net.minecraft.client.gui.GuiGraphics;

/**
 * A mod's screens, drawn into {@link Graphics120} rather than onto a GPU.
 *
 * <p>The drawing a screen does goes through {@code GuiGraphics}, whose calls the mixins take at
 * their head and record, before any of the object's own state is read. So the {@code
 * GuiGraphics} handed to a screen needs no buffers, no pose stack and no client: it is
 * allocated and never filled in, as the mirror's server is.
 */
public final class Screens120 {
    private Screens120() {}

    /** The title of the menu each player last opened: the menu does not keep it. */
    private static final java.util.Map<java.util.UUID, net.minecraft.network.chat.Component> TITLES =
            new java.util.HashMap<>();

    /** A menu opened for `player` with this title (ServerPlayerMenuMixin). */
    public static void opened(net.minecraft.server.level.ServerPlayer player,
            net.minecraft.network.chat.Component title) {
        TITLES.put(player.getUUID(), title);
    }

    /**
     * The menu `player` has open that `before` was not, as its type and its title's JSON, or
     * null when the call left them where they were. A menu is open when the player's container
     * menu is not their own inventory's.
     */
    public static String[] openedSince(net.minecraft.server.level.ServerPlayer player,
            net.minecraft.world.inventory.AbstractContainerMenu before) {
        var menu = player.containerMenu;
        if (menu == null || menu == before || menu == player.inventoryMenu) {
            return null;
        }
        String type;
        try {
            var key = net.minecraft.core.registries.BuiltInRegistries.MENU.getKey(menu.getType());
            type = key == null ? "" : key.toString();
        } catch (UnsupportedOperationException e) {
            // A menu with no type (the player's own inventory has none): not one a screen opens.
            return null;
        }
        var title = TITLES.get(player.getUUID());
        String json = title == null ? "" : net.minecraft.network.chat.Component.Serializer.toJson(title);
        return new String[] {type, json};
    }

    /** A {@code GuiGraphics} that records instead of drawing. */
    public static GuiGraphics recorder() {
        return Allocate.make(GuiGraphics.class);
    }

    /**
     * Draws the shapes a screen draws and gives back what was recorded, as the frame JSON
     * Horizon's screen drawing reads. Proves the recording works end to end -- the mixins bound
     * to the right methods, the operations carry what Horizon needs -- without a mod, a screen
     * or a window, none of which exist this early.
     *
     * <p>If a mixin had not bound, the call would reach vanilla's own drawing and fail on the
     * allocated object's null state instead of returning operations.
     */
    public static Object[] probe() {
        Graphics120 recording = Graphics120.begin();
        GuiGraphics graphics = recorder();
        // What a container screen draws: its background, its border, a label, a slot's item.
        graphics.fill(10, 20, 110, 70, 0xFF202020);
        graphics.fillGradient(10, 20, 110, 24, 0xFFFFFFFF, 0xFF000000);
        graphics.enableScissor(10, 20, 110, 70);
        graphics.drawString(null, "Cooking Pot", 14, 24, 0xFF404040, false);
        graphics.disableScissor();
        return new Object[] {recording.finish().toString()};
    }
}

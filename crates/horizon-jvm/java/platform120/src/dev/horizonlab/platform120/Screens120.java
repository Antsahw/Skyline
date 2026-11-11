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

    /**
     * A {@code GuiGraphics} that records instead of drawing. Allocated, never constructed: its
     * calls are taken before they read its state. The one exception is the pose stack, which a
     * screen translates to draw its slots and labels relative to its own corner, so it gets a
     * real one and the recorder reads where each call really landed.
     */
    public static GuiGraphics recorder() {
        GuiGraphics graphics = Allocate.make(GuiGraphics.class);
        var access = (dev.horizonlab.platform120.mixin.GuiGraphicsAccess) graphics;
        access.horizon$setPose(new com.mojang.blaze3d.vertex.PoseStack());
        access.horizon$setMinecraft(net.minecraft.client.Minecraft.getInstance());
        return graphics;
    }

    /** Each player's open screen, and the menu it was made for. */
    private static final java.util.Map<java.util.UUID, net.minecraft.client.gui.screens.Screen> SCREENS =
            new java.util.HashMap<>();
    private static final java.util.Map<java.util.UUID, net.minecraft.world.inventory.AbstractContainerMenu> MADE_FOR =
            new java.util.HashMap<>();

    /**
     * The font screens measure their text with. A real one needs the client's resources loaded
     * (the glyph sheets, the font manager), which this runtime does not do: Horizon draws the
     * text, with its own font, at the place the screen chose. What the screen needs from a font
     * is only how wide text is, to centre a title or right-align a count, so this is a font that
     * measures and nothing else: 1.20.1's default glyph widths for ASCII, the commonest width
     * for the rest.
     */
    static synchronized net.minecraft.client.gui.Font font() {
        var minecraft = net.minecraft.client.Minecraft.getInstance();
        if (minecraft.font != null) {
            return minecraft.font;
        }
        net.minecraft.client.gui.Font font = Allocate.make(net.minecraft.client.gui.Font.class);
        var access = (dev.horizonlab.platform120.mixin.FontAccess) font;
        access.horizon$setLineHeight(9);
        access.horizon$setSplitter(new net.minecraft.client.StringSplitter(
                (codePoint, style) -> advance(codePoint) + (style.isBold() ? 1 : 0)));
        ((dev.horizonlab.platform120.mixin.MinecraftAccess) minecraft).horizon$setFont(font);
        return font;
    }

    /** How far the default font moves on after a character: its glyph's width and a pixel. */
    static float advance(int codePoint) {
        switch (codePoint) {
            case '!': case ',': case '.': case ':': case ';': case 'i': case '|': case '\'':
                return codePoint == '\'' ? 3 : 2;
            case 'l': case '`':
                return 3;
            case ' ': case 'I': case '[': case ']': case 't':
                return 4;
            case '"': case '(': case ')': case '*': case '<': case '>': case 'f': case 'k':
            case '{': case '}':
                return 5;
            case '@': case '~':
                return 7;
            default:
                return 6;
        }
    }

    /**
     * Builds the screen its mod registered for `player`'s open menu, on the server's menu
     * itself (the screen reads what the menu holds as it is, which in this runtime is the
     * same object a server holds), and lays it out at `width` by `height`. False when the
     * player has no menu open or its mod registered no screen for it.
     */
    public static boolean open(net.minecraft.server.level.ServerPlayer player, int width, int height) {
        var menu = player.containerMenu;
        if (menu == null || menu == player.inventoryMenu) {
            SCREENS.remove(player.getUUID());
            MADE_FOR.remove(player.getUUID());
            return false;
        }
        if (MADE_FOR.get(player.getUUID()) == menu) {
            var screen = SCREENS.get(player.getUUID());
            if (screen != null && (screen.width != width || screen.height != height)) {
                screen.resize(net.minecraft.client.Minecraft.getInstance(), width, height);
            }
            return screen != null;
        }
        Object constructor = dev.horizonlab.platform120.mixin.MenuScreensAccess.horizon$screens().get(menu.getType());
        if (constructor == null) {
            return false;
        }
        var title = TITLES.getOrDefault(player.getUUID(), net.minecraft.network.chat.Component.empty());
        try {
            var screen = (net.minecraft.client.gui.screens.Screen) constructorOf(constructor)
                    .invoke(constructor, menu, player.getInventory(), title);
            font();
            try {
                screen.init(net.minecraft.client.Minecraft.getInstance(), width, height);
            } catch (RuntimeException | LinkageError e) {
                System.out.println("Screens120: laying out " + screen.getClass().getName() + " failed:");
                e.printStackTrace(System.out);
                throw e;
            }
            SCREENS.put(player.getUUID(), screen);
            MADE_FOR.put(player.getUUID(), menu);
            return true;
        } catch (ReflectiveOperationException e) {
            Throwable cause = e.getCause() != null ? e.getCause() : e;
            throw new IllegalStateException("the screen of "
                    + net.minecraft.core.registries.BuiltInRegistries.MENU.getKey(menu.getType())
                    + " could not be made: " + cause, cause);
        }
    }

    /**
     * The method that makes a screen from a menu, on a mod's registered constructor. Found by
     * shape -- the one abstract method of its interface, taking three arguments -- because its
     * interface is package-private and every name in this runtime is intermediary.
     */
    private static java.lang.reflect.Method constructorOf(Object constructor) {
        for (Class<?> type = constructor.getClass(); type != null; type = type.getSuperclass()) {
            for (Class<?> each : type.getInterfaces()) {
                for (var method : each.getMethods()) {
                    if (java.lang.reflect.Modifier.isAbstract(method.getModifiers())
                            && method.getParameterCount() == 3) {
                        method.setAccessible(true);
                        return method;
                    }
                }
            }
        }
        throw new IllegalStateException("no screen constructor on " + constructor.getClass());
    }

    /**
     * One frame of `player`'s open screen, drawn at `width` by `height` with the mouse at
     * (`mouseX`, `mouseY`), as the operations Horizon's screen drawing reads. Null when no
     * screen is open for them.
     */
    public static String frame(net.minecraft.server.level.ServerPlayer player, int width, int height,
            int mouseX, int mouseY) {
        if (!open(player, width, height)) {
            return null;
        }
        var screen = SCREENS.get(player.getUUID());
        Graphics120 recording = Graphics120.begin();
        try {
            screen.render(recorder(), mouseX, mouseY, 0f);
            return recording.finish().toString();
        } catch (RuntimeException | LinkageError e) {
            // The whole stack, in the runtime's log: the error that reaches Horizon carries only
            // the message, and which of the screen's own methods threw is what says what the
            // headless client is missing.
            System.out.println("Screens120: a frame of " + screen.getClass().getName() + " failed:");
            e.printStackTrace(System.out);
            throw e;
        } finally {
            // A screen that threw leaves nothing recording: the next frame starts clean.
            if (Graphics120.current() == recording) {
                recording.finish();
            }
        }
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

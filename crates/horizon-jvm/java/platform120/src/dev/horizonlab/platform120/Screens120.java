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

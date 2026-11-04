package dev.horizonlab.platform;

/**
 * Extractors mods make themselves on the one a screen draws with (a subclass built from its
 * fields, on the same render state: REI's {@code compat.GuiGraphics.of(graphics)}): every
 * method HorizonGraphics records starts by handing the call to the HorizonGraphics whose render
 * state they draw on (PlatformPlugin writes that into GuiGraphicsExtractor), so what they draw
 * is recorded, whatever the mod. Without it they draw as vanilla does, through atlases the
 * headless client lacks, and nothing is recorded.
 */
public final class HorizonGraphicsWrappers {
    private HorizonGraphicsWrappers() {}

    /** The HorizonGraphics an extractor records into: itself, the one whose render state it draws on, or null. */
    public static HorizonGraphics of(Object extractor) {
        if (extractor instanceof HorizonGraphics graphics) return graphics;
        if (extractor instanceof net.minecraft.client.gui.GuiGraphicsExtractor other) return HorizonGraphics.ownerOf(stateOf(other));
        return null;
    }

    /**
     * For GuiGraphicsExtractor's methods (PlatformPlugin): the HorizonGraphics to hand the call
     * to, when {@code self} isn't one and draws on one's render state; else null (its own body).
     */
    public static HorizonGraphics forward(Object self, Object state) {
        if (self instanceof HorizonGraphics) return null;
        return HorizonGraphics.ownerOf(state);
    }

    private static java.lang.reflect.Field state;

    private static Object stateOf(net.minecraft.client.gui.GuiGraphicsExtractor extractor) {
        try {
            if (state == null) {
                var f = net.minecraft.client.gui.GuiGraphicsExtractor.class.getDeclaredField("guiRenderState");
                f.setAccessible(true);
                state = f;
            }
            return state.get(extractor);
        } catch (ReflectiveOperationException e) {
            return null;
        }
    }
}

package dev.horizonlab.platform120;

import net.minecraft.client.gui.Font;
import net.minecraft.client.renderer.MultiBufferSource;
import net.minecraft.network.chat.Component;
import net.minecraft.util.FormattedCharSequence;
import org.joml.Matrix4f;

/**
 * A font whose drawing is recorded, for what draws with the font itself rather than through
 * GuiGraphics: a tooltip component's `renderText` (Farmer's Delight's cooking pot names its
 * meal there). Each `drawInBatch` becomes a text operation of the current recording, moved by
 * the matrix's translation; measuring is the measuring font's ({@link Screens120#font}).
 */
final class TextRecorder120 extends Font {
    private TextRecorder120() {
        super(null, false);
    }

    /** A recording font measuring as `measure` does. */
    static Font of(Font measure) {
        TextRecorder120 font = Allocate.make(TextRecorder120.class);
        var access = (dev.horizonlab.platform120.mixin.FontAccess) (Object) font;
        access.horizon$setLineHeight(measure.lineHeight);
        access.horizon$setSplitter(measure.getSplitter());
        return font;
    }

    private static int[] at(float x, float y, Matrix4f matrix) {
        float dx = matrix == null ? 0 : matrix.m30();
        float dy = matrix == null ? 0 : matrix.m31();
        return new int[] {Math.round(x + dx), Math.round(y + dy)};
    }

    @Override
    public int drawInBatch(String text, float x, float y, int color, boolean shadow, Matrix4f matrix,
            MultiBufferSource buffers, Font.DisplayMode mode, int background, int light) {
        return drawInBatch(text, x, y, color, shadow, matrix, buffers, mode, background, light, false);
    }

    @Override
    public int drawInBatch(String text, float x, float y, int color, boolean shadow, Matrix4f matrix,
            MultiBufferSource buffers, Font.DisplayMode mode, int background, int light, boolean bidi) {
        Graphics120 to = Graphics120.current();
        int[] a = at(x, y, matrix);
        if (to != null && text != null) to.text(text, a[0], a[1], color, shadow);
        return a[0] + (text == null ? 0 : width(text));
    }

    @Override
    public int drawInBatch(Component text, float x, float y, int color, boolean shadow, Matrix4f matrix,
            MultiBufferSource buffers, Font.DisplayMode mode, int background, int light) {
        Graphics120 to = Graphics120.current();
        int[] a = at(x, y, matrix);
        if (to != null && text != null) to.text(text, a[0], a[1], color, shadow);
        return a[0] + (text == null ? 0 : width(text));
    }

    @Override
    public int drawInBatch(FormattedCharSequence text, float x, float y, int color, boolean shadow,
            Matrix4f matrix, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light) {
        Graphics120 to = Graphics120.current();
        int[] a = at(x, y, matrix);
        if (to != null && text != null) to.text(text, a[0], a[1], color, shadow);
        return a[0] + (text == null ? 0 : width(text));
    }
}

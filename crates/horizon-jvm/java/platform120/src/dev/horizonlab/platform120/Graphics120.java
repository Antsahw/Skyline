package dev.horizonlab.platform120;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.world.item.ItemStack;

/**
 * What a 1.20.1 mod's screen draws, recorded for Horizon to draw.
 *
 * <p>The 26.3 bridge records the same way (`HorizonGraphics`) but can extend vanilla's own
 * `GuiGraphicsExtractor`, which 1.20.1 has no equivalent of: there `GuiGraphics` draws straight
 * to the GPU, and this runtime has no GPU, no window and no render system. So the calls are
 * taken by mixins on `GuiGraphics` instead ({@code GuiGraphicsMixin}), which record here and
 * cancel the real drawing. Nothing reaches a GPU.
 *
 * <p>The operations are exactly the ones Horizon's screen drawing already reads
 * (`game/java_screen.rs`, a frame being {@code {"window": id, "ops": [...]}}), so the whole
 * Rust half is shared with the 26.3 bridge rather than written twice.
 */
public final class Graphics120 {
    /** The recorder the mixins write to while a screen draws, or null when none does. */
    private static Graphics120 current;

    private final JsonArray ops = new JsonArray();

    /** Starts recording; what was recorded before is dropped. */
    public static Graphics120 begin() {
        current = new Graphics120();
        return current;
    }

    /** Stops recording and gives what was drawn, in drawing order. */
    public JsonArray finish() {
        current = null;
        return ops;
    }

    /** The recorder a mixin writes to, or null when nothing is being recorded. */
    public static Graphics120 current() {
        return current;
    }

    private JsonObject op(String name) {
        JsonObject o = new JsonObject();
        o.addProperty("op", name);
        ops.add(o);
        return o;
    }

    /** A rectangle as Horizon's screen drawing reads one: its two corners. */
    private static void rect(JsonObject o, int x, int y, int width, int height) {
        o.addProperty("x0", x);
        o.addProperty("y0", y);
        o.addProperty("x1", x + width);
        o.addProperty("y1", y + height);
    }

    /** A filled rectangle. `GuiGraphics.fill`. */
    public void fill(int x0, int y0, int x1, int y1, int color) {
        JsonObject o = op("fill");
        rect(o, Math.min(x0, x1), Math.min(y0, y1), Math.abs(x1 - x0), Math.abs(y1 - y0));
        o.addProperty("color", color);
    }

    /** A rectangle shading from one colour to another. `GuiGraphics.fillGradient`. */
    public void gradient(int x0, int y0, int x1, int y1, int top, int bottom) {
        JsonObject o = op("gradient");
        rect(o, Math.min(x0, x1), Math.min(y0, y1), Math.abs(x1 - x0), Math.abs(y1 - y0));
        o.addProperty("top", top);
        o.addProperty("bottom", bottom);
    }

    /**
     * A part of a texture, the corners already fractions of it: 1.20.1 works the pixels out
     * before it reaches the drawing, and gives the horizontal pair before the vertical.
     */
    public void blitUv(ResourceLocation texture, int x, int y, int width, int height,
            float u0, float v0, float u1, float v1) {
        if (texture == null) {
            return;
        }
        JsonObject o = op("blit");
        o.addProperty("texture", texture.toString());
        rect(o, x, y, width, height);
        o.addProperty("u0", u0);
        o.addProperty("v0", v0);
        o.addProperty("u1", u1);
        o.addProperty("v1", v1);
    }

    /** A line of text. Styled text keeps its styling, as Horizon reads the component's JSON. */
    public void text(Component text, int x, int y, int color, boolean shadow) {
        if (text == null) {
            return;
        }
        JsonObject o = op("text");
        o.addProperty("x", x);
        o.addProperty("y", y);
        o.addProperty("color", color);
        o.addProperty("shadow", shadow);
        o.addProperty("json", Component.Serializer.toJson(text));
    }

    /** A line of already-shaped text: its styling is gone, so it goes as plain characters. */
    public void text(FormattedCharSequence text, int x, int y, int color, boolean shadow) {
        if (text == null) {
            return;
        }
        StringBuilder out = new StringBuilder();
        text.accept((index, style, codePoint) -> {
            out.appendCodePoint(codePoint);
            return true;
        });
        text(out.toString(), x, y, color, shadow);
    }

    /** A line of plain text. */
    public void text(String text, int x, int y, int color, boolean shadow) {
        if (text == null) {
            return;
        }
        JsonObject o = op("text");
        o.addProperty("x", x);
        o.addProperty("y", y);
        o.addProperty("color", color);
        o.addProperty("shadow", shadow);
        o.addProperty("text", text);
    }

    /** An item in a slot, as its NBT: Horizon draws it with its own model. */
    public void item(ItemStack stack, int x, int y) {
        if (stack == null || stack.isEmpty()) {
            return;
        }
        JsonObject o = op("item");
        o.addProperty("x", x);
        o.addProperty("y", y);
        o.addProperty("item", stack.save(new net.minecraft.nbt.CompoundTag()).toString());
    }

    /**
     * A slot's count and durability bar over its item. `text` is what the screen asked to show
     * for the count (null: vanilla's own, the count unless it is one).
     */
    public void decorations(ItemStack stack, int x, int y, String text) {
        if (stack == null || stack.isEmpty()) {
            return;
        }
        JsonObject o = op("decorations");
        o.addProperty("x", x);
        o.addProperty("y", y);
        o.addProperty("item", stack.save(new net.minecraft.nbt.CompoundTag()).toString());
        String count = text != null ? text : stack.getCount() != 1 ? Integer.toString(stack.getCount()) : null;
        if (count != null) {
            o.addProperty("count", count);
        }
    }

    /** A tooltip of lines, each kept as its component's JSON; Horizon places it at its mouse. */
    public void tooltip(java.util.List<? extends Component> lines) {
        if (lines == null || lines.isEmpty()) {
            return;
        }
        JsonArray out = new JsonArray();
        for (Component line : lines) {
            out.add(Component.Serializer.toJson(line));
        }
        op("tooltip").add("lines", out);
    }

    /** A tooltip of already-shaped lines: their styling is gone, so they go as plain text. */
    public void tooltipChars(java.util.List<? extends FormattedCharSequence> lines) {
        if (lines == null || lines.isEmpty()) {
            return;
        }
        java.util.List<Component> plain = new java.util.ArrayList<>();
        for (FormattedCharSequence line : lines) {
            StringBuilder out = new StringBuilder();
            line.accept((index, style, codePoint) -> {
                out.appendCodePoint(codePoint);
                return true;
            });
            plain.add(Component.literal(out.toString()));
        }
        tooltip(plain);
    }

    /** An item's own tooltip, which Horizon makes from the stack. */
    public void itemTooltip(ItemStack stack) {
        if (stack == null || stack.isEmpty()) {
            return;
        }
        op("item_tooltip").addProperty("item", stack.save(new net.minecraft.nbt.CompoundTag()).toString());
    }

    /** Drawing kept inside a rectangle until {@link #unscissor}. */
    public void scissor(int x0, int y0, int x1, int y1) {
        JsonObject o = op("scissor");
        rect(o, Math.min(x0, x1), Math.min(y0, y1), Math.abs(x1 - x0), Math.abs(y1 - y0));
    }

    /** The end of a {@link #scissor}. */
    public void unscissor() {
        op("unscissor");
    }

    /** Whether a GL scissor box is in the frame: GL's is one state, not a stack. */
    private boolean glScissor;

    /**
     * A scissor box set on GL, in window pixels from the bottom left: the window is the frame's
     * size at GUI scale 1 ({@link Screens120#pointer}), so only y turns over.
     */
    public void glScissor(int x, int y, int width, int height) {
        glUnscissor();
        int top = Screens120.windowHeight() - y - height;
        scissor(x, top, x + width, top + height);
        glScissor = true;
    }

    /** GL's scissor test turned off. */
    public void glUnscissor() {
        if (glScissor) {
            unscissor();
            glScissor = false;
        }
    }
}

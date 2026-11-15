package dev.horizonlab.platform120.mixin;

import com.mojang.blaze3d.vertex.BufferBuilder;
import com.mojang.blaze3d.vertex.BufferUploader;
import com.mojang.blaze3d.vertex.DefaultVertexFormat;
import com.mojang.blaze3d.vertex.VertexFormat;
import dev.horizonlab.platform120.Graphics120;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Colored quads a mod draws straight through a BufferBuilder (Jade's tooltip panel, HUD
 * bars), while a frame is being recorded: each becomes a fill, or a gradient when its top
 * and bottom differ, at the corners its vertices give (already through the pose). Other
 * formats and modes are dropped, as there is no GPU to draw them on.
 */
@Mixin(BufferUploader.class)
public abstract class BufferUploaderMixin {
    @Inject(method = "drawWithShader", at = @At("HEAD"), cancellable = true)
    private static void horizon$drawWithShader(BufferBuilder.RenderedBuffer buffer, CallbackInfo ci) {
        Graphics120 to = Graphics120.current();
        if (to == null) return;
        try {
            var state = buffer.drawState();
            if (state.format() == DefaultVertexFormat.POSITION_COLOR && state.mode() == VertexFormat.Mode.QUADS
                    && !state.indexOnly()) {
                var bytes = buffer.vertexBuffer();
                int stride = state.format().getVertexSize();
                int base = bytes.position();
                for (int q = 0; q + 3 < state.vertexCount(); q += 4) {
                    float minX = Float.MAX_VALUE, minY = Float.MAX_VALUE, maxX = -Float.MAX_VALUE, maxY = -Float.MAX_VALUE;
                    int top = 0, bottom = 0;
                    float topY = Float.MAX_VALUE, bottomY = -Float.MAX_VALUE;
                    for (int v = 0; v < 4; v++) {
                        int at = base + (q + v) * stride;
                        float x = bytes.getFloat(at), y = bytes.getFloat(at + 4);
                        // Color as bytes R, G, B, A: ARGB as GuiGraphics takes it.
                        int r = bytes.get(at + 12) & 0xFF, g = bytes.get(at + 13) & 0xFF;
                        int b = bytes.get(at + 14) & 0xFF, a = bytes.get(at + 15) & 0xFF;
                        int argb = a << 24 | r << 16 | g << 8 | b;
                        minX = Math.min(minX, x);
                        maxX = Math.max(maxX, x);
                        minY = Math.min(minY, y);
                        maxY = Math.max(maxY, y);
                        if (y < topY) {
                            topY = y;
                            top = argb;
                        }
                        if (y > bottomY) {
                            bottomY = y;
                            bottom = argb;
                        }
                    }
                    int x0 = Math.round(minX), y0 = Math.round(minY), x1 = Math.round(maxX), y1 = Math.round(maxY);
                    if (x1 <= x0 || y1 <= y0) continue;
                    if (top == bottom) to.fill(x0, y0, x1, y1, top);
                    else to.gradient(x0, y0, x1, y1, top, bottom);
                }
            }
        } finally {
            buffer.release();
            ci.cancel();
        }
    }
}

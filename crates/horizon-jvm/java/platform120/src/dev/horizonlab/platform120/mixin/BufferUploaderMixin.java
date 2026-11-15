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
 * Quads a mod draws straight through a BufferBuilder (Jade's tooltip panel and hearts, HUD
 * bars), while a frame is being recorded: colored ones become fills, or gradients when their
 * top and bottom differ; textured ones blits of the shader's texture; at the corners their
 * vertices give (already through the pose). Other formats and modes are dropped, as there is
 * no GPU to draw them on.
 */
@Mixin(BufferUploader.class)
public abstract class BufferUploaderMixin {
    @Inject(method = "drawWithShader", at = @At("HEAD"), cancellable = true)
    private static void horizon$drawWithShader(BufferBuilder.RenderedBuffer buffer, CallbackInfo ci) {
        Graphics120 to = Graphics120.current();
        if (to == null) return;
        try {
            var state = buffer.drawState();
            if (state.mode() == VertexFormat.Mode.QUADS && !state.indexOnly()
                    && state.format().getElements().contains(DefaultVertexFormat.ELEMENT_UV0)) {
                textured(to, buffer.vertexBuffer(), state.format(), state.vertexCount());
            } else if (state.format() == DefaultVertexFormat.POSITION_COLOR && state.mode() == VertexFormat.Mode.QUADS
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

    /**
     * Textured quads: each a blit of the shader's texture (Graphics120.shaderTexture) at the
     * corners its positions give, from the corners its texture coordinates give (Jade's hearts
     * from icons.png, which the recorder maps to 26.3's sprites).
     */
    private static void textured(Graphics120 to, java.nio.ByteBuffer bytes, VertexFormat format, int count) {
        var texture = Graphics120.shaderTexture;
        if (texture == null) return;
        int stride = format.getVertexSize(), position = -1, uv = -1, offset = 0;
        for (var element : format.getElements()) {
            if (element == DefaultVertexFormat.ELEMENT_POSITION) position = offset;
            if (element == DefaultVertexFormat.ELEMENT_UV0) uv = offset;
            offset += element.getByteSize();
        }
        if (position < 0 || uv < 0) return;
        int base = bytes.position();
        for (int q = 0; q + 3 < count; q += 4) {
            float minX = Float.MAX_VALUE, minY = Float.MAX_VALUE, maxX = -Float.MAX_VALUE, maxY = -Float.MAX_VALUE;
            float u0 = 0, v0 = 0, u1 = 0, v1 = 0;
            for (int v = 0; v < 4; v++) {
                int at = base + (q + v) * stride;
                float x = bytes.getFloat(at + position), y = bytes.getFloat(at + position + 4);
                float u = bytes.getFloat(at + uv), w = bytes.getFloat(at + uv + 4);
                // The texture's corners go with the position's: the top-left vertex's, the
                // bottom-right's.
                if (x <= minX && y <= minY) {
                    u0 = u;
                    v0 = w;
                }
                if (x >= maxX && y >= maxY) {
                    u1 = u;
                    v1 = w;
                }
                minX = Math.min(minX, x);
                maxX = Math.max(maxX, x);
                minY = Math.min(minY, y);
                maxY = Math.max(maxY, y);
            }
            int x0 = Math.round(minX), y0 = Math.round(minY), x1 = Math.round(maxX), y1 = Math.round(maxY);
            if (x1 <= x0 || y1 <= y0) continue;
            to.blitUv(texture, x0, y0, x1 - x0, y1 - y0, u0, v0, u1, v1);
        }
    }
}

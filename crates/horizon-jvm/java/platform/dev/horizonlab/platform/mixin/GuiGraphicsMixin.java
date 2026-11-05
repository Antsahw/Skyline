package dev.horizonlab.platform.mixin;

import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import dev.horizonlab.platform.HorizonGraphics;
import dev.horizonlab.platform.HorizonGraphicsWrappers;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * GuiGraphicsExtractor's private innerBlit, which mods' access wideners open (public and
 * final, so no subclass overrides it) and call themselves (REI's textured widgets): on a
 * HorizonGraphics or a wrapper of one, recorded as an image instead of drawn through the
 * texture manager the headless client lacks.
 */
@Mixin(GuiGraphicsExtractor.class)
public abstract class GuiGraphicsMixin {
    @Inject(method = "innerBlit(Lcom/mojang/renderpearl/api/pipeline/RenderPipeline;Lnet/minecraft/resources/Identifier;IIIIFFFFI)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$innerBlit(RenderPipeline pipeline, Identifier texture, int x0, int x1, int y0, int y1,
                                   float u0, float u1, float v0, float v1, int color, CallbackInfo ci) {
        HorizonGraphics graphics = HorizonGraphicsWrappers.of(this);
        if (graphics == null) return;
        ci.cancel();
        graphics.horizon$innerBlit(pipeline, texture, x0, x1, y0, y1, u0, u1, v0, v1, color);
    }

    /**
     * Its private blitTiledSprite (blitSprite's tiled scaling; JEI's fluids, mods' tanks): the
     * sprite's own picture tiled, as Horizon draws named textures; the atlas's texture and a
     * tiled blit through it are the headless client's GPU's, which Horizon's window never sees
     * (a fluid was a black square).
     */
    @Inject(method = "blitTiledSprite(Lcom/mojang/renderpearl/api/pipeline/RenderPipeline;Lnet/minecraft/client/renderer/texture/TextureAtlasSprite;IIIIIIIIIII)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$tiledSprite(RenderPipeline pipeline, net.minecraft.client.renderer.texture.TextureAtlasSprite sprite,
                                     int x, int y, int w, int h, int u, int v, int tileW, int tileH, int texW, int texH,
                                     int color, CallbackInfo ci) {
        HorizonGraphics graphics = HorizonGraphicsWrappers.of(this);
        if (graphics == null) return;
        ci.cancel();
        graphics.horizon$tiledSprite(sprite, x, y, w, h, u, v, tileW, tileH, texW, texH, color);
    }
}

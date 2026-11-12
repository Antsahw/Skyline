package dev.horizonlab.platform120.mixin;

import com.mojang.blaze3d.platform.NativeImage;
import net.minecraft.client.renderer.texture.SpriteContents;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A stand-in sprite (Sprites120) has no image: Horizon draws sprites from its own atlases. Vanilla's
 * item model generator reads a sprite's pixels for the edges of a flat item; with no image every
 * pixel is clear, so such an item bakes with its front and back only, where it threw (and logged)
 * for each of vanilla's items as the client's models were baked.
 */
@Mixin(SpriteContents.class)
public abstract class SpriteContentsMixin {
    @Shadow @Final private NativeImage originalImage;

    @Inject(method = "isTransparent", at = @At("HEAD"), cancellable = true)
    private void horizon$noImage(int frame, int x, int y, CallbackInfoReturnable<Boolean> cir) {
        if (originalImage == null) cir.setReturnValue(true);
    }
}

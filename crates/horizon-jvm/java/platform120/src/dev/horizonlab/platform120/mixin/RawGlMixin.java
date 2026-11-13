package dev.horizonlab.platform120.mixin;

import org.lwjgl.opengl.GL11;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * GL called straight through LWJGL by mods, past GlStateManager (GlStateManagerMixin): this
 * runtime has no GL context, and LWJGL then aborts the whole JVM, which no handler can catch.
 * AppleSkin's HUD asks whether blending is on before it draws its overlay; Moonlight sets
 * pixel storage around reading a texture back. Answered as by a context with everything off.
 */
@Mixin(value = GL11.class, remap = false)
public abstract class RawGlMixin {
    @Inject(method = "glIsEnabled(I)Z", at = @At("HEAD"), cancellable = true, require = 0)
    private static void horizon$isEnabled(int cap, CallbackInfoReturnable<Boolean> cir) {
        cir.setReturnValue(false);
    }

    @Inject(method = "glGetTexLevelParameteri(III)I", at = @At("HEAD"), cancellable = true, require = 0)
    private static void horizon$texLevel(int target, int level, int name, CallbackInfoReturnable<Integer> cir) {
        cir.setReturnValue(0);
    }

    @Inject(method = {"glPixelStorei(II)V", "glEnable(I)V", "glDisable(I)V"},
            at = @At("HEAD"), cancellable = true, require = 0)
    private static void horizon$noGl(CallbackInfo ci) {
        ci.cancel();
    }
}

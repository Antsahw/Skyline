package dev.horizonlab.platform120.mixin;

import com.mojang.blaze3d.systems.RenderSystem;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The headless client has no render thread: nothing here renders. RenderSystem checks it is on
 * one before touching GL, which a mod's screen does around its drawing; with every GL call
 * stopped (GlStateManagerMixin) there is nothing for the check to protect, so it always passes.
 *
 * <p>Calls made off the render thread are queued for it to run later. With no render thread
 * the queue would only grow, and what is in it -- a shader colour, a texture to bind -- is
 * nothing Horizon reads, so it is dropped.
 */
@Mixin(RenderSystem.class)
public abstract class RenderSystemMixin {
    @Inject(method = {"assertOnRenderThread()V", "assertOnRenderThreadOrInit()V"},
            at = @At("HEAD"), cancellable = true)
    private static void horizon$anyThread(CallbackInfo ci) {
        ci.cancel();
    }

    @Inject(method = "recordRenderCall(Lcom/mojang/blaze3d/pipeline/RenderCall;)V",
            at = @At("HEAD"), cancellable = true)
    private static void horizon$noQueue(CallbackInfo ci) {
        ci.cancel();
    }
}

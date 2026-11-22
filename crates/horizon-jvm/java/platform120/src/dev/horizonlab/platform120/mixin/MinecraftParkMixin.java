package dev.horizonlab.platform120.mixin;

import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * A client that went headless (Forge's, once its mods loaded: Loader120.goHeadless) runs no more
 * frames: its render thread waits here, and the platform answers Horizon from the conversation
 * thread as the Fabric runtime's headless client does. Its own level and screens are never
 * ticked or drawn (they are the platform's, made for Horizon's calls). The Fabric runtime's client
 * never runs a frame at all.
 */
@Mixin(Minecraft.class)
public abstract class MinecraftParkMixin {
    @Inject(method = "runTick(Z)V", at = @At("HEAD"), cancellable = true)
    private void horizon$park(boolean render, CallbackInfo ci) {
        if (!dev.horizonlab.platform120.Loader120.headless()) {
            dev.horizonlab.platform120.Loader120.runWhenLoaded(((Minecraft) (Object) this).getOverlay() == null);
            return;
        }
        try {
            Thread.sleep(50);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
        ci.cancel();
    }
}

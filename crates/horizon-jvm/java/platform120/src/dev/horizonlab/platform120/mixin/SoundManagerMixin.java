package dev.horizonlab.platform120.mixin;

import net.minecraft.client.sounds.SoundManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The headless client's sound manager is an empty one ({@code Screens120.pointer}): a mod's
 * button plays its click through it (AbstractWidget.playDownSound), and Horizon plays the
 * sounds of its screens itself. Every void call that would reach the sound engine is stopped.
 */
@Mixin(SoundManager.class)
public abstract class SoundManagerMixin {
    @Inject(method = {
            "play(Lnet/minecraft/client/resources/sounds/SoundInstance;)V",
            "playDelayed(Lnet/minecraft/client/resources/sounds/SoundInstance;I)V",
            "queueTickingSound(Lnet/minecraft/client/resources/sounds/TickableSoundInstance;)V",
            "stop(Lnet/minecraft/client/resources/sounds/SoundInstance;)V",
            "stop(Lnet/minecraft/resources/ResourceLocation;Lnet/minecraft/sounds/SoundSource;)V",
            "stop()V",
            "pause()V",
            "resume()V",
            "tick(Z)V",
            "updateSourceVolume(Lnet/minecraft/sounds/SoundSource;F)V"
    }, at = @At("HEAD"), cancellable = true, require = 0)
    private void horizon$silent(CallbackInfo ci) {
        ci.cancel();
    }
}

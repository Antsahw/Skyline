package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonClient;
import dev.horizonlab.platform.HorizonSounds;
import net.minecraft.client.resources.sounds.SoundInstance;
import net.minecraft.client.resources.sounds.TickableSoundInstance;
import net.minecraft.client.sounds.SoundEngine;
import net.minecraft.client.sounds.SoundManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * The headless client's sound manager (made without its constructor, so without a sound
 * engine): what mods' client code plays goes to the player's game, which plays it
 * (HorizonSounds); a screen's button click, for one, reaches here.
 */
@Mixin(SoundManager.class)
public abstract class SoundManagerMixin {
    @Inject(method = "play", at = @At("HEAD"), cancellable = true)
    private void horizon$play(SoundInstance sound, CallbackInfoReturnable<SoundEngine.PlayResult> cir) {
        if (!HorizonClient.headless()) return;
        cir.setReturnValue(HorizonSounds.play(sound) ? SoundEngine.PlayResult.STARTED : SoundEngine.PlayResult.NOT_STARTED);
    }

    @Inject(method = "playDelayed", at = @At("HEAD"), cancellable = true)
    private void horizon$playDelayed(SoundInstance sound, int delay, CallbackInfo ci) {
        if (!HorizonClient.headless()) return;
        ci.cancel();
        HorizonSounds.play(sound);
    }

    @Inject(method = "queueTickingSound", at = @At("HEAD"), cancellable = true)
    private void horizon$queue(TickableSoundInstance sound, CallbackInfo ci) {
        if (!HorizonClient.headless()) return;
        ci.cancel();
        HorizonSounds.play(sound);
    }

    @Inject(method = "stop(Lnet/minecraft/client/resources/sounds/SoundInstance;)V", at = @At("HEAD"), cancellable = true)
    private void horizon$stop(SoundInstance sound, CallbackInfo ci) {
        if (!HorizonClient.headless()) return;
        ci.cancel();
        HorizonSounds.stop(sound);
    }

    @Inject(method = "stop()V", at = @At("HEAD"), cancellable = true)
    private void horizon$stopAll(CallbackInfo ci) {
        if (!HorizonClient.headless()) return;
        ci.cancel();
        HorizonSounds.stopAll();
    }

    @Inject(method = "stop(Lnet/minecraft/resources/Identifier;Lnet/minecraft/sounds/SoundSource;)V", at = @At("HEAD"), cancellable = true)
    private void horizon$stopNamed(net.minecraft.resources.Identifier sound, net.minecraft.sounds.SoundSource source, CallbackInfo ci) {
        if (!HorizonClient.headless()) return;
        ci.cancel();
        HorizonSounds.stopNamed(sound, source);
    }

    @Inject(method = {"tick", "pauseAllExcept", "resume", "updateSource", "refreshCategoryVolume", "updateCategoryVolume"},
            at = @At("HEAD"), cancellable = true, require = 0)
    private void horizon$noEngine(CallbackInfo ci) {
        if (HorizonClient.headless()) ci.cancel();
    }

    @Inject(method = "isActive", at = @At("HEAD"), cancellable = true)
    private void horizon$active(SoundInstance sound, CallbackInfoReturnable<Boolean> cir) {
        if (HorizonClient.headless()) cir.setReturnValue(HorizonSounds.active(sound));
    }
}

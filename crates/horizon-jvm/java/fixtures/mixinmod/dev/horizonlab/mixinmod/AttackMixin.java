package dev.horizonlab.mixinmod;

import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Pigs can't be hit (the head of Player.attack). */
@Mixin(Player.class)
public abstract class AttackMixin {
    @Inject(method = "attack", at = @At("HEAD"), cancellable = true)
    private void horizonMixins$sparePigs(Entity target, CallbackInfo ci) {
        if (net.minecraft.core.registries.BuiltInRegistries.ENTITY_TYPE.getKey(target.getType()).getPath().equals("pig")) ci.cancel();
    }
}

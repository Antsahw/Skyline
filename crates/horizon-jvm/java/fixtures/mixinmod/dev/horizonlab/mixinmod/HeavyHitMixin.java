package dev.horizonlab.mixinmod;

import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;

/** Inside Player.attack: every hit does ten times the damage (a cow dies to one punch). */
@Mixin(Player.class)
public abstract class HeavyHitMixin {
    @ModifyArg(method = "attack", at = @At(value = "INVOKE",
            target = "Lnet/minecraft/world/entity/Entity;hurtOrSimulate(Lnet/minecraft/world/damagesource/DamageSource;F)Z"), index = 1)
    private float horizonMixins$heavy(float damage) {
        return damage * 10;
    }
}

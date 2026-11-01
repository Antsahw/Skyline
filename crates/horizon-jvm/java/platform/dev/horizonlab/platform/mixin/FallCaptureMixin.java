package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonVanillaActions;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A player's routed fall (HorizonVanillaActions.fall): calculateFallDamage answers with
 * Horizon's attributes (the mods' wraps around the call and changes to its arguments still
 * apply), and the damage reaching hurtServer is caught for Horizon to deal.
 */
@Mixin(value = LivingEntity.class, priority = Integer.MAX_VALUE)
public abstract class FallCaptureMixin {
    @Inject(method = "calculateFallDamage", at = @At("HEAD"), cancellable = true)
    private void horizon$fallDamage(double distance, float multiplier, CallbackInfoReturnable<Integer> cir) {
        if (HorizonVanillaActions.falling(this)) cir.setReturnValue(HorizonVanillaActions.fallDamage(distance, multiplier));
    }

    @Inject(method = "hurtServer", at = @At("HEAD"), cancellable = true)
    private void horizon$catchFall(ServerLevel level, DamageSource source, float amount, CallbackInfoReturnable<Boolean> cir) {
        if (!HorizonVanillaActions.falling(this)) return;
        HorizonVanillaActions.caughtFall(amount);
        cir.setReturnValue(amount > 0);
    }
}

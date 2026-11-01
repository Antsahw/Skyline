package dev.horizonlab.mixinmod;

import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.Items;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A player holding a feather takes no fall damage (the head of LivingEntity.causeFallDamage);
 * one holding a slime ball falls half as far (its distance argument).
 */
@Mixin(LivingEntity.class)
public abstract class FallMixin {
    @Inject(method = "causeFallDamage", at = @At("HEAD"), cancellable = true)
    private void horizonMixins$feather(double distance, float multiplier, DamageSource source, CallbackInfoReturnable<Boolean> cir) {
        if ((Object) this instanceof Player player && player.getMainHandItem().is(Items.FEATHER)) cir.setReturnValue(false);
    }

    @ModifyVariable(method = "causeFallDamage", at = @At("HEAD"), argsOnly = true)
    private double horizonMixins$slime(double distance) {
        return (Object) this instanceof Player player && player.getMainHandItem().is(Items.SLIME_BALL) ? distance / 2 : distance;
    }
}

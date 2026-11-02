package dev.horizonlab.mixinmod;

import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.effect.MobEffects;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Players are immune to poison, haste and Bad Omen (the head of LivingEntity.addEffect). */
@Mixin(LivingEntity.class)
public abstract class EffectMixin {
    @Inject(method = "addEffect(Lnet/minecraft/world/effect/MobEffectInstance;Lnet/minecraft/world/entity/Entity;)Z", at = @At("HEAD"),
            cancellable = true)
    private void horizonMixins$noPoison(MobEffectInstance effect, Entity source, CallbackInfoReturnable<Boolean> cir) {
        if ((Object) this instanceof Player && (effect.is(MobEffects.POISON) || effect.is(MobEffects.HASTE) || effect.is(MobEffects.BAD_OMEN))) {
            cir.setReturnValue(false);
        }
    }
}

package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonCombat;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A routed attack's copies of Horizon's mobs (HorizonCombat): what reaches hurtServer and
 * knockback is caught, after the mods' own changes at those heads (the highest priority comes
 * last), and vanilla's body doesn't run on the copy: Horizon hurts and pushes its own mob.
 */
@Mixin(value = LivingEntity.class, priority = Integer.MAX_VALUE)
public abstract class CombatCaptureMixin {
    @Inject(method = "hurtServer", at = @At("HEAD"), cancellable = true)
    private void horizon$catchHurt(ServerLevel level, DamageSource source, float amount, CallbackInfoReturnable<Boolean> cir) {
        if (!HorizonCombat.isCopy(this)) return;
        HorizonCombat.caughtHurt((LivingEntity) (Object) this, source, amount);
        cir.setReturnValue(amount > 0);
    }

    @Inject(method = "knockback(DDDLnet/minecraft/world/damagesource/DamageSource;FZ)V", at = @At("HEAD"), cancellable = true)
    private void horizon$catchKnockback(double strength, double x, double z, DamageSource source, float amount, boolean resisted,
                                        CallbackInfo ci) {
        if (!HorizonCombat.isCopy(this)) return;
        HorizonCombat.caughtKnockback((LivingEntity) (Object) this, strength, x, z);
        ci.cancel();
    }
}

package dev.horizonlab.platform120.mixin;

import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/**
 * How charged a player's blow is: the mirror player never ticks, so its count of ticks since it
 * last swung is Horizon's (Player.getAttackStrengthScale reads it).
 */
@Mixin(LivingEntity.class)
public interface LivingEntityAccess {
    @Accessor("attackStrengthTicker")
    void horizon$setAttackStrengthTicker(int ticks);
}

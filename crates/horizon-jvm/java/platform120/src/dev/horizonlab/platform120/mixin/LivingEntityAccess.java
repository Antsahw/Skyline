package dev.horizonlab.platform120.mixin;

import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/**
 * How charged a player's blow is: the mirror player never ticks, so its count of ticks since it
 * last swung is Horizon's (Player.getAttackStrengthScale reads it).
 */
@Mixin(LivingEntity.class)
public interface LivingEntityAccess {
    @Accessor("attackStrengthTicker")
    void horizon$setAttackStrengthTicker(int ticks);

    /** The HUD's client player: the server player's attributes (max health, armour). */
    @Mutable
    @Accessor("attributes")
    void horizon$setAttributes(net.minecraft.world.entity.ai.attributes.AttributeMap attributes);

    /** And its effects (hunger, poison: the HUD's colours). */
    @Mutable
    @Accessor("activeEffects")
    void horizon$setActiveEffects(java.util.Map<net.minecraft.world.effect.MobEffect, net.minecraft.world.effect.MobEffectInstance> effects);

    @Accessor("absorptionAmount")
    float horizon$absorption();
}

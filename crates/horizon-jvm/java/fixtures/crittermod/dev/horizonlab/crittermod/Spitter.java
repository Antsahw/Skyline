package dev.horizonlab.crittermod;

import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.ai.goal.FloatGoal;
import net.minecraft.world.entity.ai.goal.RangedAttackGoal;
import net.minecraft.world.entity.ai.goal.target.NearestAttackableTargetGoal;
import net.minecraft.world.entity.animal.sheep.Sheep;
import net.minecraft.world.entity.monster.RangedAttackMob;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.Level;

/**
 * A critter that stands and spits Pebbles (the mod's own projectile) at the nearest player, else
 * at a sheep: a check of a mod's projectile hitting Horizon's players and mobs.
 */
public class Spitter extends Critter implements net.minecraft.world.entity.monster.Enemy, RangedAttackMob {
    public Spitter(EntityType<? extends Spitter> type, Level level) {
        super(type, level);
    }

    @Override
    protected void registerGoals() {
        goalSelector.addGoal(0, new FloatGoal(this));
        // Speed 0: it stands where it was put and only turns and spits, once a second.
        goalSelector.addGoal(2, new RangedAttackGoal(this, 0.0, 20, 12.0F));
        targetSelector.addGoal(1, new NearestAttackableTargetGoal<>(this, Player.class, true));
        targetSelector.addGoal(2, new NearestAttackableTargetGoal<>(this, Sheep.class, false));
    }

    @Override
    public void performRangedAttack(LivingEntity target, float power) {
        Pebble pebble = new Pebble(CritterMod.PEBBLE, level());
        pebble.setOwner(this);
        pebble.setPos(getX(), getEyeY() - 0.1, getZ());
        double dx = target.getX() - getX();
        double dy = target.getY(0.5) - pebble.getY();
        double dz = target.getZ() - getZ();
        double flat = Math.sqrt(dx * dx + dz * dz);
        pebble.shoot(dx, dy + flat * 0.12, dz, 1.4F, 0.0F);
        level().addFreshEntity(pebble);
    }
}

package dev.horizonlab.crittermod;

import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.ai.goal.FloatGoal;
import net.minecraft.world.entity.ai.goal.MeleeAttackGoal;
import net.minecraft.world.entity.ai.goal.RandomStrollGoal;
import net.minecraft.world.entity.ai.goal.target.NearestAttackableTargetGoal;
import net.minecraft.world.entity.animal.sheep.Sheep;
import net.minecraft.world.level.Level;

/** A critter that hunts sheep and iron golems (tests: Horizon's own) and bites (3 damage); not an Enemy. */
public class Hunter extends Critter {
    public Hunter(EntityType<? extends Hunter> type, Level level) {
        super(type, level);
    }

    @Override
    protected void registerGoals() {
        goalSelector.addGoal(0, new FloatGoal(this));
        goalSelector.addGoal(2, new MeleeAttackGoal(this, 1.2, false));
        goalSelector.addGoal(5, new RandomStrollGoal(this, 1.0));
        targetSelector.addGoal(1, new NearestAttackableTargetGoal<>(this, Sheep.class, false));
        // Iron golems too (a test: Horizon's golem, which doesn't hunt it, strikes back).
        targetSelector.addGoal(2, new NearestAttackableTargetGoal<>(this, net.minecraft.world.entity.animal.golem.IronGolem.class, false));
    }
}

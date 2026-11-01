package dev.horizonlab.crittermod;

import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.Level;
import net.minecraft.world.phys.Vec3;

/**
 * A critter a player rides (as a strider or a saddled pig, without the saddle): a use gets on,
 * the rider steers it with the movement keys and faces it where they look, sneaking gets off.
 */
public class Mount extends Critter {
    public Mount(EntityType<? extends Mount> type, Level level) {
        super(type, level);
    }

    /** It stands still unless ridden (a test finds it where it was summoned). */
    @Override
    protected void registerGoals() {
        goalSelector.addGoal(0, new net.minecraft.world.entity.ai.goal.FloatGoal(this));
        goalSelector.addGoal(6, new net.minecraft.world.entity.ai.goal.LookAtPlayerGoal(this, Player.class, 6.0F));
    }

    @Override
    protected InteractionResult mobInteract(Player player, InteractionHand hand) {
        if (isVehicle() || player.isSecondaryUseActive()) return super.mobInteract(player, hand);
        if (!level().isClientSide()) player.startRiding(this);
        return InteractionResult.SUCCESS;
    }

    @Override
    public LivingEntity getControllingPassenger() {
        return getFirstPassenger() instanceof Player player ? player : super.getControllingPassenger();
    }

    @Override
    protected void tickRidden(Player player, Vec3 input) {
        super.tickRidden(player, input);
        setRot(player.getYRot(), player.getXRot() * 0.5F);
        yRotO = yBodyRot = yHeadRot = getYRot();
    }

    @Override
    protected Vec3 getRiddenInput(Player player, Vec3 input) {
        return new Vec3(player.xxa * 0.5F, 0.0, player.zza);
    }

    @Override
    protected float getRiddenSpeed(Player player) {
        return 0.3F;
    }
}

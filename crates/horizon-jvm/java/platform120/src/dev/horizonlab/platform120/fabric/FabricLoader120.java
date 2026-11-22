package dev.horizonlab.platform120.fabric;

import dev.horizonlab.platform120.Loader120;
import net.fabricmc.fabric.api.event.player.PlayerBlockBreakEvents;
import net.fabricmc.fabric.api.event.player.UseEntityCallback;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.EntityHitResult;

/** {@link Loader120} under Fabric: Fabric API's events. */
public final class FabricLoader120 implements Loader120 {
    @Override
    public InteractionResult useEntity(ServerPlayer player, ServerLevel level, InteractionHand hand, Entity target, EntityHitResult hit) {
        return UseEntityCallback.EVENT.invoker().interact(player, level, hand, target, hit);
    }

    @Override
    public boolean beforeBreak(ServerLevel level, ServerPlayer player, BlockPos pos, BlockState state, BlockEntity entity) {
        return PlayerBlockBreakEvents.BEFORE.invoker().beforeBlockBreak(level, player, pos, state, entity);
    }

    @Override
    public void canceledBreak(ServerLevel level, ServerPlayer player, BlockPos pos, BlockState state, BlockEntity entity) {
        PlayerBlockBreakEvents.CANCELED.invoker().onBlockBreakCanceled(level, player, pos, state, entity);
    }

    @Override
    public void afterBreak(ServerLevel level, ServerPlayer player, BlockPos pos, BlockState state, BlockEntity entity) {
        PlayerBlockBreakEvents.AFTER.invoker().afterBlockBreak(level, player, pos, state, entity);
    }
}

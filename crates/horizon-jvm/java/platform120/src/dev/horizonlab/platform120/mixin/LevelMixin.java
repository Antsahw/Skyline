package dev.horizonlab.platform120.mixin;

import dev.horizonlab.platform120.Mirror120;
import net.minecraft.core.BlockPos;
import net.minecraft.core.RegistryAccess;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.material.FluidState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A mirror level's blocks are Horizon's (Mirror120): reads ask Horizon, writes go back to it.
 * Server levels only: the headless client's levels (none yet) keep vanilla's.
 */
@Mixin(Level.class)
public abstract class LevelMixin {
    private boolean horizon$mirror() {
        return !((Level) (Object) this).isClientSide;
    }

    @Inject(method = "getBlockState", at = @At("HEAD"), cancellable = true)
    private void horizon$blockState(BlockPos pos, CallbackInfoReturnable<BlockState> cir) {
        if (horizon$mirror()) cir.setReturnValue(Mirror120.blockState((Level) (Object) this, pos));
    }

    @Inject(method = "getFluidState", at = @At("HEAD"), cancellable = true)
    private void horizon$fluidState(BlockPos pos, CallbackInfoReturnable<FluidState> cir) {
        if (horizon$mirror()) cir.setReturnValue(Mirror120.blockState((Level) (Object) this, pos).getFluidState());
    }

    @Inject(method = "setBlock(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/state/BlockState;II)Z",
            at = @At("HEAD"), cancellable = true)
    private void horizon$setBlock(BlockPos pos, BlockState state, int flags, int recursion, CallbackInfoReturnable<Boolean> cir) {
        if (horizon$mirror()) cir.setReturnValue(Mirror120.setBlock((Level) (Object) this, pos, state, flags));
    }

    /** Block entities: phase 3's next step. */
    @Inject(method = "getBlockEntity", at = @At("HEAD"), cancellable = true)
    private void horizon$blockEntity(BlockPos pos, CallbackInfoReturnable<BlockEntity> cir) {
        if (horizon$mirror()) cir.setReturnValue(null);
    }

    @Inject(method = "isLoaded", at = @At("HEAD"), cancellable = true)
    private void horizon$isLoaded(BlockPos pos, CallbackInfoReturnable<Boolean> cir) {
        if (horizon$mirror()) cir.setReturnValue(!((Level) (Object) this).isOutsideBuildHeight(pos));
    }

    /** A mirror level has no level data: spawn is the origin. */
    @Inject(method = "getSharedSpawnPos", at = @At("HEAD"), cancellable = true)
    private void horizon$spawn(CallbackInfoReturnable<BlockPos> cir) {
        if (horizon$mirror()) cir.setReturnValue(BlockPos.ZERO);
    }

    @Inject(method = "getSharedSpawnAngle", at = @At("HEAD"), cancellable = true)
    private void horizon$spawnAngle(CallbackInfoReturnable<Float> cir) {
        if (horizon$mirror()) cir.setReturnValue(0.0f);
    }

    /** Vanilla's built-in registries (the data-driven ones as 1.20.1's defaults). */
    @Inject(method = "registryAccess", at = @At("HEAD"), cancellable = true)
    private void horizon$registries(CallbackInfoReturnable<RegistryAccess> cir) {
        if (horizon$mirror()) cir.setReturnValue(Mirror120.registries());
    }
}

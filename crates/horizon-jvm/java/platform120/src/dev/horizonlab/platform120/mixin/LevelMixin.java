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
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
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

    /** Block entities are Horizon's too: loaded from its NBT, one object per call. */
    @Inject(method = "getBlockEntity", at = @At("HEAD"), cancellable = true)
    private void horizon$blockEntity(BlockPos pos, CallbackInfoReturnable<BlockEntity> cir) {
        if (horizon$mirror()) cir.setReturnValue(Mirror120.blockEntity((Level) (Object) this, pos));
    }

    /**
     * BlockEntity.setChanged: vanilla marks the chunk unsaved, and a mirror level's chunk
     * source holds no chunks. What a mod changed goes to Horizon when the call ends
     * (Mirror120.finish), so there is nothing to do here.
     */
    @Inject(method = "blockEntityChanged", at = @At("HEAD"), cancellable = true)
    private void horizon$blockEntityChanged(BlockPos pos, CallbackInfo ci) {
        if (horizon$mirror()) ci.cancel();
    }

    /**
     * Level.setBlockEntity: a mod adding one straight to the level (rather than by placing its
     * block). Kept for this call and sent to Horizon with the rest.
     */
    @Inject(method = "setBlockEntity", at = @At("HEAD"), cancellable = true)
    private void horizon$setBlockEntity(BlockEntity entity, CallbackInfo ci) {
        if (horizon$mirror()) {
            Mirror120.addBlockEntity((Level) (Object) this, entity);
            ci.cancel();
        }
    }

    /**
     * The shape update a neighbour gets when a block changes, and the updates around a block
     * bar one side: both go through Level's own neighbour updater, which a mirror level never
     * made. Neighbour updates are Horizon's to run, as the rest of them are.
     *
     * <p>Wired Redstone placing a cable reaches the first of these through LibMultipart, so
     * without it a cable could not be placed at all.
     */
    @Inject(method = "neighborShapeChanged(Lnet/minecraft/core/Direction;Lnet/minecraft/world/level/block/state/BlockState;Lnet/minecraft/core/BlockPos;Lnet/minecraft/core/BlockPos;II)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$shapeChanged(net.minecraft.core.Direction side,
            net.minecraft.world.level.block.state.BlockState state, BlockPos pos, BlockPos from,
            int flags, int depth, CallbackInfo ci) {
        if (horizon$mirror()) ci.cancel();
    }

    @Inject(method = "updateNeighborsAtExceptFromFacing(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/Block;Lnet/minecraft/core/Direction;)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$neighboursExcept(BlockPos pos,
            net.minecraft.world.level.block.Block block, net.minecraft.core.Direction side,
            CallbackInfo ci) {
        if (horizon$mirror()) ci.cancel();
    }

    /**
     * The comparators beside a block entity that changed: a neighbour update, which is
     * Horizon's to run (as with the rest of them in ServerLevelMixin).
     */
    @Inject(method = "updateNeighbourForOutputSignal", at = @At("HEAD"), cancellable = true)
    private void horizon$outputSignal(BlockPos pos, net.minecraft.world.level.block.Block block,
            CallbackInfo ci) {
        if (horizon$mirror()) ci.cancel();
    }

    /** Vanilla removes it from the chunk; here, from this call's own set. */
    @Inject(method = "removeBlockEntity", at = @At("HEAD"), cancellable = true)
    private void horizon$removeBlockEntity(BlockPos pos, CallbackInfo ci) {
        if (horizon$mirror()) {
            Mirror120.removeBlockEntity((Level) (Object) this, pos);
            ci.cancel();
        }
    }

    /** Heightmaps are Horizon's (a mod's feature placed on the surface asks them). */
    @Inject(method = "getHeight(Lnet/minecraft/world/level/levelgen/Heightmap$Types;II)I", at = @At("HEAD"), cancellable = true)
    private void horizon$height(net.minecraft.world.level.levelgen.Heightmap.Types type, int x, int z,
            CallbackInfoReturnable<Integer> cir) {
        if (horizon$mirror()) cir.setReturnValue(Mirror120.height((Level) (Object) this, type, x, z));
    }

    /** Biomes are Horizon's: a mirror level's biome manager asks it (Biomes120). */
    @Inject(method = "getBiomeManager", at = @At("HEAD"), cancellable = true)
    private void horizon$biomes(CallbackInfoReturnable<net.minecraft.world.level.biome.BiomeManager> cir) {
        if (horizon$mirror()) cir.setReturnValue(Mirror120.biomeManager((Level) (Object) this));
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

    /**
     * A mirror level has no profiler: its constructor never ran, so the supplier Level.getProfiler
     * reads is null. Vanilla counts work through it on paths that have nothing to do with
     * profiling -- Level.getEntities starts with {@code getProfiler().incrementCounter(...)} --
     * so a block item placed through the mirror (BlockItem.place -> isUnobstructed ->
     * getEntities) threw before it could place. Answered with the profiler that records nothing.
     */
    @Inject(method = "getProfiler", at = @At("HEAD"), cancellable = true)
    private void horizon$profiler(
            CallbackInfoReturnable<net.minecraft.util.profiling.ProfilerFiller> cir) {
        if (horizon$mirror()) {
            cir.setReturnValue(dev.horizonlab.platform120.NoProfiler.INSTANCE);
        }
    }

    /** Vanilla's built-in registries (the data-driven ones as 1.20.1's defaults). */
    @Inject(method = "registryAccess", at = @At("HEAD"), cancellable = true)
    private void horizon$registries(CallbackInfoReturnable<RegistryAccess> cir) {
        if (horizon$mirror()) cir.setReturnValue(Mirror120.registries());
    }
}

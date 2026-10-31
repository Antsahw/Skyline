package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonFunctions;
import dev.horizonlab.platform.HorizonServer;
import net.minecraft.server.level.ServerLevel;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A mirrored level's neighbour updates and forced chunks. Blocks a mod sets go to Horizon,
 * which updates their neighbours as it sets them; the mirror has no neighbour updater of its
 * own (a structure placed by a data pack's /place template asks it: nothing to do here). A
 * forced chunk is Horizon's to load (HorizonFunctions.forced), not vanilla's at once.
 */
@Mixin(ServerLevel.class)
public abstract class MirrorUpdatesMixin {
    @Inject(method = {
            "updateNeighborsAt(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/Block;)V",
            "updateNeighborsAt(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/Block;Lnet/minecraft/world/level/redstone/Orientation;)V",
            "updateNeighborsAtExceptFromFacing",
            "neighborChanged(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/Block;Lnet/minecraft/world/level/redstone/Orientation;)V",
            "neighborChanged(Lnet/minecraft/world/level/block/state/BlockState;Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/Block;Lnet/minecraft/world/level/redstone/Orientation;Z)V"},
            at = @At("HEAD"), cancellable = true)
    private void horizon$horizonUpdates(CallbackInfo ci) {
        if (HorizonServer.isMirror(this)) ci.cancel();
    }

    /** ServerLevel.setChunkForced: the ticket, without vanilla's getChunk (Horizon loads it). */
    @Inject(method = "setChunkForced", at = @At("HEAD"), cancellable = true)
    private void horizon$forced(int x, int z, boolean forced, CallbackInfoReturnable<Boolean> cir) {
        if (!HorizonServer.isMirror(this)) return;
        cir.setReturnValue(((ServerLevel) (Object) this).getChunkSource().updateChunkForced(new net.minecraft.world.level.ChunkPos(x, z), forced));
    }
}

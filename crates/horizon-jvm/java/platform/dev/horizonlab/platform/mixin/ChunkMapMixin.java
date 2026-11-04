package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonServer;
import java.util.function.Consumer;
import net.minecraft.server.level.ChunkMap;
import net.minecraft.world.level.chunk.LevelChunk;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * A mirrored level's chunk map (HorizonServer.chunkMap) holds no chunks: Horizon random-ticks
 * its own. Mods that walk the ticking chunks themselves (Serene Seasons' snow and ice, through
 * forEachBlockTickingChunk) walk none.
 */
@Mixin(ChunkMap.class)
public abstract class ChunkMapMixin {
    @Inject(method = "forEachBlockTickingChunk", at = @At("HEAD"), cancellable = true)
    private void horizon$noChunks(Consumer<LevelChunk> action, CallbackInfo ci) {
        if (HorizonServer.isMirrorChunkMap(this)) ci.cancel();
    }

    /** A chunk's save (a pregenerator's "is it generated?"): Horizon's region file, read-only. */
    @Inject(method = "readChunk", at = @At("HEAD"), cancellable = true)
    private void horizon$readChunk(net.minecraft.world.level.ChunkPos pos,
                                   org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable<
                                           java.util.concurrent.CompletableFuture<java.util.Optional<net.minecraft.nbt.CompoundTag>>> cir) {
        if (HorizonServer.isMirrorChunkMap(this)) cir.setReturnValue(dev.horizonlab.platform.HorizonChunkLoads.read(
                (net.minecraft.server.level.ServerLevel) horizon$level(), pos));
    }

    @org.spongepowered.asm.mixin.Shadow @org.spongepowered.asm.mixin.Final
    net.minecraft.server.level.ServerLevel level;

    private net.minecraft.server.level.ServerLevel horizon$level() {
        return level;
    }
}

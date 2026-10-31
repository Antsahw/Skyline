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
}

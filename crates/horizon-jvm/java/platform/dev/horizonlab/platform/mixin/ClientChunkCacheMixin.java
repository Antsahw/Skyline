package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonChunks;
import dev.horizonlab.platform.HorizonServer;
import net.minecraft.client.multiplayer.ClientChunkCache;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.chunk.status.ChunkStatus;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A headless client level's chunk cache holds nothing (no chunk packets come): its chunks are
 * Horizon's (HorizonChunks' copies for that level), as the level's own getChunk gives them
 * (LevelMixin). Mods read through the cache: Diorama's map.
 */
@Mixin(ClientChunkCache.class)
public abstract class ClientChunkCacheMixin {
    @Shadow
    @Final
    ClientLevel level;

    @Inject(method = "getChunk(IILnet/minecraft/world/level/chunk/status/ChunkStatus;Z)Lnet/minecraft/world/level/chunk/LevelChunk;",
            at = @At("HEAD"), cancellable = true)
    private void horizon$chunk(int x, int z, ChunkStatus status, boolean load, CallbackInfoReturnable<LevelChunk> cir) {
        if (level == null || !HorizonServer.isMirror(level)) return;
        if (HorizonServer.serverSide(level) instanceof ServerLevel server) {
            cir.setReturnValue(HorizonChunks.chunk(server, x, z, level));
        }
    }
}

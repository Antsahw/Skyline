package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonServer;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;
import net.minecraft.server.level.ChunkMap;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.chunk.LevelChunk;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A mirrored level's chunk map (HorizonServer.chunkMap) holds no chunks: Horizon random-ticks
 * its own. Mods that walk the ticking chunks themselves (Serene Seasons' snow and ice, through
 * forEachBlockTickingChunk) walk none. It has no player map either (made without its
 * constructor): the players who see a chunk are the level's mirrored players within the
 * server's view distance of it (HorizonServer.tracking), as Fabric's PlayerLookup.tracking
 * answers. TACZ sends its guns' sounds to getPlayers' players (it threw on the null map).
 */
@Mixin(ChunkMap.class)
public abstract class ChunkMapMixin {
    @Shadow
    @Final
    ServerLevel level;

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

    private net.minecraft.server.level.ServerLevel horizon$level() {
        return level;
    }

    @Inject(method = "getPlayers", at = @At("HEAD"), cancellable = true)
    private void horizon$players(ChunkPos pos, boolean onlyOnEdge, CallbackInfoReturnable<List<ServerPlayer>> cir) {
        if (!HorizonServer.isMirrorChunkMap(this)) return;
        // (The players at the view distance's edge: those a chunk is about to be sent to.
        // Horizon sends chunks itself.)
        cir.setReturnValue(onlyOnEdge ? new ArrayList<>()
                : new ArrayList<>(HorizonServer.tracking(level, pos.getMiddleBlockX(), pos.getMiddleBlockZ(), null)));
    }

    @Inject(method = "isChunkTracked", at = @At("HEAD"), cancellable = true)
    private void horizon$tracked(ServerPlayer player, int x, int z, CallbackInfoReturnable<Boolean> cir) {
        if (!HorizonServer.isMirrorChunkMap(this)) return;
        cir.setReturnValue(HorizonServer.tracking(level, x * 16 + 8, z * 16 + 8, null).contains(player));
    }
}

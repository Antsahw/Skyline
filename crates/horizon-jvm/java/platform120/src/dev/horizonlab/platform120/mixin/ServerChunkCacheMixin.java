package dev.horizonlab.platform120.mixin;

import net.minecraft.server.level.ServerChunkCache;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.LevelChunk;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A mirror level's chunk source holds no chunks: the blocks are Horizon's, read one at a time
 * (Mirror120), and this source exists only for the saved data a mod keeps on the level.
 *
 * <p>It is made without its constructor, so its chunk map is null. Vanilla and mods ask it
 * plenty of questions on the way through code that has nothing to do with chunks
 * ({@code BlockEntity.setChanged} asks {@code hasChunkAt} twice), and every one of those would
 * be a NullPointerException. Answered here instead: no chunk is loaded, and none can be had.
 * Callers then take their "not loaded" path, which is the one that leaves the world to Horizon.
 */
@Mixin(ServerChunkCache.class)
public abstract class ServerChunkCacheMixin {
    @org.spongepowered.asm.mixin.Shadow
    @org.spongepowered.asm.mixin.Final
    net.minecraft.server.level.ServerLevel level;

    @Inject(method = "hasChunk", at = @At("HEAD"), cancellable = true)
    private void horizon$hasChunk(int x, int z, CallbackInfoReturnable<Boolean> cir) {
        cir.setReturnValue(false);
    }

    @Inject(method = "getChunkNow", at = @At("HEAD"), cancellable = true)
    private void horizon$chunkNow(int x, int z, CallbackInfoReturnable<LevelChunk> cir) {
        cir.setReturnValue(null);
    }

    @Inject(method = "getChunk(IILnet/minecraft/world/level/chunk/ChunkStatus;Z)Lnet/minecraft/world/level/chunk/ChunkAccess;",
            at = @At("HEAD"), cancellable = true)
    private void horizon$chunk(int x, int z, net.minecraft.world.level.chunk.ChunkStatus status,
            boolean require, CallbackInfoReturnable<ChunkAccess> cir) {
        // Code that must have a chunk (a disk feature marks the block above for post-processing
        // on it) gets an empty one, whose marks and writes go nowhere, rather than vanilla's
        // "Should always be able to create a chunk!" ending the feature halfway: its blocks
        // are written through the level, which is Horizon's world.
        if (require && level != null) {
            var plains = level.registryAccess().registryOrThrow(net.minecraft.core.registries.Registries.BIOME)
                    .getHolderOrThrow(net.minecraft.world.level.biome.Biomes.PLAINS);
            cir.setReturnValue(new net.minecraft.world.level.chunk.EmptyLevelChunk(level,
                    new net.minecraft.world.level.ChunkPos(x, z), plains));
            return;
        }
        cir.setReturnValue(null);
    }

    /**
     * Packets about an entity to the players tracking it (a mob hurt, a mob's event): a mirror
     * level tracks nobody; Horizon draws the mods' entities from their snapshots (Entities120).
     */
    @Inject(method = {"broadcast", "broadcastAndSend"}, at = @At("HEAD"), cancellable = true)
    private void horizon$broadcast(CallbackInfo ci) {
        ci.cancel();
    }

    /** A mirror level tracks no entity: one added or removed changes nothing here. */
    @Inject(method = {"addEntity", "removeEntity"}, at = @At("HEAD"), cancellable = true)
    private void horizon$track(CallbackInfo ci) {
        ci.cancel();
    }

    /**
     * Chunk tickets (a teleport loads the chunk it lands in: Waystones): the mirror loads no
     * chunks, so there is nothing to keep loaded. Its distance manager was never made.
     */
    @Inject(method = {"addRegionTicket(Lnet/minecraft/server/level/TicketType;Lnet/minecraft/world/level/ChunkPos;ILjava/lang/Object;)V",
            "removeRegionTicket(Lnet/minecraft/server/level/TicketType;Lnet/minecraft/world/level/ChunkPos;ILjava/lang/Object;)V"},
            at = @At("HEAD"), cancellable = true)
    private void horizon$noTickets(org.spongepowered.asm.mixin.injection.callback.CallbackInfo ci) {
        ci.cancel();
    }
}

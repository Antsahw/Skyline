package dev.horizonlab.platform120.mixin;

import net.minecraft.server.level.ChunkHolder;
import net.minecraft.world.level.chunk.LevelChunk;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * The one chunk holder a mirror level's chunk storage hands out for every position
 * (Mirror120.level). It holds no chunk, and says so.
 *
 * <p>It exists because mods ask for a holder and then use what comes back without checking:
 * LibNetworkStack asks who is watching a chunk and calls {@code getTickingChunk()} straight on
 * the answer, so an absent holder threw inside the mod. A holder with no chunk sends them down
 * their own empty path instead, which is the truth -- Horizon has the chunks and the players.
 *
 * <p>Made without its constructor, so every field in it is null; answering here rather than
 * letting vanilla's own code read them is what keeps that from showing.
 */
@Mixin(ChunkHolder.class)
public abstract class ChunkHolderMixin {
    @Inject(method = "getTickingChunk", at = @At("HEAD"), cancellable = true)
    private void horizon$noTickingChunk(CallbackInfoReturnable<LevelChunk> cir) {
        cir.setReturnValue(null);
    }

    @Inject(method = "getFullChunk", at = @At("HEAD"), cancellable = true)
    private void horizon$noFullChunk(CallbackInfoReturnable<LevelChunk> cir) {
        cir.setReturnValue(null);
    }
}

package dev.horizonlab.platform120.mixin;

import net.minecraft.server.level.ChunkHolder;
import net.minecraft.server.level.ChunkMap;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/**
 * A mirror level's chunk storage, holding no chunks (Mirror120.level). It exists because mods
 * ask it who is watching a chunk before sending a block entity's network update; with empty
 * maps every such lookup finds nothing, which is the truthful answer here -- Horizon, not this
 * runtime, has the players.
 */
@Mixin(ChunkMap.class)
public interface ChunkMapAccess {
    @Mutable
    @Accessor("visibleChunkMap")
    void horizon$setVisibleChunkMap(
            it.unimi.dsi.fastutil.longs.Long2ObjectLinkedOpenHashMap<ChunkHolder> map);

    @Mutable
    @Accessor("updatingChunkMap")
    void horizon$setUpdatingChunkMap(
            it.unimi.dsi.fastutil.longs.Long2ObjectLinkedOpenHashMap<ChunkHolder> map);
}

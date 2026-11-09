package dev.horizonlab.platform120.mixin;

import net.minecraft.server.level.ServerChunkCache;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.storage.DimensionDataStorage;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/** A mirror level's chunk source: its level and its saved data, no chunks (Mirror120.level). */
@Mixin(ServerChunkCache.class)
public interface ServerChunkCacheAccess {
    @Mutable
    @Accessor("level")
    void horizon$setLevel(ServerLevel level);

    @Mutable
    @Accessor("dataStorage")
    void horizon$setDataStorage(DimensionDataStorage storage);
}

package dev.horizonlab.platform120.mixin;

import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(ServerLevel.class)
public interface ServerLevelAccess {
    @Mutable
    @Accessor("server")
    void horizon$setServer(MinecraftServer server);

    @Mutable
    @Accessor("serverLevelData")
    void horizon$setServerLevelData(net.minecraft.world.level.storage.ServerLevelData data);

    @Mutable
    @Accessor("players")
    void horizon$setPlayers(java.util.List<net.minecraft.server.level.ServerPlayer> value);

    @Mutable
    @Accessor("chunkSource")
    void horizon$setChunkSource(net.minecraft.server.level.ServerChunkCache value);

    @Mutable
    @Accessor("navigatingMobs")
    void horizon$setNavigatingMobs(java.util.Set<net.minecraft.world.entity.Mob> value);

    @Mutable
    @Accessor("blockEvents")
    void horizon$setBlockEvents(it.unimi.dsi.fastutil.objects.ObjectLinkedOpenHashSet<net.minecraft.world.level.BlockEventData> value);

    @Mutable
    @Accessor("blockEventsToReschedule")
    void horizon$setBlockEventsToReschedule(java.util.List<net.minecraft.world.level.BlockEventData> value);

    @Mutable
    @Accessor("customSpawners")
    void horizon$setCustomSpawners(java.util.List<net.minecraft.world.level.CustomSpawner> value);

    @Mutable
    @Accessor("dragonParts")
    void horizon$setDragonParts(it.unimi.dsi.fastutil.ints.Int2ObjectMap<net.minecraft.world.entity.boss.EnderDragonPart> value);

    @Mutable
    @Accessor("sleepStatus")
    void horizon$setSleepStatus(net.minecraft.server.players.SleepStatus value);

    @Mutable
    @Accessor("blockTicks")
    void horizon$setBlockTicks(net.minecraft.world.ticks.LevelTicks<net.minecraft.world.level.block.Block> value);

    @Mutable
    @Accessor("fluidTicks")
    void horizon$setFluidTicks(net.minecraft.world.ticks.LevelTicks<net.minecraft.world.level.material.Fluid> value);

    @Mutable
    @Accessor("randomSequences")
    void horizon$setRandomSequences(net.minecraft.world.RandomSequences value);
}

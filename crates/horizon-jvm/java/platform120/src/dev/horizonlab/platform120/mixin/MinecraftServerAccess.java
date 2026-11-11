package dev.horizonlab.platform120.mixin;

import net.minecraft.server.MinecraftServer;
import net.minecraft.server.players.PlayerList;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(MinecraftServer.class)
public interface MinecraftServerAccess {
    @Accessor("playerList")
    void horizon$setPlayerList(PlayerList list);

    @org.spongepowered.asm.mixin.Mutable
    @Accessor("worldData")
    void horizon$setWorldData(net.minecraft.world.level.storage.WorldData data);

    @org.spongepowered.asm.mixin.Mutable
    @Accessor("scoreboard")
    void horizon$setScoreboard(net.minecraft.server.ServerScoreboard scoreboard);

    /** The server's levels (overworld(), getLevel, getAllLevels): the mirror's, as they are made. */
    @Mutable
    @Accessor("levels")
    void horizon$setLevels(java.util.Map<net.minecraft.resources.ResourceKey<net.minecraft.world.level.Level>, net.minecraft.server.level.ServerLevel> levels);

    /** The server's tick count (getTickCount), which mods time periodic work by: Horizon's. */
    @Accessor("tickCount")
    void horizon$setTickCount(int ticks);

    /** Where the server keeps its files (getWorldPath): the runtime's own folder. */
    @Mutable
    @Accessor("storageSource")
    void horizon$setStorageSource(net.minecraft.world.level.storage.LevelStorageSource.LevelStorageAccess storage);
}

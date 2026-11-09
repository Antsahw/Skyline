package dev.horizonlab.platform120.mixin;

import net.minecraft.server.MinecraftServer;
import net.minecraft.server.players.PlayerList;
import org.spongepowered.asm.mixin.Mixin;
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
}

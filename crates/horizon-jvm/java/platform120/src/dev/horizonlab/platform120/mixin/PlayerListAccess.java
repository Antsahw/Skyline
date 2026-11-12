package dev.horizonlab.platform120.mixin;

import java.util.List;
import java.util.Map;
import java.util.UUID;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.players.PlayerList;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(PlayerList.class)
public interface PlayerListAccess {
    @Mutable
    @Accessor("server")
    void horizon$setServer(MinecraftServer server);

    /** The players a mod finds on the server (getPlayers, getPlayer): Horizon's, as they act. */
    @Mutable
    @Accessor("players")
    void horizon$setPlayers(List<ServerPlayer> players);

    @Mutable
    @Accessor("playersByUUID")
    void horizon$setPlayersByUUID(Map<UUID, ServerPlayer> players);

    /** The server's operators, bans and whitelist: empty, as a new server's (nobody is an op). */
    @Mutable
    @Accessor("ops")
    void horizon$setOps(net.minecraft.server.players.ServerOpList ops);

    @Mutable
    @Accessor("bans")
    void horizon$setBans(net.minecraft.server.players.UserBanList bans);

    @Mutable
    @Accessor("ipBans")
    void horizon$setIpBans(net.minecraft.server.players.IpBanList bans);

    @Mutable
    @Accessor("whitelist")
    void horizon$setWhitelist(net.minecraft.server.players.UserWhiteList whitelist);
}

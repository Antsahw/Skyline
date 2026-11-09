package dev.horizonlab.platform120.mixin;

import java.util.HashMap;
import java.util.Map;
import java.util.UUID;
import net.minecraft.server.PlayerAdvancements;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.players.PlayerList;
import net.minecraft.stats.ServerStatsCounter;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A mirror player's statistics and advancements: kept in memory (Horizon has the player's
 * real ones). An advancement made without its constructor listens to no trigger, so vanilla's
 * criteria triggers pass over mirror players. Static: the player list is made without its
 * constructor (Mirror120), so instance fields would never be set; there is one.
 */
@Mixin(PlayerList.class)
public abstract class PlayerListMixin {
    @Shadow
    @org.spongepowered.asm.mixin.Final
    private net.minecraft.server.MinecraftServer server;

    @Unique
    private static final Map<UUID, ServerStatsCounter> horizon$stats = new HashMap<>();

    @Unique
    private static final Map<UUID, PlayerAdvancements> horizon$advancements = new HashMap<>();

    @Inject(method = "getPlayerStats", at = @At("HEAD"), cancellable = true)
    private void horizon$stats(Player player, CallbackInfoReturnable<ServerStatsCounter> cir) {
        cir.setReturnValue(horizon$stats.computeIfAbsent(player.getUUID(),
                id -> new ServerStatsCounter(server, new java.io.File("horizon-no-stats-" + id + ".json"))));
    }

    @Inject(method = "getPlayerAdvancements", at = @At("HEAD"), cancellable = true)
    private void horizon$advancements(ServerPlayer player, CallbackInfoReturnable<PlayerAdvancements> cir) {
        cir.setReturnValue(horizon$advancements.computeIfAbsent(player.getUUID(),
                id -> dev.horizonlab.platform120.Allocate.make(PlayerAdvancements.class)));
    }
}

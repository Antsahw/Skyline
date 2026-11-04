package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonClient;
import dev.horizonlab.platform.HorizonServer;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.entity.LevelEntityGetter;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * The headless client's levels (HorizonClient.clientLevel) hold no entities of their own: they
 * see the mirror's (its players, mods' entities), as a client sees what the server tracks.
 */
@Mixin(ClientLevel.class)
public abstract class ClientLevelMixin {
    @Inject(method = "getEntities()Lnet/minecraft/world/level/entity/LevelEntityGetter;", at = @At("HEAD"), cancellable = true)
    private void horizon$entities(CallbackInfoReturnable<LevelEntityGetter<Entity>> cir) {
        if (HorizonServer.isMirror(this)) cir.setReturnValue(HorizonClient.entities((ClientLevel) (Object) this));
    }

    // What a client level asks its connection for: the mirror's level answers (one process).

    @Inject(method = "enabledFeatures", at = @At("HEAD"), cancellable = true)
    private void horizon$features(CallbackInfoReturnable<net.minecraft.world.flag.FeatureFlagSet> cir) {
        if (HorizonServer.isMirror(this)) cir.setReturnValue(horizon$server().enabledFeatures());
    }

    @Inject(method = "recipeAccess", at = @At("HEAD"), cancellable = true)
    private void horizon$recipes(CallbackInfoReturnable<net.minecraft.world.item.crafting.RecipeAccess> cir) {
        if (HorizonServer.isMirror(this)) cir.setReturnValue(horizon$server().recipeAccess());
    }

    @Inject(method = "getScoreboard", at = @At("HEAD"), cancellable = true)
    private void horizon$scoreboard(CallbackInfoReturnable<net.minecraft.world.scores.Scoreboard> cir) {
        if (HorizonServer.isMirror(this)) cir.setReturnValue(horizon$server().getScoreboard());
    }

    /**
     * Whether the client has a chunk: whether Horizon has it (its chunk source, a client chunk
     * cache, has none of Horizon's). VoxelMap maps only chunks it has.
     */
    @Inject(method = "hasChunk", at = @At("HEAD"), cancellable = true)
    private void horizon$hasChunk(int x, int z, CallbackInfoReturnable<Boolean> cir) {
        if (HorizonServer.isMirror(this) && horizon$server() instanceof net.minecraft.server.level.ServerLevel server) {
            cir.setReturnValue(dev.horizonlab.platform.HorizonChunks.has(server, x, z, (ClientLevel) (Object) this));
        }
    }

    /** A packet a mod's client code sends through the level: no connection to take it. */
    @Inject(method = "sendPacketToServer", at = @At("HEAD"), cancellable = true)
    private void horizon$send(net.minecraft.network.protocol.Packet<?> packet, org.spongepowered.asm.mixin.injection.callback.CallbackInfo ci) {
        if (HorizonServer.isMirror(this)) ci.cancel();
    }

    /**
     * A particle a mod's client code adds (Create's belt connector between its shafts): the
     * headless client has no particle engine (vanilla's createParticle on null failed the mod's
     * whole client tick), so it goes to the client's player as the packet a server would send
     * (count 0: one particle, its speed the given one), which Horizon's game draws.
     */
    @Inject(method = "doAddParticle", at = @At("HEAD"), cancellable = true)
    private void horizon$particle(net.minecraft.core.particles.ParticleOptions options, boolean force, boolean always,
                                  double x, double y, double z, double xd, double yd, double zd,
                                  org.spongepowered.asm.mixin.injection.callback.CallbackInfo ci) {
        var mc = net.minecraft.client.Minecraft.getInstance();
        if (!HorizonServer.isMirror(this) || mc.particleEngine != null) return;
        ci.cancel();
        if (mc.player == null) return;
        var player = HorizonServer.player(mc.player.getUUID());
        if (player == null) return;
        dev.horizonlab.platform.HorizonPackets.forward(player, new net.minecraft.network.protocol.game.ClientboundLevelParticlesPacket(
                options, force, always, x, y, z, (float) xd, (float) yd, (float) zd, 1.0F, 0));
    }

    private net.minecraft.world.level.Level horizon$server() {
        return HorizonServer.serverSide((ClientLevel) (Object) this);
    }
}

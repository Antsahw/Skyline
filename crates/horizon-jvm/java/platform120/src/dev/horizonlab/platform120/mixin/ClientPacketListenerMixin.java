package dev.horizonlab.platform120.mixin;

import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.network.protocol.Packet;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * What the headless client sends: there is no server at the end of its connection, only the
 * server half in this same runtime, which a mod's own payload goes to (Packets120). Vanilla's
 * packets (the client's moves, its clicks) are Horizon's own business and go nowhere.
 */
@Mixin(ClientPacketListener.class)
public abstract class ClientPacketListenerMixin {
    @Inject(method = "send(Lnet/minecraft/network/protocol/Packet;)V", at = @At("HEAD"), cancellable = true)
    private void horizon$send(Packet<?> packet, CallbackInfo ci) {
        dev.horizonlab.platform120.Packets120.fromClient(packet);
        ci.cancel();
    }

    @org.spongepowered.asm.mixin.Shadow
    @org.spongepowered.asm.mixin.Final
    private java.util.Map<java.util.UUID, net.minecraft.client.multiplayer.PlayerInfo> playerInfoMap;

    /**
     * Who is on the tab list: nobody the headless client knows of (its connection was never
     * made, so it has no list). A renderer asks whether the client's player is a spectator
     * through it, for whether a mob glows.
     */
    @Inject(method = {"getPlayerInfo(Ljava/util/UUID;)Lnet/minecraft/client/multiplayer/PlayerInfo;",
            "getPlayerInfo(Ljava/lang/String;)Lnet/minecraft/client/multiplayer/PlayerInfo;"},
            at = @At("HEAD"), cancellable = true)
    private void horizon$noList(CallbackInfoReturnable<net.minecraft.client.multiplayer.PlayerInfo> cir) {
        if (playerInfoMap == null) cir.setReturnValue(null);
    }
}

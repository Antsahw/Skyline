package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonServer;
import net.minecraft.network.Connection;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A mirrored player's network connection has no channel (Horizon holds the real one): it is
 * connected, not connecting (Open Parties and Claims handles a player's login only on a
 * connection that is). The headless client's own connection is its memory connection to the
 * server in this process, as single player's is: connected (mods keep what the server sent
 * for the connection they are on), its address "local"; what is sent on it raw goes nowhere
 * (the packet listener's sends go to the mods' server code: ClientCommonPacketListenerMixin).
 */
@Mixin(Connection.class)
public abstract class MirrorConnectionMixin {
    @Inject(method = "isConnecting", at = @At("HEAD"), cancellable = true)
    private void horizon$connected(CallbackInfoReturnable<Boolean> cir) {
        if (HorizonServer.isMirror(this)) cir.setReturnValue(false);
    }

    @Inject(method = "isConnected", at = @At("HEAD"), cancellable = true)
    private void horizon$clientConnected(CallbackInfoReturnable<Boolean> cir) {
        if (HorizonServer.isClientConnection(this)) cir.setReturnValue(true);
    }

    @org.spongepowered.asm.mixin.Shadow
    @org.spongepowered.asm.mixin.Final
    private java.util.Queue<?> pendingActions;

    @Inject(method = {"flushChannel", "runOnceConnected", "tick"}, at = @At("HEAD"), cancellable = true)
    private void horizon$noChannel(CallbackInfo ci) {
        if (HorizonServer.isClientConnection(this) || pendingActions == null) ci.cancel();
    }

    /**
     * A packet sent raw on the client's connection, or on any connection the bridge made
     * without its constructor (no channel, no queue: FancyMenu's handshake failed on its null
     * pendingActions): a mod's custom payload from the client goes to the mods' server code
     * (HorizonLoop), as the packet listener's sends do; the rest goes nowhere.
     */
    @Inject(method = "send(Lnet/minecraft/network/protocol/Packet;Lio/netty/channel/ChannelFutureListener;Z)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$send(net.minecraft.network.protocol.Packet<?> packet, io.netty.channel.ChannelFutureListener listener,
                              boolean flush, CallbackInfo ci) {
        boolean client = HorizonServer.isClientConnection(this);
        if (!client && pendingActions != null) return;
        if (!client && HorizonServer.isMirror(this)) return;
        ci.cancel();
        if (dev.horizonlab.platform.HorizonClient.headless()
                && packet instanceof net.minecraft.network.protocol.common.ServerboundCustomPayloadPacket(var payload)) {
            dev.horizonlab.platform.HorizonLoop.toServer(payload);
        }
    }
}

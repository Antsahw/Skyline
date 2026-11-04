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

    @Inject(method = {"send(Lnet/minecraft/network/protocol/Packet;Lio/netty/channel/ChannelFutureListener;Z)V", "flushChannel",
            "runOnceConnected", "tick"}, at = @At("HEAD"), cancellable = true)
    private void horizon$noChannel(CallbackInfo ci) {
        if (HorizonServer.isClientConnection(this)) ci.cancel();
    }
}

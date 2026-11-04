package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonLoop;
import dev.horizonlab.platform.HorizonPacketTap;
import io.netty.channel.ChannelFutureListener;
import io.netty.channel.ChannelHandlerContext;
import net.minecraft.network.Connection;
import net.minecraft.network.PacketListener;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.common.ServerboundCustomPayloadPacket;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The window's own player's client connection (HorizonPacketTap): it reads the packets the
 * server sends that player through vanilla's pipeline, for the mods that read a client's
 * connection, but handles none of them as vanilla's client would (Horizon's game is the
 * player's client), never sends, and never ends on a fault.
 */
@Mixin(Connection.class)
public abstract class TapConnectionMixin {
    @Shadow
    private static <T extends PacketListener> void genericsFtw(Packet<T> packet, PacketListener listener) {
        throw new AssertionError();
    }

    /** channelRead0 hands the packet to the phase's listener of the tap (the mods' injections only). */
    @Redirect(method = "channelRead0", at = @At(value = "INVOKE",
            target = "Lnet/minecraft/network/Connection;genericsFtw(Lnet/minecraft/network/protocol/Packet;Lnet/minecraft/network/PacketListener;)V"))
    private void horizon$tapListener(Packet<?> packet, PacketListener listener) {
        genericsFtw(packet, HorizonPacketTap.tapped(this) ? HorizonPacketTap.listener(listener) : listener);
    }

    /** What the client sends on it: the player's game sends; a mod's payload goes to the mods' server code. */
    @Inject(method = "send(Lnet/minecraft/network/protocol/Packet;Lio/netty/channel/ChannelFutureListener;Z)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$tapSend(Packet<?> packet, ChannelFutureListener listener, boolean flush, CallbackInfo ci) {
        if (!HorizonPacketTap.tapped(this)) return;
        ci.cancel();
        if (packet instanceof ServerboundCustomPayloadPacket(var payload)) HorizonLoop.toServer(payload);
    }

    @Inject(method = "exceptionCaught", at = @At("HEAD"), cancellable = true)
    private void horizon$tapFault(ChannelHandlerContext ctx, Throwable cause, CallbackInfo ci) {
        if (!HorizonPacketTap.tapped(this)) return;
        ci.cancel();
        HorizonPacketTap.fault("a packet couldn't be read", cause);
    }
}

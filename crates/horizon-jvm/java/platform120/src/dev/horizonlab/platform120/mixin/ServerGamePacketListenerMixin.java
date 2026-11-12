package dev.horizonlab.platform120.mixin;

import net.minecraft.network.protocol.Packet;
import net.minecraft.server.network.ServerGamePacketListenerImpl;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * A mirror player's packet listener swallows what is sent to it. Horizon owns the real players
 * and tells them about the world itself; a mod sending its own packet here has nobody to send
 * it to, and the listener has no connection behind it, so vanilla's send would throw.
 *
 * <p>A mod's own payload goes to its client half, in this same runtime (Packets120): the
 * headless client's receiver for that channel runs, standing in for this player.
 */
@Mixin(ServerGamePacketListenerImpl.class)
public abstract class ServerGamePacketListenerMixin {
    @org.spongepowered.asm.mixin.Shadow
    public net.minecraft.server.level.ServerPlayer player;

    @Inject(method = "send(Lnet/minecraft/network/protocol/Packet;)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$send(Packet<?> packet, CallbackInfo ci) {
        dev.horizonlab.platform120.Packets120.toClient(this.player, packet);
        ci.cancel();
    }

    @Inject(method = "send(Lnet/minecraft/network/protocol/Packet;Lnet/minecraft/network/PacketSendListener;)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$sendWithListener(Packet<?> packet,
            net.minecraft.network.PacketSendListener listener, CallbackInfo ci) {
        dev.horizonlab.platform120.Packets120.toClient(this.player, packet);
        ci.cancel();
    }
}

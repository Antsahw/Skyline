package dev.horizonlab.platform120.mixin;

import net.minecraft.network.Connection;
import net.minecraft.network.PacketSendListener;
import net.minecraft.network.protocol.Packet;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * What the headless client sends through its connection (Screens120.clientConnection) goes to
 * the server half in this same runtime, as what it sends through its packet listener does
 * (ClientPacketListenerMixin): there is no server at the other end.
 */
@Mixin(Connection.class)
public abstract class ConnectionMixin {
    @Inject(method = "send(Lnet/minecraft/network/protocol/Packet;Lnet/minecraft/network/PacketSendListener;)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$send(Packet<?> packet, PacketSendListener listener, CallbackInfo ci) {
        if ((Object) this != dev.horizonlab.platform120.Screens120.clientConnectionIfMade()) return;
        dev.horizonlab.platform120.Loader120.get().fromClient(packet);
        ci.cancel();
    }
}

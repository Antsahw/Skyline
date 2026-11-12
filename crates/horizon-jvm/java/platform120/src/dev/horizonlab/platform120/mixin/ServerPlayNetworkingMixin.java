package dev.horizonlab.platform120.mixin;

import net.fabricmc.fabric.api.networking.v1.PacketSender;
import net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.network.ServerGamePacketListenerImpl;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Fabric's questions about a player's connection, answered for a mirror player, whose
 * connection Fabric never set up (it keeps its channels and sender on it): the player can
 * receive a channel when the client half in this runtime receives it, and its sender hands
 * packets to that client half (Packets120).
 */
@Mixin(value = ServerPlayNetworking.class, remap = false)
public abstract class ServerPlayNetworkingMixin {
    @Inject(method = "canSend(Lnet/minecraft/server/network/ServerGamePacketListenerImpl;Lnet/minecraft/resources/ResourceLocation;)Z",
            at = @At("HEAD"), cancellable = true)
    private static void horizon$canSend(ServerGamePacketListenerImpl handler, ResourceLocation channel,
            CallbackInfoReturnable<Boolean> cir) {
        cir.setReturnValue(dev.horizonlab.platform120.Packets120.clientReceives(channel));
    }

    @Inject(method = "getSender(Lnet/minecraft/server/network/ServerGamePacketListenerImpl;)Lnet/fabricmc/fabric/api/networking/v1/PacketSender;",
            at = @At("HEAD"), cancellable = true)
    private static void horizon$sender(ServerGamePacketListenerImpl handler, CallbackInfoReturnable<PacketSender> cir) {
        cir.setReturnValue(dev.horizonlab.platform120.Packets120.serverSender(handler.player));
    }
}

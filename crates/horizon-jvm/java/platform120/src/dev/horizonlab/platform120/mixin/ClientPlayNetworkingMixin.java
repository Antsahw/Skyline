package dev.horizonlab.platform120.mixin;

import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.fabricmc.fabric.api.networking.v1.PacketSender;
import net.minecraft.resources.ResourceLocation;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Fabric's questions about the client's connection, answered for the headless client: it can
 * send a channel when the server half in this runtime receives it, and its sender hands
 * packets to that server half (Packets120).
 */
@Mixin(value = ClientPlayNetworking.class, remap = false)
public abstract class ClientPlayNetworkingMixin {
    @Inject(method = "canSend(Lnet/minecraft/resources/ResourceLocation;)Z", at = @At("HEAD"), cancellable = true)
    private static void horizon$canSend(ResourceLocation channel, CallbackInfoReturnable<Boolean> cir) {
        cir.setReturnValue(dev.horizonlab.platform120.Packets120.serverReceives(channel));
    }

    @Inject(method = "getSender()Lnet/fabricmc/fabric/api/networking/v1/PacketSender;", at = @At("HEAD"), cancellable = true)
    private static void horizon$sender(CallbackInfoReturnable<PacketSender> cir) {
        cir.setReturnValue(dev.horizonlab.platform120.Packets120.clientSender(
                dev.horizonlab.platform120.Packets120.standingIn()));
    }
}

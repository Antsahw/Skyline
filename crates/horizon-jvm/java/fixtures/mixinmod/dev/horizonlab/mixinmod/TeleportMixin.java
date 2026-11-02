package dev.horizonlab.mixinmod;

import net.minecraft.network.chat.Component;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.level.portal.TeleportTransition;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A player hears where it was teleported (the end of ServerPlayer.teleport, as GlitchCore). */
@Mixin(ServerPlayer.class)
public abstract class TeleportMixin {
    @Inject(method = "teleport(Lnet/minecraft/world/level/portal/TeleportTransition;)Lnet/minecraft/server/level/ServerPlayer;",
            at = @At("TAIL"))
    private void horizonMixins$teleported(TeleportTransition transition, CallbackInfoReturnable<ServerPlayer> cir) {
        ServerPlayer player = (ServerPlayer) (Object) this;
        player.sendSystemMessage(Component.literal("[mixinmod] teleported to " + player.blockPosition().toShortString()));
    }
}

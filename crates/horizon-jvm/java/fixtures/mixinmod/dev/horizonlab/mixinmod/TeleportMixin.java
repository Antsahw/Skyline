package dev.horizonlab.mixinmod;

import net.minecraft.network.chat.Component;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.level.portal.TeleportTransition;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A player hears where it was teleported (the end of ServerPlayer.teleport, as GlitchCore);
 * nobody may go to x 666 (its head cancels, the player stays).
 */
@Mixin(ServerPlayer.class)
public abstract class TeleportMixin {
    private static final String TELEPORT = "teleport(Lnet/minecraft/world/level/portal/TeleportTransition;)Lnet/minecraft/server/level/ServerPlayer;";

    @Inject(method = TELEPORT, at = @At("HEAD"), cancellable = true)
    private void horizonMixins$refuse(TeleportTransition transition, CallbackInfoReturnable<ServerPlayer> cir) {
        if ((int) Math.floor(transition.position().x) == 666) {
            ServerPlayer player = (ServerPlayer) (Object) this;
            player.sendSystemMessage(Component.literal("[mixinmod] no teleport to 666 in "
                    + transition.newLevel().dimension().identifier()));
            cir.setReturnValue(null);
        }
    }

    @Inject(method = TELEPORT, at = @At("TAIL"))
    private void horizonMixins$teleported(TeleportTransition transition, CallbackInfoReturnable<ServerPlayer> cir) {
        ServerPlayer player = (ServerPlayer) (Object) this;
        player.sendSystemMessage(Component.literal("[mixinmod] teleported to " + player.blockPosition().toShortString()
                + " in " + transition.newLevel().dimension().identifier()));
    }
}

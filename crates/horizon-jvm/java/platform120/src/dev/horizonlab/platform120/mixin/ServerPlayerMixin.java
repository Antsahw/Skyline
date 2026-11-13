package dev.horizonlab.platform120.mixin;

import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * A mirror player: Horizon places it (no spawn search on a server's settings), and what the mods
 * tell it goes to Horizon's player.
 */
@Mixin(ServerPlayer.class)
public abstract class ServerPlayerMixin {
    @Inject(method = "fudgeSpawnLocation", at = @At("HEAD"), cancellable = true)
    private void horizon$noSpawnSearch(ServerLevel level, CallbackInfo ci) {
        ci.cancel();
    }

    /**
     * A message to the player, in chat or over the hotbar (`sendSystemMessage`: a command's
     * answer, a mod's "waystone activated", `displayClientMessage` on the server): to Horizon's
     * player, whose game shows it. A mirror player has no connection to send it on.
     */
    @Inject(method = "sendSystemMessage(Lnet/minecraft/network/chat/Component;Z)V", at = @At("HEAD"), cancellable = true)
    private void horizon$message(net.minecraft.network.chat.Component message, boolean overlay, CallbackInfo ci) {
        dev.horizonlab.platform120.Mirror120.message((ServerPlayer) (Object) this, message, overlay);
        ci.cancel();
    }
}

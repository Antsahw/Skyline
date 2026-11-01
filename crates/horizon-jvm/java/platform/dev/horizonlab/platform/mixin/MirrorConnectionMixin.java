package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonServer;
import net.minecraft.network.Connection;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A mirrored player's network connection has no channel (Horizon holds the real one): it is
 * connected, not connecting (Open Parties and Claims handles a player's login only on a
 * connection that is).
 */
@Mixin(Connection.class)
public abstract class MirrorConnectionMixin {
    @Inject(method = "isConnecting", at = @At("HEAD"), cancellable = true)
    private void horizon$connected(CallbackInfoReturnable<Boolean> cir) {
        if (HorizonServer.isMirror(this)) cir.setReturnValue(false);
    }
}

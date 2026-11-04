package dev.horizonlab.platform.mixin.mods;

import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Pseudo;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * JEI keeps the client's recipes only for the server they came from (Internal's connection id:
 * the connection's address, when it is connected). The headless client's connection has no
 * channel, so it never is, and the recipes Horizon gives JEI (HorizonJei.prepare: the mirror's)
 * were dropped: JEI started with no recipes. Every player's client here is a client of the one
 * server, so the id is one name.
 */
@Pseudo
@Mixin(targets = "mezz.jei.common.Internal", remap = false)
public abstract class JeiConnectionMixin {
    @Inject(method = "getRemoteConnectionId", at = @At("HEAD"), cancellable = true, remap = false)
    private static void horizon$oneServer(CallbackInfoReturnable<String> cir) {
        cir.setReturnValue("horizon");
    }
}

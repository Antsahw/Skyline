package dev.horizonlab.platform120.mixin;

import net.minecraft.network.Connection;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Forge's answer to "does the client have this mod's channel": the bridge's client is this same
 * game, so it has every channel the server has. Forge reads the answer from what a real client
 * said at login, which a mirror player's connection never heard.
 */
@Mixin(value = net.minecraftforge.network.NetworkInstance.class, remap = false)
public abstract class NetworkInstanceMixin {
    @Inject(method = "isRemotePresent(Lnet/minecraft/network/Connection;)Z", at = @At("HEAD"), cancellable = true)
    private void horizon$present(Connection connection, CallbackInfoReturnable<Boolean> cir) {
        if (dev.horizonlab.platform120.Loader120.headless()) cir.setReturnValue(true);
    }
}

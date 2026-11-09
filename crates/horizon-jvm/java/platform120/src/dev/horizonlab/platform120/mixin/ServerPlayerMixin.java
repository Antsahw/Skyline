package dev.horizonlab.platform120.mixin;

import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** A mirror player: Horizon places it (no spawn search on a server's settings). */
@Mixin(ServerPlayer.class)
public abstract class ServerPlayerMixin {
    @Inject(method = "fudgeSpawnLocation", at = @At("HEAD"), cancellable = true)
    private void horizon$noSpawnSearch(ServerLevel level, CallbackInfo ci) {
        ci.cancel();
    }
}

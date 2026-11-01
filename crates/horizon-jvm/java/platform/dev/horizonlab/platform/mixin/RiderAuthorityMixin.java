package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonServer;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A mirrored player steering a mod's mob: vanilla leaves a ridden mob's movement to the
 * rider's client (Player.isClientAuthoritative, so LivingEntity.travelRidden only zeroes its
 * motion on the server). Horizon's games have no Java client for the mob, so the mirror
 * moves it, from the rider's keys (HorizonServer.tick's input bits).
 */
@Mixin(Player.class)
public abstract class RiderAuthorityMixin {
    @Inject(method = "isClientAuthoritative", at = @At("HEAD"), cancellable = true)
    private void horizon$mirrorSteers(CallbackInfoReturnable<Boolean> cir) {
        Player self = (Player) (Object) this;
        if (self instanceof net.minecraft.server.level.ServerPlayer && HorizonServer.isMirror(self)) {
            cir.setReturnValue(false);
        }
    }
}

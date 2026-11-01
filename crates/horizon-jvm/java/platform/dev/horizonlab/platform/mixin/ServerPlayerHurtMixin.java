package dev.horizonlab.platform.mixin;

import dev.horizonlab.bridge.Natives;
import dev.horizonlab.platform.HorizonServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.damagesource.DamageSource;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Java hurting a Horizon player (a mod's mob biting, its projectile, its effect): the mirror's
 * player is Horizon's, so the damage goes to Horizon (Natives.hurtPlayer: its type, amount,
 * where it came from and who did it), which hurts the player as its own mobs do (armour, blocking, knockback,
 * death) and runs the mods' hurt hooks then. Before mods' own heads (priority): those run
 * when Horizon hurts the player. True for a hit Horizon may take (the attacker's follow-up:
 * its knockback, its effects, on the mirror).
 */
@Mixin(value = ServerPlayer.class, priority = 100)
public abstract class ServerPlayerHurtMixin {
    @Inject(method = "hurtServer", at = @At("HEAD"), cancellable = true)
    private void horizon$hurt(ServerLevel level, DamageSource source, float amount, CallbackInfoReturnable<Boolean> cir) {
        ServerPlayer self = (ServerPlayer) (Object) this;
        if (!HorizonServer.isMirror(self) || HorizonServer.player(self.getUUID()) != self) return;
        String type = source.typeHolder().unwrapKey().map(k -> k.identifier().toString()).orElse("minecraft:generic");
        var from = source.getSourcePosition();
        // Who did it, by name (the death message's killer: "slain by Biter").
        var by = source.getEntity() != null ? source.getEntity() : source.getDirectEntity();
        Natives.hurtPlayer(self.getStringUUID(), type, amount,
                from == null ? Double.NaN : from.x, from == null ? Double.NaN : from.y, from == null ? Double.NaN : from.z,
                by == null ? null : by.getDisplayName().getString());
        cir.setReturnValue(amount > 0.0F && !self.isCreative() && !self.isSpectator());
    }
}

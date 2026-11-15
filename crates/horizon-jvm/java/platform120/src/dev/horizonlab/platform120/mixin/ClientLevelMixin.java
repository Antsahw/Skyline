package dev.horizonlab.platform120.mixin;

import dev.horizonlab.platform120.Hud120;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.Level;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A stand-in client level (Hud120.clientLevel) holds no entities of its own: one asked for by
 * id is its server level's (what a mod's sync packet names: Trinkets' player).
 */
@Mixin(ClientLevel.class)
public abstract class ClientLevelMixin {
    @Inject(method = "getEntity(I)Lnet/minecraft/world/entity/Entity;", at = @At("HEAD"), cancellable = true)
    private void horizon$entity(int id, CallbackInfoReturnable<Entity> cir) {
        Level server = Hud120.serverFor((Level) (Object) this);
        if (server != null) cir.setReturnValue(server.getEntity(id));
    }
}

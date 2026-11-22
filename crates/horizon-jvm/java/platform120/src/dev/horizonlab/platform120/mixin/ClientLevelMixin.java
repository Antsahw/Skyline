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
 * A stand-in client level (Hud120.clientLevel) has no connection and holds no entities of
 * its own: what ClientLevel asks of those is its server level's (an entity by id, which a
 * mod's sync packet names: Trinkets' player; the enabled features, which a tooltip asks).
 */
@Mixin(ClientLevel.class)
public abstract class ClientLevelMixin {
    @Inject(method = "getEntity(I)Lnet/minecraft/world/entity/Entity;", at = @At("HEAD"), cancellable = true)
    private void horizon$entity(int id, CallbackInfoReturnable<Entity> cir) {
        Level server = Hud120.serverFor((Level) (Object) this);
        if (server != null) cir.setReturnValue(server.getEntity(id));
    }

    /**
     * Its entities (what a mod's client tick looks through: Create's trains): the server
     * level's, none without one. Its own entity storage was never made.
     */
    @Inject(method = "getEntities()Lnet/minecraft/world/level/entity/LevelEntityGetter;", at = @At("HEAD"), cancellable = true)
    private void horizon$entities(CallbackInfoReturnable<net.minecraft.world.level.entity.LevelEntityGetter<Entity>> cir) {
        Level server = Hud120.serverFor((Level) (Object) this);
        if (server != null) {
            cir.setReturnValue(((LevelAccess) server).horizon$entities());
        } else if (dev.horizonlab.platform120.Loader120.headless()) {
            cir.setReturnValue(dev.horizonlab.platform120.NoEntities.INSTANCE);
        }
    }

    /** Its enabled features (its connection's): the server level's. */
    @Inject(method = "enabledFeatures()Lnet/minecraft/world/flag/FeatureFlagSet;", at = @At("HEAD"), cancellable = true)
    private void horizon$features(CallbackInfoReturnable<net.minecraft.world.flag.FeatureFlagSet> cir) {
        Level server = Hud120.serverFor((Level) (Object) this);
        if (server != null) cir.setReturnValue(server.enabledFeatures());
    }
}

package dev.horizonlab.platform120.mixin;

import java.util.Collection;
import net.fabricmc.fabric.api.networking.v1.PlayerLookup;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.level.ChunkPos;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Who sees a chunk, answered for a mirror level: Fabric asks the level's chunk map, which a
 * mirror level never built (its player map is null). GraphLib asks it whenever two graphs merge,
 * to tell the players' clients, so the throw stopped Wired Redstone's wires from ever linking.
 * The answer is the mirror's players in that level within view distance (Mirror120.tracking);
 * the other {@code tracking} overloads (block entities) come here.
 */
@Mixin(value = PlayerLookup.class, remap = false)
public abstract class PlayerLookupMixin {
    @Inject(method = "tracking(Lnet/minecraft/server/level/ServerLevel;Lnet/minecraft/world/level/ChunkPos;)Ljava/util/Collection;",
            at = @At("HEAD"), cancellable = true)
    private static void horizon$tracking(ServerLevel level, ChunkPos pos, CallbackInfoReturnable<Collection<ServerPlayer>> cir) {
        cir.setReturnValue(dev.horizonlab.platform120.Mirror120.tracking(level, pos));
    }
}

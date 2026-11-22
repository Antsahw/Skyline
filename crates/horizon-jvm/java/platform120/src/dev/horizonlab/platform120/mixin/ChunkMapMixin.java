package dev.horizonlab.platform120.mixin;

import java.util.List;
import net.minecraft.server.level.ChunkMap;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.level.ChunkPos;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Who sees a chunk, asked of a mirror level's chunk map (Forge's PacketDistributor
 * .TRACKING_CHUNK: Waystones' teleport effects), whose player map was never built: the mirror's
 * players in that level within view distance (Mirror120.tracking), as Fabric's PlayerLookup
 * gets them (PlayerLookupMixin).
 */
@Mixin(ChunkMap.class)
public abstract class ChunkMapMixin {
    @Shadow
    @Final
    ServerLevel level;

    @Inject(method = "getPlayers(Lnet/minecraft/world/level/ChunkPos;Z)Ljava/util/List;", at = @At("HEAD"), cancellable = true)
    private void horizon$players(ChunkPos pos, boolean edge, CallbackInfoReturnable<List<ServerPlayer>> cir) {
        if (level != null) cir.setReturnValue(List.copyOf(dev.horizonlab.platform120.Mirror120.tracking(level, pos)));
    }
}

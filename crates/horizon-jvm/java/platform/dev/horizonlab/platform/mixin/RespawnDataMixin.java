package dev.horizonlab.platform.mixin;

import dev.horizonlab.bridge.Natives;
import dev.horizonlab.platform.HorizonServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.storage.LevelData;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The world spawn a mod or a data pack sets on a mirrored level (/setworldspawn from Skyblock
 * Infinite's island function: ServerLevel.setRespawnData): Horizon's world spawn moves there,
 * so players respawn on the island rather than in the void.
 */
@Mixin(ServerLevel.class)
public abstract class RespawnDataMixin {
    @Inject(method = "setRespawnData", at = @At("TAIL"))
    private void horizon$worldSpawn(LevelData.RespawnData data, CallbackInfo ci) {
        if (!HorizonServer.isMirror(this) || data == null) return;
        var pos = data.pos();
        Natives.log("Java mods: the world spawn moves to " + pos.toShortString() + " (" + data.dimension().identifier() + ")");
        Natives.worldSpawn(data.dimension().identifier().toString(), pos.getX(), pos.getY(), pos.getZ(), data.yaw(), data.pitch());
    }
}

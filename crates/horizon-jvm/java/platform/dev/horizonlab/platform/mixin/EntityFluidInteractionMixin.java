package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonServer;
import net.minecraft.world.entity.EntityFluidInteraction;
import net.minecraft.world.level.Level;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * An entity's fluid check on a mirrored level: vanilla asks each chunk's sections whether they
 * may hold a fluid (a whole chunk copy each tick, per mob); here every loaded box may, so the
 * exact check that follows reads the box's blocks (a few, from HorizonServer's section copies).
 */
@Mixin(EntityFluidInteraction.class)
public abstract class EntityFluidInteractionMixin {
    @Inject(method = "hasFluidAndLoaded", at = @At("HEAD"), cancellable = true)
    private static void horizon$hasFluid(Level level, int minX, int minY, int minZ, int maxX, int maxY, int maxZ,
                                         CallbackInfoReturnable<Boolean> cir) {
        if (!HorizonServer.isMirror(level)) return;
        for (int x = minX >> 4; x <= maxX >> 4; x++) {
            for (int z = minZ >> 4; z <= maxZ >> 4; z++) {
                if (!HorizonServer.loaded(level, x << 4, minY, z << 4)) {
                    cir.setReturnValue(false);
                    return;
                }
            }
        }
        cir.setReturnValue(true);
    }
}

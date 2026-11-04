package dev.horizonlab.kineticmod.mixin;

import dev.horizonlab.kineticmod.KineticMod;
import net.minecraft.network.syncher.SyncedDataHolder;
import net.minecraft.network.syncher.SynchedEntityData;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Create's SynchedEntityDataBuilderMixin: the mod's data accessors made per entity class. */
@Mixin(SynchedEntityData.Builder.class)
public class SynchedDataBuilderMixin {
    @Inject(method = "<init>", at = @At("TAIL"))
    private void horizonKinetic$classData(SyncedDataHolder entity, CallbackInfo ci) {
        KineticMod.DATA_CLASSES.add(entity.getClass());
    }
}

package dev.horizonlab.platform.mixin.mods;

import dev.horizonlab.platform.HorizonClientConfig;
import java.io.File;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Pseudo;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A LAN guest's JourneyMap settings stay in memory (HorizonClientConfig keeps them per
 * player): saved, they'd overwrite the host's files.
 */
@Pseudo
@Mixin(targets = "journeymap.common.properties.PropertiesBase", remap = false)
public abstract class JourneyMapPropertiesMixin {
    @Inject(method = "save(Ljava/io/File;Z)Z", at = @At("HEAD"), cancellable = true, remap = false)
    private void horizon$guestsInMemory(File file, boolean force, CallbackInfoReturnable<Boolean> cir) {
        if (HorizonClientConfig.guestNow()) cir.setReturnValue(true);
    }
}

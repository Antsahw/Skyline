package dev.horizonlab.kineticmod.mixin;

import dev.horizonlab.kineticmod.KineticClient;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.extract.LevelExtractor;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Flywheel's LevelExtractorMixin.flywheel$reload: its helpers made at allChanged's end. */
@Mixin(LevelExtractor.class)
public class ReloadMixin {
    @Shadow
    private ClientLevel level;

    @Inject(method = "allChanged()V", at = @At("RETURN"))
    private void horizonKinetic$reload(CallbackInfo ci) {
        if (level != null) KineticClient.helper = Minecraft.getInstance().getBlockColors();
    }
}

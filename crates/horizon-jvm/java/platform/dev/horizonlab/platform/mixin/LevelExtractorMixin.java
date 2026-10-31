package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonClient;
import net.minecraft.client.renderer.extract.LevelExtractor;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The headless client's level extractor (26.3's level renderer) is made without its
 * constructor: Horizon meshes and draws the world. Mods that ask it to redraw (Serene Seasons
 * when the season's colours change: allChanged; blocks or sections made dirty) ask nothing.
 */
@Mixin(LevelExtractor.class)
public abstract class LevelExtractorMixin {
    @Inject(method = {"allChanged", "setBlocksDirty", "setBlockDirty", "setSectionDirtyWithNeighbors",
            "setSectionRangeDirty", "setSectionDirty", "blockChanged", "resetSampler"}, at = @At("HEAD"), cancellable = true)
    private void horizon$nothingToRedraw(CallbackInfo ci) {
        if (HorizonClient.headless()) ci.cancel();
    }
}

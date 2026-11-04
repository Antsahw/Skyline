package dev.horizonlab.kineticmod.mixin;

import dev.horizonlab.kineticmod.KineticClient;
import net.minecraft.client.GameLoadCookie;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Create's MinecraftMixin.endReload: Flywheel's backend chosen at the resource reload's end. */
@Mixin(Minecraft.class)
public class ResourcesMixin {
    @Inject(method = "onResourceLoadFinished(Lnet/minecraft/client/GameLoadCookie;)V", at = @At("HEAD"))
    private void horizonKinetic$endReload(GameLoadCookie cookie, CallbackInfo ci) {
        KineticClient.reloadEnded = true;
    }
}

package dev.horizonlab.fovmod.mixin;

import dev.horizonlab.fovmod.FovClient;

import com.llamalad7.mixinextras.injector.ModifyReturnValue;
import net.minecraft.client.Camera;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/** Zoomify's way: the camera's field of view, halved while the zoom key is held. */
@Mixin(Camera.class)
public abstract class CameraMixin {
    @ModifyReturnValue(method = "calculateFov", at = @At("RETURN"))
    private float horizonFov$zoom(float fov) {
        return FovClient.zoom != null && FovClient.zoom.isDown() ? fov * 0.5f : fov;
    }
}

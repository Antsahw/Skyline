package dev.horizonlab.platform120.mixin;

import net.minecraft.client.Camera;
import net.minecraft.client.renderer.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/** A GameRenderer made without its constructor gets its camera here (Render120). */
@Mixin(GameRenderer.class)
public interface GameRendererAccess {
    @Mutable
    @Accessor("mainCamera")
    void horizon$setMainCamera(Camera camera);
}

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

    @org.spongepowered.asm.mixin.Mutable
    @Accessor("minecraft")
    void horizon$setMinecraft(net.minecraft.client.Minecraft minecraft);

    @Accessor("fov")
    void horizon$setFov(float fov);

    @Accessor("oldFov")
    void horizon$setOldFov(float fov);

    /** The field of view, with what mods inject into it (a zoom mod's). */
    @org.spongepowered.asm.mixin.gen.Invoker("getFov")
    double horizon$getFov(net.minecraft.client.Camera camera, float partialTick, boolean useFovSetting);
}

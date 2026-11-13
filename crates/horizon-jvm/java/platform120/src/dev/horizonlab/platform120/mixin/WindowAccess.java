package dev.horizonlab.platform120.mixin;

import com.mojang.blaze3d.platform.Window;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/**
 * The never-opened window's size (Screens120.window): a frame's, at GUI scale 1, so a screen
 * that turns the mouse's window position into its own (Cloth Config's) gets the same point.
 */
@Mixin(Window.class)
public interface WindowAccess {
    @Accessor("width")
    void horizon$setWidth(int width);

    @Accessor("height")
    void horizon$setHeight(int height);

    @Accessor("framebufferWidth")
    void horizon$setFramebufferWidth(int width);

    @Accessor("framebufferHeight")
    void horizon$setFramebufferHeight(int height);

    @Accessor("guiScaledWidth")
    void horizon$setGuiScaledWidth(int width);

    @Accessor("guiScaledHeight")
    void horizon$setGuiScaledHeight(int height);

    @Accessor("guiScale")
    void horizon$setGuiScale(double scale);
}

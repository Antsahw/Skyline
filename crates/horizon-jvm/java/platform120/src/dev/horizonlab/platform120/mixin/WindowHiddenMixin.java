package dev.horizonlab.platform120.mixin;

import com.mojang.blaze3d.platform.Window;
import org.lwjgl.glfw.GLFW;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * A client Horizon started (Forge's: docs/java-bridge-120-forge.md) keeps its window hidden: it
 * needs GL to load its mods and bake their models, but nothing of it is ever shown (Horizon draws
 * the game). The Fabric runtime's headless client makes no window at all.
 */
@Mixin(Window.class)
public abstract class WindowHiddenMixin {
    /**
     * Hidden as soon as it is made: Forge makes the window itself (its own window handling takes
     * over vanilla's creation), so the hint can't go before; this is the moment after.
     */
    @Inject(method = "<init>", at = @At("RETURN"))
    private void horizon$hidden(CallbackInfo ci) {
        if (System.getenv("HORIZON_REMOTE_CONNECT") != null) GLFW.glfwHideWindow(((Window) (Object) this).getWindow());
    }
}

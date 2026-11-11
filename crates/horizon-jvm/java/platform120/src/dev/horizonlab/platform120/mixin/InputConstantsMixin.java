package dev.horizonlab.platform120.mixin;

import com.mojang.blaze3d.platform.InputConstants;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Whether a key is held, asked of GLFW by a screen (a shift-click moves a whole stack): this
 * client has no GLFW, so the answer is the modifiers the player's input came with.
 */
@Mixin(InputConstants.class)
public abstract class InputConstantsMixin {
    @Inject(method = "isKeyDown(JI)Z", at = @At("HEAD"), cancellable = true)
    private static void horizon$keyDown(long window, int key, CallbackInfoReturnable<Boolean> cir) {
        cir.setReturnValue(dev.horizonlab.platform120.Screens120.keyDown(key));
    }
}

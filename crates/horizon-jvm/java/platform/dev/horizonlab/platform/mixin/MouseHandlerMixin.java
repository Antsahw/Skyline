package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonClient;
import net.minecraft.client.MouseHandler;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Mods call the mouse's window events themselves (JourneyMap's screens release the button
 * as they open: onButton): the headless client has no window. Its input reaches screens
 * from Horizon's window (HorizonScreenEvents); its mouse handler only keeps the position.
 */
@Mixin(MouseHandler.class)
public abstract class MouseHandlerMixin {
    @Inject(method = {"onButton", "onScroll", "onDrop", "onMove"}, at = @At("HEAD"), cancellable = true)
    private void horizon$noWindow(CallbackInfo ci) {
        if (HorizonClient.headless()) ci.cancel();
    }
}

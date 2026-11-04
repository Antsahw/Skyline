package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonClient;
import dev.horizonlab.platform.HorizonTextFields;
import net.minecraft.client.gui.components.EditBox;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** A text box taking or losing the keyboard, in the headless client (HorizonTextFields). */
@Mixin(EditBox.class)
public abstract class EditBoxFocusMixin {
    @Inject(method = "setFocused", at = @At("TAIL"))
    private void horizon$focus(boolean focused, CallbackInfo ci) {
        if (HorizonClient.headless()) HorizonTextFields.focusChanged((EditBox) (Object) this, focused);
    }
}

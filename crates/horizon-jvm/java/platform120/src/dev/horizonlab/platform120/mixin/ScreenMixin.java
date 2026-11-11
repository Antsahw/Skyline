package dev.horizonlab.platform120.mixin;

import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.GuiGraphics;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A mod's screen in the headless client. Horizon shows it and reads it aloud if it reads
 * anything: this runtime has no narrator (Minecraft's constructor makes one; this client's
 * Minecraft never ran its constructor), so a screen asking whether to narrate is told no.
 * Every narration path of a 1.20.1 screen asks that one question first.
 */
@Mixin(Screen.class)
public abstract class ScreenMixin {
    @Shadow
    public int width;
    @Shadow
    public int height;

    /**
     * The darkened world behind a menu. Vanilla draws that wash only with a level loaded and
     * the title screen's dirt otherwise; this client has no level, but Horizon always shows a
     * screen over its world, so it is always the wash -- vanilla's own colours.
     */
    @Inject(method = "renderBackground(Lnet/minecraft/client/gui/GuiGraphics;)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$background(GuiGraphics graphics, CallbackInfo ci) {
        if (dev.horizonlab.platform120.Graphics120.current() != null) {
            graphics.fillGradient(0, 0, this.width, this.height, -1072689136, -804253680);
            ci.cancel();
        }
    }
    @Inject(method = "shouldRunNarration()Z", at = @At("HEAD"), cancellable = true)
    private void horizon$noNarrator(CallbackInfoReturnable<Boolean> cir) {
        cir.setReturnValue(false);
    }
}
